mod access;
mod builder;
mod bundle;
mod error;
mod events;
mod flush;
mod guard;
mod owner;

pub(crate) use owner::WorldOwner;
mod query;
mod resources;
mod spawn;

pub use builder::WorldBuilder;
pub use bundle::{Bundle, BundleWriter};
pub use error::{WorldAllocatorError, WorldError};

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;

use crate::component::{ComponentId, ComponentRegistry, StorageKind};
use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::storage::{ArchetypeStorage, SparseStore, TypedSparseStorage};
use crate::time::{ChangeTick, ChangeTickError, WorldTick};

use self::guard::RunGuard;

/// ECS world with checked sparse-component lifecycle.
pub struct World {
    owner: WorldOwner,
    allocator: EntityAllocator,
    registry: ComponentRegistry,
    sparse_stores: Vec<SparseStore>,
    archetypes: ArchetypeStorage,
    change_tick: ChangeTick,
    world_tick: WorldTick,
    run_guard: RunGuard,
    mutation_poisoned: bool,
}

impl World {
    pub(crate) fn from_parts(
        owner: WorldOwner,
        registry: ComponentRegistry,
        sparse_stores: Vec<SparseStore>,
        archetypes: ArchetypeStorage,
    ) -> Self {
        Self {
            owner,
            allocator: EntityAllocator::new(),
            registry,
            sparse_stores,
            archetypes,
            change_tick: ChangeTick::ZERO,
            world_tick: WorldTick::ZERO,
            run_guard: RunGuard::Idle,
            mutation_poisoned: false,
        }
    }

    pub fn world_tick(&self) -> WorldTick {
        self.world_tick
    }

    #[allow(dead_code)]
    pub(crate) fn begin_run(&mut self, operation: crate::operation::StageOperation) {
        self.run_guard = RunGuard::Running(operation);
    }

    #[allow(dead_code)]
    pub(crate) fn end_run(&mut self) {
        self.run_guard = RunGuard::Idle;
    }

