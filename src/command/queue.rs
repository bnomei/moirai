//! Deferred command queue, preflight validation, and the [`Commands`] facade.
//!
//! Structural changes enqueue as [`CommandOp`] variants and commit during schedule flush after
//! batch validation against live and reserved entity state.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::time::ChangeTick;
use crate::world::{Bundle, BundleWriter, FlushError, World, WorldError, WorldOwner};

const MAX_RETAINED_COMMAND_BYTES: usize = 256 * 1024;

pub(crate) struct CommandQueue {
    ops: Vec<CommandOp>,
    owner: Option<WorldOwner>,
    components: Vec<(Option<TypeId>, bool)>,
    preflight_scratch: PreflightScratch,
}

pub(crate) enum CommandOp {
    SpawnReserved {
        entity: EntityId,
    },
    Despawn {
        entity: EntityId,
    },
    Insert {
        entity: EntityId,
        component_index: u32,
        value: Box<dyn ErasedComponentValue>,
    },
    InsertTag {
        entity: EntityId,
        component_index: u32,
    },
    Remove {
        entity: EntityId,
        component_index: u32,
    },
}

pub(crate) trait ErasedComponentValue: Any {
    fn apply_insert(
        self: Box<Self>,
        world: &mut World,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError>;
}

impl<T: 'static> ErasedComponentValue for T {
    fn apply_insert(
        self: Box<Self>,
        world: &mut World,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        world.commit_insert_erased(entity, component_index, *self, tick)
    }
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            owner: None,
            components: Vec::new(),
            preflight_scratch: PreflightScratch::default(),
        }
    }

    pub(crate) fn configured(owner: WorldOwner, components: Vec<(Option<TypeId>, bool)>) -> Self {
        Self {
            ops: Vec::new(),
            owner: Some(owner),
            components,
            preflight_scratch: PreflightScratch::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn push(&mut self, op: CommandOp) {
        self.ops.push(op);
    }

    pub(crate) fn owner(&self) -> &WorldOwner {
        self.owner
            .as_ref()
            .expect("world command queue is configured with an owner")
    }

    pub(crate) fn is_tag_component(&self, component_index: usize) -> bool {
        self.components
            .get(component_index)
            .map(|(_, is_tag)| *is_tag)
            .unwrap_or(false)
    }

    pub(crate) fn enqueue_insert<T: 'static>(
        &mut self,
        entity: EntityId,
        value: T,
    ) -> Result<(), WorldError> {
        let component_index = self
            .components
            .iter()
            .position(|(type_id, _)| *type_id == Some(TypeId::of::<T>()))
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: String::from(core::any::type_name::<T>()),
            })?;
        if self.components[component_index].1 {
            self.ops.push(CommandOp::InsertTag {
                entity,
                component_index: component_index as u32,
            });
        } else {
            self.ops.push(CommandOp::Insert {
                entity,
                component_index: component_index as u32,
                value: Box::new(value),
            });
        }
        Ok(())
    }

    pub(crate) fn enqueue_dynamic_insert(
        &mut self,
        entity: EntityId,
        component_index: usize,
        value: Box<dyn ErasedComponentValue>,
    ) -> Result<(), WorldError> {
        let (expected_type, is_tag) = self.components.get(component_index).ok_or_else(|| {
            WorldError::UnregisteredComponent {
                name: alloc::format!("component {component_index}"),
            }
        })?;
        if *is_tag || *expected_type != Some(value.as_ref().type_id()) {
            return Err(WorldError::WrongStorageKind {
                name: alloc::format!("component {component_index}"),
            });
        }
        self.ops.push(CommandOp::Insert {
            entity,
            component_index: component_index as u32,
            value,
        });
        Ok(())
    }

    pub(crate) fn enqueue_tag(
        &mut self,
        entity: EntityId,
        component_index: usize,
    ) -> Result<(), WorldError> {
        if !self.is_tag_component(component_index) {
            return Err(WorldError::WrongStorageKind {
                name: alloc::format!("component {component_index}"),
            });
        }
        self.ops.push(CommandOp::InsertTag {
            entity,
            component_index: component_index as u32,
        });
        Ok(())
    }

    pub(crate) fn enqueue_remove<T: 'static>(
        &mut self,
        entity: EntityId,
    ) -> Result<(), WorldError> {
        let component_index = self
            .components
            .iter()
            .position(|(type_id, _)| *type_id == Some(TypeId::of::<T>()))
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: String::from(core::any::type_name::<T>()),
            })?;
        self.ops.push(CommandOp::Remove {
            entity,
            component_index: component_index as u32,
        });
        Ok(())
    }

    pub fn discard(&mut self, allocator: &mut EntityAllocator) -> Result<(), WorldError> {
        for op in &self.ops {
            if let CommandOp::SpawnReserved { entity } = op {
                allocator
                    .release_reserved(*entity)
                    .map_err(|error| map_allocator_error(*entity, error))?;
            }
        }
        self.ops.clear();
        self.trim_empty_scratch_to_budget();
        Ok(())
    }

    pub(crate) fn take_preflight_scratch(&mut self) -> PreflightScratch {
        core::mem::take(&mut self.preflight_scratch)
    }

    pub(crate) fn restore_preflight_scratch(&mut self, mut scratch: PreflightScratch) {
        scratch.reset();
        if scratch.retained_bytes() <= MAX_RETAINED_COMMAND_BYTES {
            self.preflight_scratch = scratch;
        }
    }

    pub fn preflight(
        &self,
        world: &World,
        scratch: &mut PreflightScratch,
    ) -> Result<(), FlushError> {
        let mut live = LiveSet::new(world, scratch);
        for (index, op) in self.ops.iter().enumerate() {
            if let Err(detail) = op.preflight(&mut live, world) {
                return Err(FlushError::CommandValidation { index, detail });
            }
        }
        Ok(())
    }

    pub fn take_ops(&mut self) -> Vec<CommandOp> {
        core::mem::take(&mut self.ops)
    }

    pub fn restore_ops(&mut self, ops: Vec<CommandOp>) {
        debug_assert!(ops.is_empty());
        self.ops = ops;
        self.trim_empty_scratch_to_budget();
    }

    pub fn truncate(&mut self, len: usize) {
        self.ops.truncate(len);
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    fn trim_empty_scratch_to_budget(&mut self) {
        if self.retained_scratch_bytes() > MAX_RETAINED_COMMAND_BYTES {
            self.ops = Vec::new();
            self.preflight_scratch = PreflightScratch::default();
        }
    }

    fn retained_scratch_bytes(&self) -> usize {
        self.ops
            .capacity()
            .saturating_mul(core::mem::size_of::<CommandOp>())
            .saturating_add(self.preflight_scratch.retained_bytes())
    }

    #[cfg(test)]
    pub(crate) fn retained_scratch_bytes_for_test(&self) -> usize {
        self.retained_scratch_bytes()
    }
}

