//! Typed World access helpers.

use alloc::collections::BTreeMap;
use core::fmt;

use crate::entity::EntityId;

use super::World;

/// Failure while accessing transient entity-keyed scratch storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EntityScratchError {
    /// The supplied world is not the world that created the scratch storage.
    WrongWorld,
    /// The entity generation is no longer live in the bound world.
    StaleEntity { entity: EntityId },
    /// The entity belongs to the bound world but is not live yet.
    EntityNotLive { entity: EntityId },
}

impl fmt::Display for EntityScratchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongWorld => f.write_str("entity scratch used with the wrong world"),
            Self::StaleEntity { entity } => write!(f, "stale entity {entity:?}"),
            Self::EntityNotLive { entity } => write!(f, "entity {entity:?} is not live"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EntityScratchError {}

/// Transient, world-bound storage keyed by full generational entity handles.
///
/// Scratch storage is intended to be captured by a system or kept as other local
/// host state. It is not attached to [`World`] and deliberately exposes no raw
/// entity-slot or persistence representation.
pub struct EntityScratch<V> {
    owner: u32,
    values: BTreeMap<EntityId, V>,
}

impl<V> EntityScratch<V> {
    /// Creates empty scratch storage bound to `world`.
    pub fn new(world: &World) -> Self {
        Self {
            owner: world.owner.token(),
            values: BTreeMap::new(),
        }
    }

    /// Inserts a value for a live entity, returning the value it replaced.
    pub fn insert(
        &mut self,
        world: &World,
        entity: EntityId,
        value: V,
    ) -> Result<Option<V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        Ok(self.values.insert(entity, value))
    }

    /// Gets a shared reference for a live entity.
    pub fn get<'a>(
        &'a self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<&'a V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        Ok(self.values.get(&entity))
    }

    /// Gets a mutable reference for a live entity.
    pub fn get_mut<'a>(
        &'a mut self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<&'a mut V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        Ok(self.values.get_mut(&entity))
    }

    /// Removes and returns the value for a live entity.
    pub fn remove(
        &mut self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        Ok(self.values.remove(&entity))
    }

    /// Removes entries whose recorded entity generation is no longer live.
    pub fn retain_live(&mut self, world: &World) -> Result<usize, EntityScratchError> {
        self.validate_world(world)?;
        let before = self.values.len();
        self.values.retain(|entity, _| world.is_alive(*entity));
        Ok(before - self.values.len())
    }

    /// Removes all scratch values.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Returns the number of stored values, including entries that have become stale.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether no scratch values are stored.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn validate_entity(&self, world: &World, entity: EntityId) -> Result<(), EntityScratchError> {
        self.validate_world(world)?;
        if world.allocator.is_alive(entity) {
            Ok(())
        } else if world.allocator.is_reserved(entity) {
            Err(EntityScratchError::EntityNotLive { entity })
        } else {
            Err(EntityScratchError::StaleEntity { entity })
        }
    }

    fn validate_world(&self, world: &World) -> Result<(), EntityScratchError> {
        if world.owner.token() == self.owner {
            Ok(())
        } else {
            Err(EntityScratchError::WrongWorld)
        }
    }
}
