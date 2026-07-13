use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::time::ChangeTick;
use crate::world::{Bundle, BundleWriter, FlushError, World, WorldError};

pub(crate) struct CommandQueue {
    ops: Vec<CommandOp>,
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
        &self,
        world: &mut World,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError>;
}

impl<T: Clone + 'static> ErasedComponentValue for T {
    fn apply_insert(
        &self,
        world: &mut World,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        world.commit_insert_erased(entity, component_index, self.clone(), tick)
    }
}

impl CommandQueue {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn push(&mut self, op: CommandOp) {
        self.ops.push(op);
    }

    pub fn discard(&mut self, allocator: &mut EntityAllocator) -> Result<(), WorldError> {
        for entity in self.reserved_entities() {
            allocator
                .release_reserved(entity)
                .map_err(|error| map_allocator_error(entity, error))?;
        }
        self.ops.clear();
        Ok(())
    }

    pub fn reserved_entities(&self) -> Vec<EntityId> {
        let mut reserved = Vec::new();
        for op in &self.ops {
            if let CommandOp::SpawnReserved { entity } = op {
                reserved.push(*entity);
            }
        }
        reserved
    }

    pub fn preflight(&self, world: &World) -> Result<(), FlushError> {
        let mut live = LiveSet::from_world(world);
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

    pub fn truncate(&mut self, len: usize) {
        self.ops.truncate(len);
    }

    pub fn len(&self) -> usize {
        self.ops.len()
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

struct LiveSet {
    entities: Vec<EntityId>,
}

impl LiveSet {
    fn from_world(world: &World) -> Self {
        let mut entities = Vec::new();
        world.collect_live_entities(&mut entities);
        Self { entities }
    }

    fn contains(&self, entity: EntityId) -> bool {
        self.entities.contains(&entity)
    }

    fn insert(&mut self, entity: EntityId) {
        if !self.contains(entity) {
            self.entities.push(entity);
        }
    }

    fn remove(&mut self, entity: EntityId) {
        if let Some(index) = self
            .entities
            .iter()
            .position(|candidate| *candidate == entity)
        {
            self.entities.swap_remove(index);
        }
    }
}

/// Borrowed deferred structural mutation facade.
pub struct Commands<'w> {
    world: &'w mut World,
}

impl<'w> Commands<'w> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        Self { world }
    }

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

    pub fn despawn(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        self.world
            .command_queue_mut()
            .push(CommandOp::Despawn { entity });
        Ok(())
    }

    pub fn insert<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
        value: T,
    ) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        let component_index = self.world.component_index::<T>()? as u32;
        self.world.command_queue_mut().push(CommandOp::Insert {
            entity,
            component_index,
            value: Box::new(value),
        });
        Ok(())
    }

    pub fn remove<T: Clone + 'static>(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.world.ensure_mutable()?;
        self.world.ensure_command_target(entity)?;
        let component_index = self.world.component_index::<T>()? as u32;
        self.world.command_queue_mut().push(CommandOp::Remove {
            entity,
            component_index,
        });
        Ok(())
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

    #[derive(Clone, Copy)]
    struct Health(#[allow(dead_code)] i32);

    fn world_with_health() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("health");
        builder.build().expect("build")
    }

    #[test]
    fn preflight_rejects_invalid_spawn_insert_and_remove_targets() {
        let mut world = world_with_health();
        let live = world.spawn().expect("live");
        let stale = EntityId::from_parts(99, 1);
        let mut queue = CommandQueue::new();
        queue.push(CommandOp::SpawnReserved { entity: live });
        assert!(matches!(
            queue.preflight(&world),
            Err(FlushError::CommandValidation { .. })
        ));

        queue = CommandQueue::new();
        queue.push(CommandOp::Insert {
            entity: stale,
            component_index: 0,
            value: Box::new(Health(1)),
        });
        assert!(matches!(
            queue.preflight(&world),
            Err(FlushError::CommandValidation { .. })
        ));

        queue = CommandQueue::new();
        queue.push(CommandOp::InsertTag {
            entity: stale,
            component_index: 0,
        });
        assert!(matches!(
            queue.preflight(&world),
            Err(FlushError::CommandValidation { .. })
        ));

        queue = CommandQueue::new();
        queue.push(CommandOp::Remove {
            entity: stale,
            component_index: 0,
        });
        assert!(matches!(
            queue.preflight(&world),
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
        queue.preflight(&world).expect("valid batch");
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
        let mut live = LiveSet {
            entities: alloc::vec::Vec::new(),
        };
        let entity = EntityId::from_parts(4, 1);
        live.insert(entity);
        assert!(live.contains(entity));
    }
}