impl CommandOp {
    fn preflight(&self, live: &mut LiveSet, world: &World) -> Result<(), String> {
        match self {
            Self::SpawnReserved { entity } => {
                if !world.allocator_is_reserved(*entity) {
                    return Err(String::from("spawn target is not reserved"));
                }
                live.insert(*entity);
                Ok(())
            }
            Self::Despawn { entity } => {
                if !live.contains(*entity) {
                    return Err(String::from("despawn target is not live in batch"));
                }
                live.remove(*entity);
                Ok(())
            }
            Self::Insert {
                entity,
                component_index,
                value,
            } => {
                if !live.contains(*entity) {
                    return Err(String::from("insert target is not live in batch"));
                }
                world
                    .validate_component_insert(*entity, *component_index, value.as_ref().type_id())
                    .map_err(format_world_error)
            }
            Self::InsertTag {
                entity,
                component_index,
            } => {
                if !live.contains(*entity) {
                    return Err(String::from("insert target is not live in batch"));
                }
                world
                    .validate_component_insert(*entity, *component_index, TypeId::of::<()>())
                    .map_err(format_world_error)
            }
            Self::Remove {
                entity,
                component_index,
            } => {
                if !live.contains(*entity) {
                    return Err(String::from("remove target is not live in batch"));
                }
                world
                    .validate_component_remove(*entity, *component_index)
                    .map_err(format_world_error)
            }
        }
    }

