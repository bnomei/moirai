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
    SpawnReserved { entity: EntityId },
    Despawn { entity: EntityId },
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
        if let Some(index) = self.entities.iter().position(|candidate| *candidate == entity) {
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
        let entity = self.spawn()?;
        bundle.write(&mut BundleWriter::deferred(self.world, entity))?;
        Ok(entity)
    }

    pub fn despawn(&mut self, entity: EntityId) {
        self.world
            .command_queue_mut()
            .push(CommandOp::Despawn { entity });
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