    pub fn spawn_bundle<B: Bundle>(&mut self, bundle: B) -> Result<EntityId, WorldError> {
        let entity = self.spawn()?;
        bundle.write(&mut BundleWriter::new(self, entity))?;
        Ok(entity)
    }

    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.allocator.is_alive(entity)
    }

    pub fn spawn(&mut self) -> Result<EntityId, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        Ok(self.allocator.alloc())
    }

    pub fn despawn(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_alive(entity)?;
        for store in &mut self.sparse_stores {
            store.remove_entity(entity);
        }
        self.archetypes.remove_entity(entity);
        self.allocator
            .free(entity)
            .map_err(|error| self.map_allocator_error(entity, error))
    }

    pub fn insert<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
        value: T,
    ) -> Result<Option<T>, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.registry.is_table_component(&component_id) {
            let tick = self.issue_change_tick()?;
            return Ok(self.archetypes.insert_table(
                entity,
                component_id.index() as u32,
                value,
                tick,
            ));
        }
        self.ensure_sparse_kind(&component_id)?;
        let index = component_id.index();
        let tick = self.issue_change_tick()?;
        let store = self
            .sparse_stores
            .get_mut(index)
            .and_then(|store| store.typed_mut::<T>())
            .ok_or_else(|| WorldError::WrongStorageKind {
                name: String::from(type_name::<T>()),
            })?;
        Ok(store.insert_with_tick(entity, value, tick))
    }

    pub fn get<T: Clone + 'static>(&self, entity: EntityId) -> Result<Option<&T>, WorldError> {
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.registry.is_table_component(&component_id) {
            return Ok(self
                .archetypes
                .get_table(entity, component_id.index() as u32));
        }
        Ok(self.sparse_store::<T>(component_id)?.get(entity))
    }

    pub fn get_mut<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
    ) -> Result<Option<&mut T>, WorldError> {
        self.ensure_mutable()?;
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.registry.is_table_component(&component_id) {
            if self
                .archetypes
                .get_table::<T>(entity, component_id.index() as u32)
                .is_none()
            {
                return Ok(None);
            }
            let tick = self.issue_change_tick()?;
            return Ok(self
                .archetypes
                .get_table_mut(entity, component_id.index() as u32, tick));
        }
        self.ensure_sparse_kind(&component_id)?;
        let index = component_id.index();
        let has_component = match self
            .sparse_stores
            .get(index)
            .and_then(|store| store.typed::<T>())
        {
            Some(store) => store.contains(entity),
            None => {
                return Err(WorldError::WrongStorageKind {
                    name: String::from(type_name::<T>()),
                });
            }
        };
        if !has_component {
            return Ok(None);
        }
        let tick = self.issue_change_tick()?;
        Ok(self
            .sparse_stores
            .get_mut(index)
            .and_then(|store| store.typed_mut::<T>())
            .expect("typed sparse store checked above")
            .get_mut_with_tick(entity, tick))
    }

    pub fn remove<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
    ) -> Result<Option<T>, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.registry.is_table_component(&component_id) {
            return Ok(self
                .archetypes
                .remove_table(entity, component_id.index() as u32));
        }
        Ok(self.sparse_store_mut::<T>(component_id)?.remove(entity))
    }

    pub fn len_sparse<T: 'static>(&self) -> Result<usize, WorldError> {
        let component_id = self.component_id::<T>()?;
        Ok(self.sparse_store::<T>(component_id)?.len())
    }

    fn component_id<T: 'static>(&self) -> Result<ComponentId, WorldError> {
        self.registry
            .id_of::<T>(&self.owner)
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: String::from(type_name::<T>()),
            })
    }

    fn sparse_store<T: 'static>(
        &self,
        component_id: ComponentId,
    ) -> Result<&TypedSparseStorage<T>, WorldError> {
        self.ensure_sparse_kind(&component_id)?;
        self.sparse_stores
            .get(component_id.index())
            .and_then(|store| store.typed::<T>())
            .ok_or_else(|| WorldError::WrongStorageKind {
                name: String::from(type_name::<T>()),
            })
    }

    fn sparse_store_mut<T: 'static>(
        &mut self,
        component_id: ComponentId,
    ) -> Result<&mut TypedSparseStorage<T>, WorldError> {
        self.ensure_sparse_kind(&component_id)?;
        self.sparse_stores
            .get_mut(component_id.index())
            .and_then(|store| store.typed_mut::<T>())
            .ok_or_else(|| WorldError::WrongStorageKind {
                name: String::from(type_name::<T>()),
            })
    }

    fn ensure_sparse_kind(&self, component_id: &ComponentId) -> Result<(), WorldError> {
        match self.registry.storage_kind(component_id) {
            Some(StorageKind::Sparse) => Ok(()),
            Some(StorageKind::Table) => Err(WorldError::WrongStorageKind {
                name: format!("component {}", component_id.index()),
            }),
            None => Err(WorldError::UnregisteredComponent {
                name: format!("component {}", component_id.index()),
            }),
        }
    }

    fn ensure_alive(&self, entity: EntityId) -> Result<(), WorldError> {
        if self.allocator.is_alive(entity) {
            Ok(())
        } else {
            Err(WorldError::StaleEntity { entity })
        }
    }

    fn ensure_idle_structural(&self) -> Result<(), WorldError> {
        if self.run_guard.is_idle() {
            Ok(())
        } else {
            Err(WorldError::StructuralMutationDuringRun)
        }
    }

    fn ensure_mutable(&self) -> Result<(), WorldError> {
        if self.mutation_poisoned {
            Err(WorldError::ChangeTickExhausted)
        } else {
            Ok(())
        }
    }

    fn issue_change_tick(&mut self) -> Result<ChangeTick, WorldError> {
        match self.change_tick.issue() {
            Ok(tick) => Ok(tick),
            Err(ChangeTickError::Exhausted) => {
                self.mutation_poisoned = true;
                Err(WorldError::ChangeTickExhausted)
            }
        }
    }

    fn map_allocator_error(&self, entity: EntityId, error: AllocatorError) -> WorldError {
        match error {
            AllocatorError::GenerationOverflow => {
                WorldError::Allocator(WorldAllocatorError::GenerationOverflow)
            }
            AllocatorError::SlotRetired => WorldError::Allocator(WorldAllocatorError::SlotRetired),
            AllocatorError::StaleEntity | AllocatorError::DoubleFree | AllocatorError::NotLive => {
                WorldError::StaleEntity { entity }
            }
        }
    }

    #[cfg(test)]
    fn set_change_tick_for_test(&mut self, tick: ChangeTick) {
        self.change_tick = tick;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::time::ChangeTick;

    #[derive(Clone, Copy)]
    struct Marker(u8);

    #[derive(Clone, Copy)]
    struct Other;

    #[derive(Clone, Copy)]
    struct TableComp(i32);

    fn test_world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("register marker");
        builder
            .register_component::<Other>(ComponentOptions::sparse())
            .expect("register other");
        builder.build().expect("build")
    }

    fn table_world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("register marker");
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("register table");
        builder.build().expect("build")
    }

    #[test]
    fn table_insert_round_trip() {
        let mut world = table_world();
        let entity = world.spawn().expect("spawn");
        assert!(world
            .insert(entity, TableComp(7))
            .expect("insert")
            .is_none());
        assert_eq!(
            world.get::<TableComp>(entity).expect("get").map(|c| c.0),
            Some(7)
        );
    }

    #[test]
    fn structural_mutation_rejects_while_running() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn before run");
        world.begin_run(crate::operation::StageOperation::Update);
        assert!(matches!(
            world.insert(entity, Marker(1)),
            Err(WorldError::StructuralMutationDuringRun)
        ));
        world.end_run();
    }

    #[test]
    fn missing_component_lookup_does_not_issue_change_tick() {
        let mut world = test_world();
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 2));
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Marker(1)).expect("seed");
        assert!(world.get_mut::<Other>(entity).expect("missing").is_none());
        assert!(world
            .insert(entity, Marker(2))
            .expect("still mutable")
            .is_some());
        assert!(matches!(
            world.insert(entity, Marker(3)),
            Err(WorldError::ChangeTickExhausted)
        ));
    }

    #[test]
    fn change_tick_exhaustion_poison_world_mutations() {
        let mut world = test_world();
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        let entity = world.spawn().expect("spawn before exhaustion");
        world.insert(entity, Marker(1)).expect("consume last tick");
        assert!(matches!(
            world.insert(entity, Marker(2)),
            Err(WorldError::ChangeTickExhausted)
        ));
        assert_eq!(world.spawn(), Err(WorldError::ChangeTickExhausted));
        assert!(world.is_alive(entity));
        assert_eq!(
            world
                .get::<Marker>(entity)
                .expect("read-only get")
                .map(|m| m.0),
            Some(1)
        );
    }
}