    pub(crate) fn commit(self, world: &mut World, tick: ChangeTick) -> Result<(), WorldError> {
        match self {
            Self::SpawnReserved { entity } => world.commit_reserved_spawn(entity, tick),
            Self::Despawn { entity } => world.commit_despawn(entity),
            Self::Insert {
                entity,
                component_index,
                value,
            } => value.apply_insert(world, entity, component_index, tick),
            Self::InsertTag {
                entity,
                component_index,
            } => world.commit_insert_tag_index(entity, component_index, tick),
            Self::Remove {
                entity,
                component_index,
            } => world.commit_remove_index(entity, component_index, tick),
        }
    }
}

#[derive(Default)]
pub(crate) struct PreflightScratch {
    transitions: Vec<u64>,
    touched_slots: Vec<usize>,
}

impl PreflightScratch {
    fn reset(&mut self) {
        for slot in self.touched_slots.drain(..) {
            self.transitions[slot] = 0;
        }
    }

    fn retained_bytes(&self) -> usize {
        self.transitions
            .capacity()
            .saturating_mul(core::mem::size_of::<u64>())
            .saturating_add(
                self.touched_slots
                    .capacity()
                    .saturating_mul(core::mem::size_of::<usize>()),
            )
    }
}

struct LiveSet<'a> {
    world: &'a World,
    scratch: &'a mut PreflightScratch,
}

impl<'a> LiveSet<'a> {
    fn new(world: &'a World, scratch: &'a mut PreflightScratch) -> Self {
        debug_assert!(scratch.touched_slots.is_empty());
        Self { world, scratch }
    }

    fn contains(&self, entity: EntityId) -> bool {
        if !self.world.owns_entity(entity) {
            return false;
        }
        let transition = self
            .scratch
            .transitions
            .get(entity.slot() as usize)
            .copied()
            .unwrap_or(0);
        if transition >> 1 == u64::from(entity.generation()) {
            transition & 1 != 0
        } else {
            self.world.is_alive(entity) || self.world.allocator_is_reserved(entity)
        }
    }

    fn insert(&mut self, entity: EntityId) {
        self.set(entity, true);
    }

    fn remove(&mut self, entity: EntityId) {
        self.set(entity, false);
    }

    fn set(&mut self, entity: EntityId, live: bool) {
        let slot = entity.slot() as usize;
        if self.scratch.transitions.len() <= slot {
            self.scratch.transitions.resize(slot + 1, 0);
        }
        if self.scratch.transitions[slot] == 0 {
            self.scratch.touched_slots.push(slot);
        }
        self.scratch.transitions[slot] = (u64::from(entity.generation()) << 1) | u64::from(live);
    }
}

/// Borrowed deferred structural mutation facade for one active world run.
///
/// Spawn reserves an entity immediately; commit happens at the next flush boundary.
pub struct Commands<'w> {
    world: &'w mut World,
}

impl<'w> Commands<'w> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        Self { world }
    }

    /// Reserves a new entity and queues spawn commit at the next flush.
    pub fn spawn(&mut self) -> Result<EntityId, WorldError> {
        self.world.ensure_mutable()?;
        let entity = self
            .world
            .allocator_mut()
            .reserve()
            .map_err(|error| map_allocator_error(EntityId::from_parts(0, 0), error))?;
        self.world
            .command_queue_mut()
            .push(CommandOp::SpawnReserved { entity });
        Ok(entity)
    }

    /// Spawns one entity and queues bundle writes; rolls back reservation on failure.
    pub fn spawn_bundle<B: Bundle>(&mut self, bundle: B) -> Result<EntityId, WorldError> {
        let queue_len = self.world.command_queue_mut().len();
        let entity = self.spawn()?;
        match bundle.write(&mut BundleWriter::deferred(self.world, entity)) {
            Ok(()) => Ok(entity),
            Err(error) => {
                self.rollback_deferred_spawn(entity, queue_len)?;
                Err(error)
            }
        }
    }

    fn rollback_deferred_spawn(
        &mut self,
        entity: EntityId,
        queue_len: usize,
    ) -> Result<(), WorldError> {
        self.world.command_queue_mut().truncate(queue_len);
        self.world
            .allocator_mut()
            .release_reserved(entity)
            .map_err(|allocator_error| map_allocator_error(entity, allocator_error))?;
        Ok(())
    }

    /// Queues despawn for a live or reserved entity target.
    pub fn despawn(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        self.world
            .command_queue_mut()
            .push(CommandOp::Despawn { entity });
        Ok(())
    }

    /// Queues component insertion for a live or reserved entity target.
    pub fn insert<T: 'static>(&mut self, entity: EntityId, value: T) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        self.world.command_queue_mut().enqueue_insert(entity, value)
    }

    /// Queues component removal for a live or reserved entity target.
    pub fn remove<T: 'static>(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        self.world.command_queue_mut().enqueue_remove::<T>(entity)
    }

    /// Queues bundle writes for an existing live or reserved entity target.
    pub fn insert_bundle<B: Bundle>(
        &mut self,
        entity: EntityId,
        bundle: B,
    ) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        let queue_len = self.world.command_queue_mut().len();
        match bundle.write(&mut BundleWriter::deferred(self.world, entity)) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.world.command_queue_mut().truncate(queue_len);
                Err(error)
            }
        }
    }
}

