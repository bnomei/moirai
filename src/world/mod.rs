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
pub use bundle::{Bundle, BundleWriter, DynamicBundle};
pub use error::{
    EventReadError, FlushError, FlushReport, WorldAllocatorError, WorldError,
};
pub use crate::command::Commands;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{type_name, TypeId};

use crate::command::{CommandQueue, ErasedComponentValue};
use crate::component::{ComponentId, ComponentRegistry, StorageKind};
use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::event::{EventRegistry, EventStorage};
use crate::resource::ResourceStore;
use crate::storage::{ArchetypeStorage, SparseStore, TypedSparseStorage};
use crate::time::{ChangeTick, ChangeTickError, WorldTick};

use self::guard::RunGuard;

pub(crate) struct WorldEvents {
    pub registry: EventRegistry,
    pub storage: EventStorage,
}

/// ECS world with checked sparse-component lifecycle.
pub struct World {
    owner: WorldOwner,
    allocator: EntityAllocator,
    registry: ComponentRegistry,
    sparse_stores: Vec<SparseStore>,
    archetypes: ArchetypeStorage,
    resources: ResourceStore,
    events: WorldEvents,
    command_queue: CommandQueue,
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
        resources: ResourceStore,
        events: WorldEvents,
    ) -> Self {
        Self {
            owner,
            allocator: EntityAllocator::new(),
            registry,
            sparse_stores,
            archetypes,
            resources,
            events,
            command_queue: CommandQueue::new(),
            change_tick: ChangeTick::ZERO,
            world_tick: WorldTick::ZERO,
            run_guard: RunGuard::Idle,
            mutation_poisoned: false,
        }
    }

    pub fn world_tick(&self) -> WorldTick {
        self.world_tick
    }

    pub fn commands(&mut self) -> Result<Commands<'_>, WorldError> {
        if matches!(
            self.run_guard,
            RunGuard::Running(crate::operation::StageOperation::Render)
        ) {
            return Err(WorldError::StructuralCommandsDuringRender);
        }
        Ok(Commands::new(self))
    }

    #[allow(dead_code)]
    pub(crate) fn begin_run(
        &mut self,
        operation: crate::operation::StageOperation,
    ) -> Result<(), WorldError> {
        if !self.run_guard.is_idle() {
            return Err(WorldError::NestedRun);
        }
        self.run_guard = RunGuard::Running(operation);
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn end_run(&mut self) {
        self.run_guard = RunGuard::Idle;
    }

    #[allow(dead_code)]
    pub(crate) fn advance_world_tick(&mut self) {
        if let Some(next) = self.world_tick.raw().checked_add(1) {
            self.world_tick.set_raw(next);
        }
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
        self.ensure_live_access(entity)?;
        self.remove_all_components(entity)?;
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
        self.ensure_live_access(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.is_tag_component(&component_id) {
            let tick = self.issue_change_tick()?;
            let _ = self.insert_tag_typed(entity, component_id, tick);
            return Ok(None);
        }
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

    pub fn add_tag(&mut self, entity: EntityId, tag: &ComponentId) -> Result<bool, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_live_access(entity)?;
        tag.validate_owner(&self.owner)?;
        if !self.is_tag_component(tag) {
            return Err(WorldError::WrongStorageKind {
                name: format!("component {}", tag.index()),
            });
        }
        let tick = self.issue_change_tick()?;
        Ok(self.insert_tag_index(entity, tag.index(), tick))
    }

    pub fn remove_tag(&mut self, entity: EntityId, tag: &ComponentId) -> Result<bool, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_live_access(entity)?;
        tag.validate_owner(&self.owner)?;
        if !self.is_tag_component(tag) {
            return Err(WorldError::WrongStorageKind {
                name: format!("component {}", tag.index()),
            });
        }
        Ok(self.remove_tag_index(entity, tag.index()))
    }

    pub fn has_tag(&self, entity: EntityId, tag: &ComponentId) -> Result<bool, WorldError> {
        self.ensure_live_access(entity)?;
        tag.validate_owner(&self.owner)?;
        if !self.is_tag_component(tag) {
            return Err(WorldError::WrongStorageKind {
                name: format!("component {}", tag.index()),
            });
        }
        Ok(self.tag_store(tag.index()).contains(entity))
    }

    pub fn get<T: Clone + 'static>(&self, entity: EntityId) -> Result<Option<&T>, WorldError> {
        self.ensure_live_access(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.is_tag_component(&component_id) {
            return Ok(None);
        }
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
        self.ensure_live_access(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.is_tag_component(&component_id) {
            return Err(WorldError::WrongStorageKind {
                name: String::from(type_name::<T>()),
            });
        }
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
        self.ensure_live_access(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        if self.is_tag_component(&component_id) {
            let _ = self.remove_tag_index(entity, component_id.index());
            return Ok(None);
        }
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

    pub(crate) fn owner(&self) -> &WorldOwner {
        &self.owner
    }

    pub(crate) fn command_queue_mut(&mut self) -> &mut CommandQueue {
        &mut self.command_queue
    }

    pub(crate) fn commit_command_ops(&mut self, tick: ChangeTick) -> Result<usize, WorldError> {
        let ops = self.command_queue.take_ops();
        let count = ops.len();
        for op in ops {
            op.commit(self, tick)?;
        }
        Ok(count)
    }

    pub(crate) fn allocator_mut(&mut self) -> &mut EntityAllocator {
        &mut self.allocator
    }

    pub(crate) fn allocator_is_reserved(&self, entity: EntityId) -> bool {
        self.allocator.is_reserved(entity)
    }

    pub(crate) fn ensure_command_target(&self, entity: EntityId) -> Result<(), WorldError> {
        if self.allocator.is_alive(entity) || self.allocator.is_reserved(entity) {
            Ok(())
        } else {
            Err(WorldError::StaleEntity { entity })
        }
    }

    pub(crate) fn component_index<T: 'static>(&self) -> Result<usize, WorldError> {
        Ok(self.component_id::<T>()?.index())
    }

    pub(crate) fn is_tag_component(&self, component_id: &ComponentId) -> bool {
        self.registry.is_tag(component_id) == Some(true)
    }

    pub(crate) fn collect_live_entities(&self, out: &mut Vec<EntityId>) {
        self.collect_live_entities_from_slots(out);
    }

    pub(crate) fn validate_component_insert(
        &self,
        entity: EntityId,
        component_index: u32,
        type_id: TypeId,
    ) -> Result<(), WorldError> {
        self.ensure_command_target(entity)?;
        let component_id = ComponentId::new(self.owner.clone(), component_index);
        component_id.validate_owner(&self.owner)?;
        if self.is_tag_component(&component_id) {
            if type_id != TypeId::of::<()>() {
                return Err(WorldError::WrongStorageKind {
                    name: format!("component {component_index}"),
                });
            }
            return Ok(());
        }
        if type_id != self.expected_type_id(component_index)? {
            return Err(WorldError::WrongStorageKind {
                name: format!("component {component_index}"),
            });
        }
        Ok(())
    }

    pub(crate) fn validate_component_remove(
        &self,
        entity: EntityId,
        component_index: u32,
    ) -> Result<(), WorldError> {
        self.ensure_command_target(entity)?;
        let component_id = ComponentId::new(self.owner.clone(), component_index);
        component_id.validate_owner(&self.owner)?;
        Ok(())
    }

    pub(crate) fn commit_reserved_spawn(
        &mut self,
        entity: EntityId,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let _ = tick;
        self.allocator
            .commit_reserved(entity)
            .map_err(|error| self.map_allocator_error(entity, error))
    }

    pub(crate) fn commit_despawn(&mut self, entity: EntityId) -> Result<(), WorldError> {
        if self.allocator.is_reserved(entity) {
            return self
                .allocator
                .release_reserved(entity)
                .map_err(|error| self.map_allocator_error(entity, error));
        }
        self.remove_all_components(entity)?;
        self.allocator
            .free(entity)
            .map_err(|error| self.map_allocator_error(entity, error))
    }

    pub(crate) fn commit_insert_erased<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
        component_index: u32,
        value: T,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let component_id = ComponentId::new(self.owner.clone(), component_index);
        if self.is_tag_component(&component_id) {
            let _ = self.insert_tag_index(entity, component_index as usize, tick);
            return Ok(());
        }
        if self.registry.is_table_component(&component_id) {
            let _ = self
                .archetypes
                .insert_table(entity, component_index, value, tick);
            return Ok(());
        }
        let store = self
            .sparse_stores
            .get_mut(component_index as usize)
            .and_then(|store| store.typed_mut::<T>())
            .ok_or_else(|| WorldError::WrongStorageKind {
                name: format!("component {component_index}"),
            })?;
        let _ = store.insert_with_tick(entity, value, tick);
        Ok(())
    }

    pub(crate) fn commit_insert_tag_index(
        &mut self,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let _ = self.insert_tag_index(entity, component_index as usize, tick);
        Ok(())
    }

    pub(crate) fn commit_remove_index(
        &mut self,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let _ = tick;
        let component_id = ComponentId::new(self.owner.clone(), component_index);
        if self.is_tag_component(&component_id) {
            let _ = self.remove_tag_index(entity, component_index as usize);
            return Ok(());
        }
        if self.registry.is_table_component(&component_id) {
            let _ = self
                .archetypes
                .remove_table_index(entity, component_index);
            return Ok(());
        }
        if let Some(store) = self.sparse_stores.get_mut(component_index as usize) {
            if let Some(erased) = store.as_erased_mut() {
                erased.remove_entity(entity);
            }
        }
        Ok(())
    }

    pub(crate) fn insert_dynamic(
        &mut self,
        entity: EntityId,
        component_id: ComponentId,
        value: Box<dyn ErasedComponentValue>,
    ) -> Result<Option<()>, WorldError> {
        let tick = self.issue_change_tick()?;
        value.apply_insert(self, entity, component_id.index() as u32, tick)?;
        Ok(None)
    }

    pub(crate) fn add_tag_id(
        &mut self,
        entity: EntityId,
        tag: ComponentId,
    ) -> Result<(), WorldError> {
        let tick = self.issue_change_tick()?;
        let _ = self.insert_tag_index(entity, tag.index(), tick);
        Ok(())
    }

    pub(crate) fn ensure_mutable(&self) -> Result<(), WorldError> {
        if self.mutation_poisoned {
            Err(WorldError::ChangeTickExhausted)
        } else {
            Ok(())
        }
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

    fn tag_store(&self, index: usize) -> &crate::storage::TagSparseStorage {
        self.sparse_stores
            .get(index)
            .and_then(|store| store.tag())
            .expect("tag store exists for tag component")
    }

    fn tag_store_mut(&mut self, index: usize) -> &mut crate::storage::TagSparseStorage {
        self.sparse_stores
            .get_mut(index)
            .and_then(|store| store.tag_mut())
            .expect("tag store exists for tag component")
    }

    fn insert_tag_typed(
        &mut self,
        entity: EntityId,
        component_id: ComponentId,
        tick: ChangeTick,
    ) -> bool {
        self.insert_tag_index(entity, component_id.index(), tick)
    }

    fn insert_tag_index(&mut self, entity: EntityId, index: usize, tick: ChangeTick) -> bool {
        self.tag_store_mut(index).insert_with_tick(entity, tick)
    }

    fn remove_tag_index(&mut self, entity: EntityId, index: usize) -> bool {
        self.tag_store_mut(index).remove(entity)
    }

    fn remove_all_components(&mut self, entity: EntityId) -> Result<(), WorldError> {
        for store in &mut self.sparse_stores {
            store.remove_entity(entity);
        }
        self.archetypes.remove_entity(entity);
        Ok(())
    }

    fn expected_type_id(&self, component_index: u32) -> Result<TypeId, WorldError> {
        self.registry
            .type_id_for_index(component_index as usize)
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: format!("component {component_index}"),
            })
    }

    fn collect_live_entities_from_slots(&self, out: &mut Vec<EntityId>) {
        let counts = self.allocator.counts();
        let capacity = counts.live as usize + counts.reserved as usize;
        out.reserve(capacity);
        for slot in 0..self.allocator.slot_capacity() {
            let generation = self.allocator.generation_for_slot(slot);
            if generation == 0 {
                continue;
            }
            let id = EntityId::from_parts(slot as u32, generation);
            if self.allocator.is_alive(id) || self.allocator.is_reserved(id) {
                out.push(id);
            }
        }
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

    fn ensure_live_access(&self, entity: EntityId) -> Result<(), WorldError> {
        if self.allocator.is_alive(entity) {
            Ok(())
        } else if self.allocator.is_reserved(entity) {
            Err(WorldError::EntityNotLive { entity })
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

    #[derive(Clone, Copy)]
    struct Player;

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
    fn tag_insert_and_has() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("register");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        assert!(world.add_tag(entity, &tag).expect("add"));
        assert!(world.has_tag(entity, &tag).expect("has"));
    }

    #[test]
    fn structural_mutation_rejects_while_running() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn before run");
        world
            .begin_run(crate::operation::StageOperation::Update)
            .expect("begin");
        assert!(matches!(
            world.insert(entity, Marker(1)),
            Err(WorldError::StructuralMutationDuringRun)
        ));
        world.end_run();
    }

    #[test]
    fn frame_events_clear_per_operation() {
        use crate::event::{EventOptions, EventReaderStart};
        use crate::operation::StageOperation;

        #[derive(Clone)]
        struct FrameEvent(#[allow(dead_code)] u8);

        let mut builder = WorldBuilder::new();
        builder
            .add_event::<FrameEvent>(EventOptions::frame(StageOperation::Update))
            .expect("register");
        let mut world = builder.build().expect("build");
        world.send(FrameEvent(1)).expect("send");
        world.clear_frame_events(StageOperation::Update);
        let mut reader = world
            .event_reader::<FrameEvent>(EventReaderStart::OldestRetained)
            .expect("reader");
        assert!(world.read_event(&mut reader).expect("read").is_none());
    }

    #[test]
    fn render_rejects_structural_commands() {
        let mut world = test_world();
        world
            .begin_run(crate::operation::StageOperation::Render)
            .expect("begin");
        assert!(matches!(
            world.commands(),
            Err(WorldError::StructuralCommandsDuringRender)
        ));
        world.end_run();
    }

    #[test]
    fn nested_run_rejected() {
        let mut world = test_world();
        world
            .begin_run(crate::operation::StageOperation::Update)
            .expect("begin");
        assert!(matches!(
            world.begin_run(crate::operation::StageOperation::Render),
            Err(WorldError::NestedRun)
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