mod access;
mod builder;
mod bundle;
mod error;
mod events;
mod flush;
pub(crate) mod guard;
mod lease;
mod owner;

#[cfg(any(test, feature = "testkit"))]
pub(crate) use events::set_event_sequence_for_test;
pub(crate) use owner::WorldOwner;
pub(crate) mod query;
mod resources;
mod spawn;

pub use crate::command::Commands;
pub use access::{DenseEntityScratch, EntityScratchError};
pub use builder::WorldBuilder;
pub use bundle::{Bundle, BundleWriter, DynamicBundle};
pub use error::{EventReadError, FlushError, FlushReport, WorldAllocatorError, WorldError};

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{type_name, TypeId};
use core::cell::{Cell, RefCell};

use crate::command::{CommandQueue, ErasedComponentValue};
use crate::component::{ComponentId, ComponentRegistry, StorageKind};
use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::event::{ComponentLifecycleRegistry, EventRegistry, EventStorage};
use crate::resource::ResourceStore;
use crate::storage::{ArchetypeStorage, SparseStore, TypedSparseStorage};
use crate::time::{ChangeTick, ChangeTickError, WorldTick};

use self::guard::RunGuard;

pub(crate) struct WorldEvents {
    pub registry: EventRegistry,
    pub storage: EventStorage,
    pub lifecycle: ComponentLifecycleRegistry,
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
    lifecycle_events_suppressed: bool,
    execution_lease: Option<Weak<()>>,
    lease_locked_resources: Vec<TypeId>,
    fixed_step: Option<crate::time::FixedStep>,
    query_topology_revision: u64,
    query_entity_revision: u64,
    query_component_revisions: Vec<u64>,
    query_delta_changes: VecDeque<(u64, EntityId, usize)>,
    query_delta_next_sequence: u64,
    query_delta_cursors: Vec<Weak<Cell<u64>>>,
    membership_caches: Vec<crate::world::query::cache::MembershipCacheSlot>,
    result_caches: Vec<crate::world::query::result_cache::ResultCacheSlot>,
    table_archetype_cache: Vec<Option<alloc::vec::Vec<usize>>>,
    table_archetype_cache_topology: u64,
    query_resolve_scratch: RefCell<crate::world::query::plan_cache::QueryResolveScratch>,
    resolved_plan_cache: BTreeMap<u64, Rc<crate::world::query::plan::ResolvedPlan>>,
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
        let component_count = registry.len();
        let command_components = (0..registry.len())
            .map(|index| {
                (
                    registry.type_id_for_index(index),
                    registry.entry_is_tag(index),
                )
            })
            .collect();
        let owner_token = owner.token();
        Self {
            command_queue: CommandQueue::configured(owner.clone(), command_components),
            owner,
            allocator: EntityAllocator::with_owner(owner_token),
            registry,
            sparse_stores,
            archetypes,
            resources,
            events,
            change_tick: ChangeTick::ZERO,
            world_tick: WorldTick::ZERO,
            run_guard: RunGuard::Idle,
            mutation_poisoned: false,
            lifecycle_events_suppressed: false,
            execution_lease: None,
            lease_locked_resources: Vec::new(),
            fixed_step: None,
            query_topology_revision: 1,
            query_entity_revision: 1,
            query_component_revisions: alloc::vec![1; component_count],
            query_delta_changes: VecDeque::new(),
            query_delta_next_sequence: 0,
            query_delta_cursors: Vec::new(),
            membership_caches: Vec::new(),
            result_caches: Vec::new(),
            table_archetype_cache: Vec::new(),
            table_archetype_cache_topology: 0,
            query_resolve_scratch: RefCell::new(
                crate::world::query::plan_cache::QueryResolveScratch::default(),
            ),
            resolved_plan_cache: BTreeMap::new(),
        }
    }

    pub(crate) fn bump_query_topology(&mut self) {
        self.query_topology_revision = self.query_topology_revision.saturating_add(1);
        self.query_entity_revision = self.query_entity_revision.saturating_add(1);
        self.table_archetype_cache.clear();
        self.table_archetype_cache_topology = 0;
    }

    pub(crate) fn bump_component_query_topology(&mut self, component_index: usize) {
        self.query_topology_revision = self.query_topology_revision.saturating_add(1);
        if let Some(revision) = self.query_component_revisions.get_mut(component_index) {
            *revision = revision.saturating_add(1);
        }
        self.table_archetype_cache.clear();
        self.table_archetype_cache_topology = 0;
    }

    pub(crate) fn record_component_query_topology(
        &mut self,
        entity: EntityId,
        component_index: usize,
    ) {
        self.bump_component_query_topology(component_index);
        self.prune_query_delta_changes();
        self.rebase_query_delta_sequences_if_exhausted();
        let sequence = self.query_delta_next_sequence;
        self.query_delta_next_sequence += 1;
        self.query_delta_changes
            .push_back((sequence, entity, component_index));
    }

    fn rebase_query_delta_sequences_if_exhausted(&mut self) {
        if self.query_delta_next_sequence != u64::MAX {
            return;
        }

        let retained_start = self
            .query_delta_changes
            .front()
            .map_or(self.query_delta_next_sequence, |(sequence, _, _)| *sequence);
        assert!(
            retained_start != 0,
            "query delta sequence space exhausted while the entire u64 log remains live"
        );
        for (sequence, _, _) in &mut self.query_delta_changes {
            *sequence -= retained_start;
        }
        for cursor in self.query_delta_cursors.iter().filter_map(Weak::upgrade) {
            cursor.set(cursor.get().saturating_sub(retained_start));
        }
        self.query_delta_next_sequence -= retained_start;
    }

    fn prune_query_delta_changes(&mut self) {
        self.query_delta_cursors
            .retain(|cursor| cursor.strong_count() != 0);
        let retain_from = self
            .query_delta_cursors
            .iter()
            .filter_map(Weak::upgrade)
            .map(|cursor| cursor.get())
            .min()
            .unwrap_or(self.query_delta_next_sequence);
        while self
            .query_delta_changes
            .front()
            .is_some_and(|(sequence, _, _)| *sequence < retain_from)
        {
            self.query_delta_changes.pop_front();
        }
    }

    pub(crate) fn ensure_table_archetypes(&mut self, component_index: usize) -> &[usize] {
        if self.table_archetype_cache_topology != self.query_topology_revision {
            self.table_archetype_cache.clear();
            self.table_archetype_cache_topology = self.query_topology_revision;
        }
        if self.table_archetype_cache.len() <= component_index {
            self.table_archetype_cache
                .resize_with(component_index + 1, || None);
        }
        let slot = &mut self.table_archetype_cache[component_index];
        if slot.is_none() {
            *slot = Some(
                self.archetypes
                    .archetypes_with_component(component_index as u32),
            );
        }
        slot.as_deref().expect("table archetype cache initialized")
    }

    pub(crate) fn run_guard_state(&self) -> guard::RunGuard {
        self.run_guard.clone()
    }

    pub fn world_tick(&self) -> WorldTick {
        self.world_tick
    }

    pub fn commands(&mut self) -> Result<Commands<'_>, WorldError> {
        if self.run_guard.operation() == Some(crate::operation::StageOperation::Render) {
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
        self.run_guard = RunGuard::Running {
            operation,
            event_access: None,
        };
        Ok(())
    }

    pub(crate) fn begin_system_run(
        &mut self,
        operation: crate::operation::StageOperation,
        event_access: Rc<guard::EventAccess>,
    ) -> Result<(), WorldError> {
        if !self.run_guard.is_idle() {
            return Err(WorldError::NestedRun);
        }
        self.run_guard = RunGuard::Running {
            operation,
            event_access: Some(event_access),
        };
        Ok(())
    }

    pub(crate) fn end_run(&mut self) {
        self.run_guard = RunGuard::Idle;
    }

    #[cfg(test)]
    pub(crate) fn set_run_guard_running_for_test(
        &mut self,
        operation: crate::operation::StageOperation,
    ) {
        self.run_guard = RunGuard::Running {
            operation,
            event_access: None,
        };
    }

    pub(crate) fn advance_world_tick(&mut self) -> Result<(), crate::time::WorldTickError> {
        self.world_tick.advance().map(|_| ())
    }

    pub fn spawn_bundle<B: Bundle>(&mut self, bundle: B) -> Result<EntityId, WorldError> {
        let entity = self.spawn()?;
        self.lifecycle_events_suppressed = true;
        let write_result = bundle.write(&mut BundleWriter::new(self, entity));
        self.lifecycle_events_suppressed = false;

        match write_result {
            Ok(()) => {
                let components = self.component_indices_for(entity);
                for component_index in components {
                    self.emit_component_added(entity, component_index, true)?;
                }
                Ok(entity)
            }
            Err(error) => {
                self.lifecycle_events_suppressed = true;
                let rollback_result = self.remove_all_components(entity).and_then(|()| {
                    self.allocator.free(entity).map_err(|allocator_error| {
                        self.map_allocator_error(entity, allocator_error)
                    })
                });
                self.lifecycle_events_suppressed = false;
                rollback_result?;
                Err(error)
            }
        }
    }

    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.allocator.is_alive(entity)
    }

    pub(crate) fn owns_entity(&self, entity: EntityId) -> bool {
        self.allocator.owns(entity)
    }

    pub fn spawn(&mut self) -> Result<EntityId, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        let entity = self.allocator.alloc();
        self.bump_query_topology();
        Ok(entity)
    }

    pub fn despawn(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_live_access(entity)?;
        self.remove_all_components(entity)?;
        self.allocator
            .free(entity)
            .map_err(|error| self.map_allocator_error(entity, error))?;
        self.bump_query_topology();
        Ok(())
    }

    pub fn insert<T: 'static>(
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
            let index = component_id.index();
            let tick = self.issue_change_tick()?;
            let added = self.insert_tag_typed(entity, component_id, tick);
            self.emit_component_added(entity, index, added)?;
            return Ok(None);
        }
        if self.registry.is_table_component(&component_id) {
            let tick = self.issue_change_tick()?;
            let replaced =
                self.archetypes
                    .insert_table(entity, component_id.index() as u32, value, tick);
            self.emit_component_added(entity, component_id.index(), replaced.is_none())?;
            return Ok(replaced);
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
        let replaced = store.insert_with_tick(entity, value, tick);
        self.emit_component_added(entity, index, replaced.is_none())?;
        Ok(replaced)
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
        let added = self.insert_tag_index(entity, tag.index(), tick);
        self.emit_component_added(entity, tag.index(), added)?;
        Ok(added)
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
        let removed = self.remove_tag_index(entity, tag.index());
        self.emit_component_removed_if(removed, entity, tag.index())?;
        Ok(removed)
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

    pub fn get<T: 'static>(&self, entity: EntityId) -> Result<Option<&T>, WorldError> {
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

    pub fn get_mut<T: 'static>(&mut self, entity: EntityId) -> Result<Option<&mut T>, WorldError> {
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

    pub fn remove<T: 'static>(&mut self, entity: EntityId) -> Result<Option<T>, WorldError> {
        self.ensure_idle_structural()?;
        self.ensure_mutable()?;
        self.ensure_live_access(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        let index = component_id.index();
        if self.is_tag_component(&component_id) {
            let removed = self.remove_tag_index(entity, index);
            self.emit_component_removed_if(removed, entity, index)?;
            return Ok(None);
        }
        if self.registry.is_table_component(&component_id) {
            let removed = self
                .archetypes
                .remove_table(entity, component_id.index() as u32);
            self.emit_component_removed_if(removed.is_some(), entity, index)?;
            return Ok(removed);
        }
        let removed = self.sparse_store_mut::<T>(component_id)?.remove(entity);
        self.emit_component_removed_if(removed.is_some(), entity, index)?;
        Ok(removed)
    }

    pub fn len_sparse<T: 'static>(&self) -> Result<usize, WorldError> {
        let component_id = self.component_id::<T>()?;
        Ok(self.sparse_store::<T>(component_id)?.len())
    }

    pub(crate) fn owner(&self) -> &WorldOwner {
        &self.owner
    }

    pub fn change_tick(&self) -> ChangeTick {
        self.change_tick
    }

    pub(crate) fn owner_token(&self) -> WorldOwner {
        self.owner.clone()
    }

    pub(crate) fn registry_id_of<T: 'static>(&self) -> Option<ComponentId> {
        self.registry.id_of::<T>(&self.owner)
    }

    pub(crate) fn registry_id_of_type(&self, type_id: TypeId) -> Option<ComponentId> {
        self.registry
            .index_of_type(type_id)
            .map(|index| ComponentId::new(self.owner.clone(), index as u32))
    }

    pub(crate) fn registry_is_table(&self, component_id: &ComponentId) -> bool {
        self.registry.is_table_component(component_id)
    }

    pub(crate) fn registry_component_name(&self, component_id: &ComponentId) -> String {
        self.registry.component_name(component_id)
    }

    pub(crate) fn registry_contains(&self, component_id: &ComponentId) -> bool {
        self.registry.storage_kind(component_id).is_some()
    }

    pub(crate) fn entity_has_component(&self, entity: EntityId, index: usize) -> bool {
        if self.registry.entry_is_tag(index) {
            return self.entity_has_tag(entity, index);
        }
        if self.registry.entry_is_table(index) {
            return self.archetype_has_component(entity, index as u32);
        }
        self.sparse_stores
            .get(index)
            .map(|store| store.contains_entity(entity))
            .unwrap_or(false)
    }

    pub(crate) fn entity_has_tag(&self, entity: EntityId, index: usize) -> bool {
        self.sparse_stores
            .get(index)
            .map(|store| store.contains_entity(entity))
            .unwrap_or(false)
    }

    pub(crate) fn component_added_tick(
        &self,
        entity: EntityId,
        index: usize,
    ) -> Option<ChangeTick> {
        let component_id = ComponentId::new(self.owner.clone(), index as u32);
        if self.is_tag_component(&component_id) {
            return self.tag_store(index).added_tick(entity);
        }
        if self.registry.is_table_component(&component_id) {
            return self.archetypes.table_added_tick(entity, index as u32);
        }
        self.sparse_stores
            .get(index)
            .and_then(|store| store.sparse_added_tick(entity))
    }

    pub(crate) fn component_changed_tick(
        &self,
        entity: EntityId,
        index: usize,
    ) -> Option<ChangeTick> {
        let component_id = ComponentId::new(self.owner.clone(), index as u32);
        if self.is_tag_component(&component_id) {
            return self.tag_store(index).changed_tick(entity);
        }
        if self.registry.is_table_component(&component_id) {
            return self.archetypes.table_changed_tick(entity, index as u32);
        }
        self.sparse_stores
            .get(index)
            .and_then(|store| store.sparse_changed_tick(entity))
    }

    pub(crate) fn sparse_store_by_index<T: 'static>(
        &self,
        index: usize,
    ) -> Result<&TypedSparseStorage<T>, crate::query::QueryError> {
        self.sparse_stores
            .get(index)
            .and_then(|store| store.typed::<T>())
            .ok_or_else(|| crate::query::QueryError::WrongStorageKind {
                name: alloc::format!("component {index}"),
            })
    }

    fn archetype_has_component(&self, entity: EntityId, component_index: u32) -> bool {
        self.archetypes.has_component(entity, component_index)
    }

    pub(crate) fn command_queue_mut(&mut self) -> &mut CommandQueue {
        &mut self.command_queue
    }

    pub(crate) fn commit_command_ops(&mut self, tick: ChangeTick) -> Result<usize, WorldError> {
        let mut ops = self.command_queue.take_ops();
        let count = ops.len();
        let result = {
            let mut drained = ops.drain(..);
            let mut result = Ok(count);
            for op in drained.by_ref() {
                if let Err(error) = op.commit(self, tick) {
                    result = Err(error);
                    break;
                }
            }
            result
        };
        self.command_queue.restore_ops(ops);
        result
    }

    pub(crate) fn allocator_mut(&mut self) -> &mut EntityAllocator {
        &mut self.allocator
    }

    pub(crate) fn allocator_is_reserved(&self, entity: EntityId) -> bool {
        self.allocator.is_reserved(entity)
    }

    pub(crate) fn ensure_command_target(&self, entity: EntityId) -> Result<(), WorldError> {
        if !self.owns_entity(entity) {
            return Err(WorldError::EntityOwnerMismatch { entity });
        }
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
            .map_err(|error| self.map_allocator_error(entity, error))?;
        self.bump_query_topology();
        Ok(())
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
            .map_err(|error| self.map_allocator_error(entity, error))?;
        self.bump_query_topology();
        Ok(())
    }

    pub(crate) fn commit_insert_erased<T: 'static>(
        &mut self,
        entity: EntityId,
        component_index: u32,
        value: T,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let component_id = ComponentId::new(self.owner.clone(), component_index);
        let index = component_index as usize;
        let is_new = if self.is_tag_component(&component_id) {
            self.insert_tag_index(entity, index, tick)
        } else if self.registry.is_table_component(&component_id) {
            self.archetypes
                .insert_table(entity, component_index, value, tick)
                .is_none()
        } else {
            let store = self
                .sparse_stores
                .get_mut(index)
                .and_then(|store| store.typed_mut::<T>())
                .ok_or_else(|| WorldError::WrongStorageKind {
                    name: format!("component {component_index}"),
                })?;
            store.insert_with_tick(entity, value, tick).is_none()
        };
        self.emit_component_added(entity, index, is_new)?;
        Ok(())
    }

    pub(crate) fn commit_insert_tag_index(
        &mut self,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let added = self.insert_tag_index(entity, component_index as usize, tick);
        self.emit_component_added(entity, component_index as usize, added)?;
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
        let index = component_index as usize;
        let removed = if self.is_tag_component(&component_id) {
            self.remove_tag_index(entity, index)
        } else if self.registry.is_table_component(&component_id) {
            self.archetypes.remove_table_index(entity, component_index)
        } else if let Some(store) = self.sparse_stores.get_mut(index) {
            if store.contains_entity(entity) {
                store.remove_entity(entity);
                true
            } else {
                false
            }
        } else {
            false
        };
        if removed {
            self.emit_component_removed(entity, index)?;
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
        let sparse_removals = self
            .sparse_stores
            .iter()
            .enumerate()
            .filter_map(|(index, store)| store.contains_entity(entity).then_some(index))
            .collect::<Vec<_>>();
        let table_removals = self
            .archetypes
            .table_component_indices(entity)
            .into_iter()
            .map(|index| index as usize)
            .collect::<Vec<_>>();
        for index in sparse_removals {
            self.emit_component_removed(entity, index)?;
        }
        for index in table_removals {
            self.emit_component_removed(entity, index)?;
        }
        for store in &mut self.sparse_stores {
            store.remove_entity(entity);
        }
        self.archetypes.remove_entity(entity);
        Ok(())
    }

    fn component_indices_for(&self, entity: EntityId) -> Vec<usize> {
        let mut indices = self
            .sparse_stores
            .iter()
            .enumerate()
            .filter_map(|(index, store)| store.contains_entity(entity).then_some(index))
            .collect::<Vec<_>>();
        indices.extend(
            self.archetypes
                .table_component_indices(entity)
                .into_iter()
                .map(|index| index as usize),
        );
        indices
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
            let id = EntityId::from_owned_parts(self.owner.token(), slot as u32, generation);
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
        if !self.owns_entity(entity) {
            return Err(WorldError::EntityOwnerMismatch { entity });
        }
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
    pub(crate) fn set_change_tick_for_test(&mut self, tick: ChangeTick) {
        set_change_tick_for_test(self, tick);
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn set_world_tick_for_test(&mut self, raw: u64) {
        set_world_tick_for_test(self, raw);
    }
}

#[cfg(any(test, feature = "testkit"))]
pub(crate) fn set_change_tick_for_test(world: &mut World, tick: ChangeTick) {
    world.change_tick = tick;
}

#[cfg(any(test, feature = "testkit"))]
pub(crate) fn set_world_tick_for_test(world: &mut World, raw: u64) {
    world.world_tick.set_raw(raw);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::entity::AllocatorError;
    use crate::time::ChangeTick;

    #[derive(Clone, Copy)]
    struct Marker(u8);

    #[derive(Clone, Copy)]
    struct Other(#[allow(dead_code)] u8);

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
    fn resource_scope_tick_exhaustion_restores_resource_without_scope_sentinel() {
        #[derive(Debug, PartialEq)]
        struct Score(i32);

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        world.insert_resource(Score(1)).expect("seed");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        world.insert_resource(Score(2)).expect("consume last tick");

        let result = world.resource_scope_mut::<Score, _>(|_, _| ());
        assert!(matches!(result, Err(WorldError::ChangeTickExhausted)));
        assert_eq!(
            world.resource::<Score>().expect("unchanged"),
            Some(&Score(2))
        );
    }

    #[test]
    fn resource_scope_closure_mutates_before_restore() {
        #[derive(Debug, PartialEq)]
        struct Score(i32);

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        world.insert_resource(Score(1)).expect("seed");

        world
            .resource_scope_mut::<Score, _>(|value, _| {
                value.expect("score is present").0 = 5;
            })
            .expect("scope");

        assert_eq!(world.resource::<Score>().expect("get"), Some(&Score(5)));
    }

    #[test]
    fn resource_scope_ref_preserves_ticks_and_mut_scope_marks_present_only() {
        #[derive(Debug, PartialEq)]
        struct Score(i32);
        #[derive(Debug, PartialEq)]
        struct Other(i32);

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        builder.register_resource::<Other>();
        let mut world = builder.build().expect("build");
        world.insert_resource(Score(1)).expect("seed");

        world
            .resource_scope_ref::<Score, _>(|value, _| {
                assert_eq!(value, Some(&Score(1)));
            })
            .expect("ref scope");
        assert_eq!(
            world.resource_changed_tick::<Score>().expect("tick"),
            Some(ChangeTick::from_raw(1))
        );

        world
            .resource_scope_mut::<Other, _>(|value, _| assert!(value.is_none()))
            .expect("missing mut scope");
        world.insert_resource(Other(2)).expect("insert other");
        assert_eq!(
            world.resource_changed_tick::<Other>().expect("tick"),
            Some(ChangeTick::from_raw(2))
        );

        world
            .resource_scope_mut::<Score, _>(|value, _| value.expect("score").0 = 3)
            .expect("mut scope");
        assert_eq!(
            world.resource_changed_tick::<Score>().expect("tick"),
            Some(ChangeTick::from_raw(3))
        );
    }

    #[test]
    #[cfg(feature = "std")]
    fn resource_scope_ref_restores_exact_ticks_after_unwind() {
        #[derive(Debug, PartialEq)]
        struct Score(i32);

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        world.insert_resource(Score(1)).expect("seed");

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.resource_scope_ref::<Score, _>(|value, _| {
                assert_eq!(value, Some(&Score(1)));
                panic!("scope panic");
            });
        }));
        assert!(panic.is_err());
        assert_eq!(
            world.resource_added_tick::<Score>().expect("added"),
            Some(ChangeTick::from_raw(1))
        );
        assert_eq!(
            world.resource_changed_tick::<Score>().expect("changed"),
            Some(ChangeTick::from_raw(1))
        );
    }

    #[test]
    fn frame_events_clear_per_operation() {
        use crate::event::{EventOptions, EventReaderStart};
        use crate::operation::StageOperation;

        #[derive(Clone, Copy)]
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
    fn component_ticks_track_sparse_table_and_tag() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("sparse");
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("table");
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");

        let sparse_entity = world.spawn().expect("sparse");
        world
            .insert(sparse_entity, Marker(1))
            .expect("sparse insert");
        let sparse_index = world.registry_id_of::<Marker>().expect("id").index();
        let sparse_added = world
            .component_added_tick(sparse_entity, sparse_index)
            .expect("sparse added");
        world
            .get_mut::<Marker>(sparse_entity)
            .expect("mut")
            .expect("present")
            .0 = 2;
        let sparse_changed = world
            .component_changed_tick(sparse_entity, sparse_index)
            .expect("sparse changed");
        assert!(sparse_changed > sparse_added);

        let table_entity = world.spawn().expect("table");
        world
            .insert(table_entity, TableComp(3))
            .expect("table insert");
        let table_index = world.registry_id_of::<TableComp>().expect("id").index();
        assert!(world
            .component_added_tick(table_entity, table_index)
            .is_some());
        world
            .get_mut::<TableComp>(table_entity)
            .expect("mut")
            .expect("present")
            .0 = 4;
        assert!(world
            .component_changed_tick(table_entity, table_index)
            .is_some());

        let tag_entity = world.spawn().expect("tag");
        world.add_tag(tag_entity, &tag).expect("add tag");
        let tag_index = tag.index();
        assert!(world.component_added_tick(tag_entity, tag_index).is_some());
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

    #[test]
    fn get_mut_table_absent_returns_none() {
        let mut world = table_world();
        let entity = world.spawn().expect("spawn");
        assert!(world.get_mut::<TableComp>(entity).expect("mut").is_none());
    }

    #[test]
    fn remove_tag_via_remove_generic_emits_removed() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        assert!(world.remove::<Player>(entity).expect("remove").is_none());
        assert!(!world.has_tag(entity, &tag).expect("has"));
    }

    #[test]
    fn len_sparse_reports_members() {
        let mut world = test_world();
        let a = world.spawn().expect("a");
        let b = world.spawn().expect("b");
        world.insert(a, Marker(1)).expect("a");
        world.insert(b, Marker(2)).expect("b");
        assert_eq!(world.len_sparse::<Marker>().expect("len"), 2);
    }

    #[test]
    fn validate_component_insert_rejects_value_on_tag() {
        use core::any::TypeId;

        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        assert!(matches!(
            world.validate_component_insert(entity, tag.index() as u32, TypeId::of::<Marker>()),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn validate_component_insert_rejects_type_mismatch() {
        use core::any::TypeId;

        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        let marker_index = world.registry_id_of::<Marker>().expect("id").index() as u32;
        assert!(matches!(
            world.validate_component_insert(entity, marker_index, TypeId::of::<Other>()),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn ensure_command_target_rejects_stale_entity() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.despawn(entity).expect("despawn");
        assert!(matches!(
            world.ensure_command_target(entity),
            Err(WorldError::StaleEntity { .. })
        ));
    }

    #[test]
    fn generation_overflow_on_despawn_maps_allocator_error() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Marker(1)).expect("insert");
        world
            .allocator_mut()
            .set_generation_for_test(entity, u32::MAX);
        let exhausted = entity.with_generation(u32::MAX);
        assert!(matches!(
            world.despawn(exhausted),
            Err(WorldError::Allocator(
                WorldAllocatorError::GenerationOverflow
            ))
        ));
    }

    #[test]
    fn commit_reserved_spawn_maps_slot_retired() {
        let mut world = test_world();
        let entity = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world
            .allocator_mut()
            .set_generation_for_test(entity, u32::MAX);
        let exhausted = entity.with_generation(u32::MAX);
        assert_eq!(
            world.allocator_mut().release_reserved(exhausted),
            Err(AllocatorError::GenerationOverflow)
        );
        assert!(matches!(
            world.commit_reserved_spawn(exhausted, ChangeTick::from_raw(1)),
            Err(WorldError::Allocator(WorldAllocatorError::SlotRetired))
        ));
    }

    #[test]
    fn commit_despawn_releases_reserved_spawn() {
        let mut world = test_world();
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world
            .commands()
            .expect("commands")
            .despawn(reserved)
            .expect("queue");
        world.flush().expect("flush");
        assert!(!world.is_alive(reserved));
    }

    #[test]
    fn deferred_commit_paths_cover_table_sparse_and_tag() {
        use crate::world::DynamicBundle;

        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("sparse");
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("build");

        let mut bundle = DynamicBundle::new();
        bundle.push_tag(&tag).expect("tag");
        bundle.push(&world, Marker(3)).expect("sparse");
        bundle.push(&world, TableComp(8)).expect("table");
        let entity = world
            .commands()
            .expect("commands")
            .spawn_bundle(bundle)
            .expect("spawn");
        world.flush().expect("flush");

        assert!(world.has_tag(entity, &tag).expect("tag"));
        assert_eq!(
            world.get::<Marker>(entity).expect("sparse").map(|m| m.0),
            Some(3)
        );
        assert_eq!(
            world.get::<TableComp>(entity).expect("table").map(|c| c.0),
            Some(8)
        );

        world
            .commands()
            .expect("commands")
            .remove::<Marker>(entity)
            .expect("remove sparse");
        world.flush().expect("flush sparse remove");
        assert!(world.remove_tag(entity, &tag).expect("remove tag"));
        assert!(!world.has_tag(entity, &tag).expect("tag gone"));
        assert!(world.get::<Marker>(entity).expect("sparse gone").is_none());
    }

    #[test]
    fn collect_live_entities_includes_reserved_handles() {
        let mut world = test_world();
        let live = world.spawn().expect("live");
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserved");
        let mut entities = Vec::new();
        world.collect_live_entities(&mut entities);
        assert!(entities.contains(&live));
        assert!(entities.contains(&reserved));
    }

    #[test]
    fn remove_tag_via_remove_tag_emits_removed() {
        use crate::event::EventReaderStart;

        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        let mut reader = world
            .on_remove_reader::<Player>(EventReaderStart::OldestRetained)
            .expect("reader");
        assert!(world.remove_tag(entity, &tag).expect("remove"));
        assert!(!world.has_tag(entity, &tag).expect("gone"));
        assert!(world.read_event(&mut reader).expect("read").is_some());
    }

    #[test]
    fn remove_tag_propagates_emit_errors() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.remove_tag(entity, &tag),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn sparse_remove_emits_component_removed() {
        use crate::event::EventReaderStart;

        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Marker(1)).expect("insert");
        let mut reader = world
            .on_remove_reader::<Marker>(EventReaderStart::OldestRetained)
            .expect("reader");
        assert_eq!(
            world.remove::<Marker>(entity).expect("remove").map(|m| m.0),
            Some(1)
        );
        assert!(world.read_event(&mut reader).expect("read").is_some());
    }

    #[test]
    fn sparse_remove_propagates_emit_errors() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Marker(1)).expect("insert");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.remove::<Marker>(entity),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn component_changed_tick_for_tag_matches_added_on_insert() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        let tag_index = tag.index();
        let added = world
            .component_added_tick(entity, tag_index)
            .expect("tag add records added tick");
        assert_eq!(world.component_changed_tick(entity, tag_index), Some(added));

        assert!(!world.add_tag(entity, &tag).expect("re-add existing tag"));
        let changed_after_readd = world
            .component_changed_tick(entity, tag_index)
            .expect("re-add bumps changed tick");
        assert!(changed_after_readd > added);
    }

    #[test]
    fn entity_has_component_checks_tag_presence() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        assert!(world.entity_has_component(entity, tag.index()));
    }

    #[test]
    fn sparse_store_by_index_rejects_wrong_type() {
        let world = test_world();
        let index = world.registry_id_of::<Marker>().expect("id").index();
        assert!(matches!(
            world.sparse_store_by_index::<Other>(index),
            Err(crate::query::QueryError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn ensure_mutable_rejects_poisoned_world() {
        let mut world = test_world();
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Marker(1)).expect("consume");
        assert!(matches!(
            world.insert(entity, Marker(2)),
            Err(WorldError::ChangeTickExhausted)
        ));
        assert!(matches!(
            world.ensure_mutable(),
            Err(WorldError::ChangeTickExhausted)
        ));
    }

    #[test]
    fn get_mut_sparse_wrong_storage_kind() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(1)).expect("insert");
        assert!(matches!(
            world.get_mut::<Marker>(entity),
            Err(WorldError::UnregisteredComponent { .. })
        ));
    }

    #[test]
    fn despawn_emits_table_component_removed() {
        let mut world = table_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(1)).expect("insert");
        world.despawn(entity).expect("despawn");
        assert!(!world.is_alive(entity));
    }

    #[test]
    fn remove_generic_on_table_propagates_emit_errors() {
        let mut world = table_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(4)).expect("insert");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.remove::<TableComp>(entity),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn remove_generic_on_table_emits_removed() {
        use crate::event::EventReaderStart;

        let mut world = table_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(4)).expect("insert");
        let mut reader = world
            .on_remove_reader::<TableComp>(EventReaderStart::OldestRetained)
            .expect("reader");
        assert_eq!(
            world
                .remove::<TableComp>(entity)
                .expect("remove")
                .map(|c| c.0),
            Some(4)
        );
        assert!(world.read_event(&mut reader).expect("read").is_some());
    }

    #[test]
    fn ensure_sparse_kind_rejects_table_component() {
        let world = table_world();
        let id = world.registry_id_of::<TableComp>().expect("id");
        assert!(matches!(
            world.ensure_sparse_kind(&id),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn expected_type_id_rejects_unregistered_index() {
        let world = test_world();
        assert!(matches!(
            world.expected_type_id(99),
            Err(WorldError::UnregisteredComponent { .. })
        ));
    }

    #[test]
    fn commit_remove_sparse_missing_entity_is_noop() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        let index = world.registry_id_of::<Marker>().expect("id").index() as u32;
        world
            .commit_remove_index(entity, index, ChangeTick::from_raw(1))
            .expect("noop");
    }

    #[test]
    fn get_mut_tag_component_reports_wrong_storage_kind() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        assert!(matches!(
            world.get_mut::<Player>(entity),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn commit_insert_erased_wrong_sparse_type_reports_kind() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        let marker_index = world.registry_id_of::<Marker>().expect("id").index() as u32;
        assert!(matches!(
            world.commit_insert_erased(entity, marker_index, Other, ChangeTick::from_raw(1)),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn map_allocator_error_maps_stale_entity() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.despawn(entity).expect("free");
        assert!(matches!(
            world.despawn(entity),
            Err(WorldError::StaleEntity { .. })
        ));
    }

    #[test]
    fn collect_live_entities_skips_empty_slots() {
        let world = test_world();
        let mut entities = Vec::new();
        world.collect_live_entities_from_slots(&mut entities);
        assert!(entities.is_empty());
    }

    #[test]
    fn sparse_store_helpers_reject_wrong_type() {
        let mut world = test_world();
        let marker_id = world.registry_id_of::<Marker>().expect("marker");
        assert!(matches!(
            world.sparse_store::<TableComp>(marker_id.clone()),
            Err(WorldError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            world.sparse_store_mut::<TableComp>(marker_id),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn ensure_sparse_kind_rejects_unregistered_component_id() {
        let world = test_world();
        let bogus = ComponentId::new(world.owner().clone(), 99);
        assert!(matches!(
            world.ensure_sparse_kind(&bogus),
            Err(WorldError::UnregisteredComponent { .. })
        ));
    }

    #[test]
    fn commit_remove_table_component_via_index() {
        let mut world = table_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(3)).expect("insert");
        let index = world.registry_id_of::<TableComp>().expect("id").index() as u32;
        world
            .commit_remove_index(entity, index, ChangeTick::from_raw(1))
            .expect("remove");
        assert!(world.get::<TableComp>(entity).expect("get").is_none());
    }

    #[test]
    fn resource_added_tick_reports_insert_tick() {
        #[derive(Debug, PartialEq)]
        struct Score(i32);

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        world.insert_resource(Score(1)).expect("insert");
        assert!(world
            .resource_added_tick::<Score>()
            .expect("tick")
            .is_some());
    }

    #[test]
    fn corrupted_sparse_store_reports_wrong_kind_on_insert_and_get_mut() {
        use crate::storage::SparseStore;

        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        let index = world.registry_id_of::<Marker>().expect("id").index();
        world.sparse_stores[index] = SparseStore::new_tag();
        assert!(matches!(
            world.insert(entity, Marker(1)),
            Err(WorldError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            world.get_mut::<Marker>(entity),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn remove_generic_on_tag_emits_removed() {
        use crate::event::EventReaderStart;

        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        let mut reader = world
            .on_remove_reader::<Player>(EventReaderStart::OldestRetained)
            .expect("reader");
        assert!(world.remove::<Player>(entity).expect("remove").is_none());
        assert!(!world.has_tag(entity, &tag).expect("gone"));
        assert!(world.read_event(&mut reader).expect("read").is_some());
    }

    #[test]
    fn remove_generic_on_tag_propagates_emit_errors() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("add");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.remove::<Player>(entity),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn commit_despawn_on_reserved_entity_releases_slot() {
        let mut world = test_world();
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world.commit_despawn(reserved).expect("release");
        assert!(!world.is_alive(reserved));
    }

    #[test]
    fn lock_resource_blocks_remove_while_locked() {
        #[derive(Debug, PartialEq)]
        struct Score(i32);

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        world.insert_resource(Score(1)).expect("seed");
        world.lock_resource::<Score>();
        assert!(matches!(
            world.remove_resource::<Score>(),
            Err(WorldError::ResourceInUse { .. })
        ));
        world.unlock_resource::<Score>();
    }

    #[test]
    fn from_parts_rejects_world_running() {
        let mut world = test_world();
        world
            .begin_run(crate::operation::StageOperation::Update)
            .expect("run");
        let mut idle = test_world();
        let schedule = crate::schedule::ScheduleBuilder::standard()
            .build(&mut idle)
            .expect("schedule");
        assert!(matches!(
            crate::app::App::from_parts(world, schedule),
            Err(crate::schedule::BuildError::WorldRunning)
        ));
    }

    #[test]
    fn commit_insert_erased_tag_and_remove_tag_via_index() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        let index = tag.index() as u32;
        let tick = ChangeTick::from_raw(1);
        world
            .commit_insert_erased::<()>(entity, index, (), tick)
            .expect("insert tag");
        assert!(world.has_tag(entity, &tag).expect("tagged"));
        world
            .commit_remove_index(entity, index, tick)
            .expect("remove tag");
        assert!(!world.has_tag(entity, &tag).expect("removed"));
    }

    #[test]
    fn commit_remove_index_without_store_is_noop() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world
            .commit_remove_index(entity, 99, ChangeTick::from_raw(1))
            .expect("noop");
    }

    #[test]
    fn collect_live_entities_skips_freed_slots_with_zero_generation() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.despawn(entity).expect("despawn");
        let mut entities = Vec::new();
        world.collect_live_entities_from_slots(&mut entities);
        assert!(!entities.contains(&entity));
    }

    #[test]
    fn map_allocator_error_maps_not_live_to_stale_entity() {
        let mut world = test_world();
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        assert!(matches!(
            world.despawn(reserved),
            Err(WorldError::EntityNotLive { .. })
        ));
    }

    struct FailingBundle;

    impl Bundle for FailingBundle {
        fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError> {
            let entity = writer.test_entity();
            writer
                .test_world()
                .allocator_mut()
                .set_generation_for_test(entity, entity.generation().wrapping_add(1));
            Err(WorldError::WrongStorageKind {
                name: String::from("bundle write failed"),
            })
        }
    }

    #[test]
    fn spawn_bundle_rollback_maps_allocator_error_on_stale_free() {
        let mut world = test_world();
        assert!(matches!(
            world.spawn_bundle(FailingBundle),
            Err(WorldError::StaleEntity { .. })
        ));
    }

    #[test]
    fn collect_live_entities_skips_zero_generation_slots() {
        let mut world = test_world();
        let entity = world.spawn().expect("spawn");
        world.allocator_mut().set_generation_for_test(entity, 0);
        let mut entities = Vec::new();
        world.collect_live_entities_from_slots(&mut entities);
        assert!(!entities.contains(&entity));
    }

    #[test]
    fn fixed_step_round_trips_through_world_accessor() {
        use core::time::Duration;

        use crate::time::FixedStep;

        let mut world = test_world();
        assert!(world.fixed_step().is_none());
        let step = FixedStep {
            index: 0,
            delta: Duration::from_millis(16),
        };
        world.set_fixed_step(Some(step));
        assert_eq!(world.fixed_step(), Some(step));
    }

    #[test]
    fn command_target_rejects_foreign_entities() {
        let first = test_world();
        let mut second = test_world();
        let foreign = second.spawn().expect("foreign");
        assert!(matches!(
            first.ensure_command_target(foreign),
            Err(WorldError::EntityOwnerMismatch { .. })
        ));
    }

    #[test]
    fn world_tick_test_setter_updates_raw_tick() {
        let mut world = test_world();
        world.set_world_tick_for_test(17);
        assert_eq!(world.world_tick().raw(), 17);
    }

    #[test]
    fn component_topology_bump_updates_registered_revision() {
        let mut builder = WorldBuilder::new();
        let component = builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("component");
        let mut world = builder.build().expect("world");
        let before = world.query_component_revisions[component.index()];
        world.bump_component_query_topology(component.index());
        assert_eq!(
            world.query_component_revisions[component.index()],
            before + 1
        );
        world.bump_component_query_topology(usize::MAX);
    }
}