fn format_world_error(error: WorldError) -> String {
    alloc::format!("{error:?}")
}

fn map_allocator_error(entity: EntityId, error: AllocatorError) -> WorldError {
    match error {
        AllocatorError::GenerationOverflow => {
            WorldError::Allocator(crate::world::WorldAllocatorError::GenerationOverflow)
        }
        AllocatorError::SlotRetired => {
            WorldError::Allocator(crate::world::WorldAllocatorError::SlotRetired)
        }
        AllocatorError::StaleEntity | AllocatorError::DoubleFree | AllocatorError::NotLive => {
            WorldError::StaleEntity { entity }
        }
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::world::WorldBuilder;
    use alloc::vec;

    #[derive(Clone, Copy)]
    struct Health(#[allow(dead_code)] i32);

    struct Marker;

    fn world_with_health() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("health");
        builder.build().expect("build")
    }

    fn preflight(queue: &CommandQueue, world: &World) -> Result<(), FlushError> {
        let mut scratch = PreflightScratch::default();
        queue.preflight(world, &mut scratch)
    }

    #[test]
    fn preflight_rejects_invalid_spawn_insert_and_remove_targets() {
        let mut world = world_with_health();
        let live = world.spawn().expect("live");
        let stale = EntityId::from_parts(99, 1);
        let mut queue = CommandQueue::new();
        queue.push(CommandOp::SpawnReserved { entity: live });
        assert!(matches!(
            preflight(&queue, &world),
            Err(FlushError::CommandValidation { .. })
        ));

        queue = CommandQueue::new();
        queue.push(CommandOp::Insert {
            entity: stale,
            component_index: 0,
            value: Box::new(Health(1)),
        });
        assert!(matches!(
            preflight(&queue, &world),
            Err(FlushError::CommandValidation { .. })
        ));

        queue = CommandQueue::new();
        queue.push(CommandOp::InsertTag {
            entity: stale,
            component_index: 0,
        });
        assert!(matches!(
            preflight(&queue, &world),
            Err(FlushError::CommandValidation { .. })
        ));

        queue = CommandQueue::new();
        queue.push(CommandOp::Remove {
            entity: stale,
            component_index: 0,
        });
        assert!(matches!(
            preflight(&queue, &world),
            Err(FlushError::CommandValidation { .. })
        ));
    }

    #[test]
    fn preflight_dedupes_live_set_for_duplicate_reserved_spawns() {
        let mut world = WorldBuilder::new().build().expect("world");
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        let mut queue = CommandQueue::new();
        queue.push(CommandOp::SpawnReserved { entity: reserved });
        queue.push(CommandOp::SpawnReserved { entity: reserved });
        preflight(&queue, &world).expect("valid batch");
    }

    #[test]
    fn preflight_overlay_preserves_order_and_generation_checks() {
        let mut world = world_with_health();
        let live = world.spawn().expect("live");
        let mut queue = CommandQueue::new();
        queue.push(CommandOp::Despawn { entity: live });
        queue.push(CommandOp::Insert {
            entity: live,
            component_index: 0,
            value: Box::new(Health(1)),
        });
        assert_eq!(
            preflight(&queue, &world),
            Err(FlushError::CommandValidation {
                index: 1,
                detail: String::from("insert target is not live in batch"),
            })
        );

        let stale = live.with_generation(live.generation() + 1);
        let mut stale_queue = CommandQueue::new();
        stale_queue.push(CommandOp::Despawn { entity: stale });
        assert_eq!(
            preflight(&stale_queue, &world),
            Err(FlushError::CommandValidation {
                index: 0,
                detail: String::from("despawn target is not live in batch"),
            })
        );
    }

    #[test]
    fn helper_errors_and_default_queue() {
        let entity = EntityId::from_parts(1, 1);
        assert!(matches!(
            map_allocator_error(entity, AllocatorError::StaleEntity),
            WorldError::StaleEntity { .. }
        ));
        assert!(matches!(
            map_allocator_error(entity, AllocatorError::GenerationOverflow),
            WorldError::Allocator(crate::world::WorldAllocatorError::GenerationOverflow)
        ));
        assert!(matches!(
            map_allocator_error(entity, AllocatorError::SlotRetired),
            WorldError::Allocator(crate::world::WorldAllocatorError::SlotRetired)
        ));
        assert!(!format_world_error(WorldError::StaleEntity { entity }).is_empty());
        assert!(CommandQueue::default().is_empty());
    }

    #[test]
    fn live_set_push_tracks_entities_not_seen_yet() {
        let mut world = WorldBuilder::new().build().expect("world");
        let entity = world.spawn().expect("entity");
        world.despawn(entity).expect("despawn");
        let mut scratch = PreflightScratch::default();
        let mut live = LiveSet::new(&world, &mut scratch);
        live.insert(entity);
        assert!(live.contains(entity));
    }

    #[test]
    fn configured_enqueue_helpers_cover_tags_values_and_errors() {
        let owner = WorldOwner::new();
        let entity = EntityId::from_parts(1, 1);
        let mut queue = CommandQueue::configured(
            owner.clone(),
            vec![(Some(TypeId::of::<Health>()), false), (None, true)],
        );

        assert!(queue.owner().same(&owner));
        queue
            .enqueue_insert(entity, Health(1))
            .expect("typed value");
        queue
            .enqueue_dynamic_insert(entity, 0, Box::new(Health(2)))
            .expect("dynamic value");
        queue.enqueue_tag(entity, 1).expect("tag");
        queue.enqueue_remove::<Health>(entity).expect("remove");
        assert_eq!(queue.ops.len(), 4);

        assert!(matches!(
            queue.enqueue_insert(entity, 3_u32),
            Err(WorldError::UnregisteredComponent { .. })
        ));
        assert!(matches!(
            queue.enqueue_dynamic_insert(entity, 99, Box::new(Health(3))),
            Err(WorldError::UnregisteredComponent { .. })
        ));
        assert!(matches!(
            queue.enqueue_dynamic_insert(entity, 1, Box::new(Health(4))),
            Err(WorldError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            queue.enqueue_dynamic_insert(entity, 0, Box::new(3_u32)),
            Err(WorldError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            queue.enqueue_tag(entity, 0),
            Err(WorldError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            queue.enqueue_remove::<u32>(entity),
            Err(WorldError::UnregisteredComponent { .. })
        ));

        let mut typed_tag =
            CommandQueue::configured(owner, vec![(Some(TypeId::of::<Marker>()), true)]);
        typed_tag
            .enqueue_insert(entity, Marker)
            .expect("typed tag insert");
        assert!(matches!(typed_tag.ops[0], CommandOp::InsertTag { .. }));

        let scratch = typed_tag.take_preflight_scratch();
        typed_tag.restore_preflight_scratch(scratch);
        assert_eq!(typed_tag.preflight_scratch.retained_bytes(), 0);

        let mut oversized = PreflightScratch::default();
        oversized.transitions.reserve(MAX_RETAINED_COMMAND_BYTES);
        typed_tag.restore_preflight_scratch(oversized);
        assert_eq!(typed_tag.preflight_scratch.retained_bytes(), 0);
    }
}
