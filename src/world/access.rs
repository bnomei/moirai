//! Typed World access helpers.

use alloc::collections::TryReserveError;
use alloc::vec::Vec;
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

struct DenseSlot<V> {
    generation: u32,
    value: Option<V>,
    active_index: usize,
}

impl<V> DenseSlot<V> {
    const INACTIVE: usize = usize::MAX;

    const fn vacant() -> Self {
        Self {
            generation: 0,
            value: None,
            active_index: Self::INACTIVE,
        }
    }
}

/// Dense transient, world-bound storage keyed by full generational entity handles.
///
/// Values are addressed directly by entity slot. An active-slot list keeps clearing
/// and liveness retention proportional to the number of stored values rather than
/// the highest entity slot that has been observed.
pub struct DenseEntityScratch<V> {
    owner: u32,
    slots: Vec<DenseSlot<V>>,
    active: Vec<u32>,
}

impl<V> DenseEntityScratch<V> {
    /// Creates empty scratch storage bound to `world`.
    pub fn new(world: &World) -> Self {
        Self {
            owner: world.owner.token(),
            slots: Vec::new(),
            active: Vec::new(),
        }
    }

    /// Creates empty scratch storage with capacity for at least `capacity` values.
    pub fn with_capacity(world: &World, capacity: usize) -> Self {
        Self {
            owner: world.owner.token(),
            slots: Vec::with_capacity(capacity),
            active: Vec::with_capacity(capacity),
        }
    }

    /// Reserves capacity for at least `additional` more dense slots and values.
    pub fn reserve(&mut self, additional: usize) {
        self.slots.reserve(additional);
        self.active.reserve(additional);
    }

    /// Tries to reserve capacity for at least `additional` more dense slots and values.
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.slots.try_reserve(additional)?;
        self.active.try_reserve(additional)
    }

    /// Returns the value capacity available without growing both internal vectors.
    pub fn capacity(&self) -> usize {
        self.slots.capacity().min(self.active.capacity())
    }

    /// Inserts a value for a live entity, returning the value it replaced.
    pub fn insert(
        &mut self,
        world: &World,
        entity: EntityId,
        value: V,
    ) -> Result<Option<V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        let slot_index = entity.slot() as usize;
        self.ensure_slot(slot_index);
        let slot = &mut self.slots[slot_index];

        if slot.value.is_none() {
            slot.generation = entity.generation();
            slot.value = Some(value);
            slot.active_index = self.active.len();
            self.active.push(entity.slot());
            return Ok(None);
        }

        if slot.generation == entity.generation() {
            return Ok(slot.value.replace(value));
        }

        // The occupied slot belongs to a stale generation. Assigning the new
        // value preserves the one active-list entry for this entity slot. The
        // stale value is dropped only after the slot records a consistent new
        // generation/value pair, including when its destructor unwinds.
        let stale = slot.value.replace(value);
        slot.generation = entity.generation();
        drop(stale);
        Ok(None)
    }

    /// Gets a shared reference for a live entity.
    pub fn get<'a>(
        &'a self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<&'a V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        Ok(self.slot_value(entity))
    }

    /// Gets a mutable reference for a live entity.
    pub fn get_mut<'a>(
        &'a mut self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<&'a mut V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        Ok(self.slot_value_mut(entity))
    }

    /// Gets the existing value or inserts one produced by `insert`.
    pub fn get_or_insert_with<'a>(
        &'a mut self,
        world: &World,
        entity: EntityId,
        insert: impl FnOnce() -> V,
    ) -> Result<&'a mut V, EntityScratchError> {
        self.validate_entity(world, entity)?;
        let slot_index = entity.slot() as usize;
        self.ensure_slot(slot_index);
        let slot = &mut self.slots[slot_index];

        if slot.value.is_none() {
            slot.generation = entity.generation();
            slot.value = Some(insert());
            slot.active_index = self.active.len();
            self.active.push(entity.slot());
        } else if slot.generation != entity.generation() {
            let value = insert();
            let stale = slot.value.replace(value);
            slot.generation = entity.generation();
            drop(stale);
        }

        Ok(slot
            .value
            .as_mut()
            .expect("dense scratch slot was populated above"))
    }

    /// Removes and returns the value for a live entity.
    pub fn remove(
        &mut self,
        world: &World,
        entity: EntityId,
    ) -> Result<Option<V>, EntityScratchError> {
        self.validate_entity(world, entity)?;
        let Some(slot) = self.slots.get_mut(entity.slot() as usize) else {
            return Ok(None);
        };
        if slot.generation != entity.generation() {
            return Ok(None);
        }
        let value = slot.value.take();
        if value.is_some() {
            self.remove_active(entity.slot());
        }
        Ok(value)
    }

    /// Removes entries whose recorded entity generation is no longer live.
    pub fn retain_live(&mut self, world: &World) -> Result<usize, EntityScratchError> {
        self.validate_world(world)?;
        let before = self.active.len();
        let mut index = 0;
        while index < self.active.len() {
            let entity_slot = self.active[index];
            let slot = &mut self.slots[entity_slot as usize];
            let entity = EntityId::from_owned_parts(self.owner, entity_slot, slot.generation);
            if world.allocator.is_alive(entity) {
                index += 1;
            } else {
                let value = slot.value.take();
                slot.active_index = DenseSlot::<V>::INACTIVE;
                self.active.swap_remove(index);
                if index < self.active.len() {
                    let moved_slot = self.active[index] as usize;
                    self.slots[moved_slot].active_index = index;
                }
                drop(value);
            }
        }
        Ok(before - self.active.len())
    }

    /// Removes all scratch values.
    pub fn clear(&mut self) {
        while let Some(entity_slot) = self.active.pop() {
            let slot = &mut self.slots[entity_slot as usize];
            let value = slot.value.take();
            slot.active_index = DenseSlot::<V>::INACTIVE;
            drop(value);
        }
    }

    /// Returns the number of stored values, including entries that have become stale.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Returns whether no scratch values are stored.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    fn ensure_slot(&mut self, slot_index: usize) {
        self.slots.resize_with(slot_index + 1, DenseSlot::vacant);
    }

    fn slot_value(&self, entity: EntityId) -> Option<&V> {
        self.slots
            .get(entity.slot() as usize)
            .filter(|slot| slot.generation == entity.generation())
            .and_then(|slot| slot.value.as_ref())
    }

    fn slot_value_mut(&mut self, entity: EntityId) -> Option<&mut V> {
        self.slots
            .get_mut(entity.slot() as usize)
            .filter(|slot| slot.generation == entity.generation())
            .and_then(|slot| slot.value.as_mut())
    }

    fn remove_active(&mut self, entity_slot: u32) {
        let slot_index = entity_slot as usize;
        let active_index = self.slots[slot_index].active_index;
        debug_assert_ne!(active_index, DenseSlot::<V>::INACTIVE);
        debug_assert_eq!(self.active[active_index], entity_slot);
        self.active.swap_remove(active_index);
        self.slots[slot_index].active_index = DenseSlot::<V>::INACTIVE;
        if active_index < self.active.len() {
            let moved_slot = self.active[active_index] as usize;
            self.slots[moved_slot].active_index = active_index;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldBuilder;
    use alloc::rc::Rc;
    use core::cell::Cell;

    #[test]
    fn capacity_and_get_or_insert_cover_dense_storage() {
        let mut world = WorldBuilder::new().build().expect("world");
        let entity = world.spawn().expect("spawn");
        let calls = Cell::new(0);
        let mut scratch = DenseEntityScratch::with_capacity(&world, 4);

        assert!(scratch.capacity() >= 4);
        assert_eq!(
            *scratch
                .get_or_insert_with(&world, entity, || {
                    calls.set(calls.get() + 1);
                    7
                })
                .expect("insert"),
            7
        );
        assert_eq!(
            *scratch
                .get_or_insert_with(&world, entity, || {
                    calls.set(calls.get() + 1);
                    9
                })
                .expect("get"),
            7
        );
        assert_eq!(calls.get(), 1);
        scratch.reserve(8);
        scratch.try_reserve(8).expect("try reserve");
    }

    #[test]
    fn stale_generation_replacement_drops_once_and_reuses_active_slot() {
        struct Tracked(Rc<Cell<usize>>);

        impl Drop for Tracked {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let mut world = WorldBuilder::new().build().expect("world");
        let stale = world.spawn().expect("spawn");
        let drops = Rc::new(Cell::new(0));
        let mut scratch = DenseEntityScratch::new(&world);
        scratch
            .insert(&world, stale, Tracked(Rc::clone(&drops)))
            .expect("insert");
        world.despawn(stale).expect("despawn");
        let replacement = world.spawn().expect("reuse slot");

        scratch
            .insert(&world, replacement, Tracked(Rc::clone(&drops)))
            .expect("replace stale");
        assert_eq!(scratch.len(), 1);
        assert_eq!(drops.get(), 1);
        scratch.clear();
        assert_eq!(drops.get(), 2);
        assert!(scratch.is_empty());
    }

    #[test]
    fn remove_updates_the_swapped_active_slot_index() {
        let mut world = WorldBuilder::new().build().expect("world");
        let first = world.spawn().expect("first");
        let middle = world.spawn().expect("middle");
        let last = world.spawn().expect("last");
        let mut scratch = DenseEntityScratch::new(&world);
        scratch.insert(&world, first, 1).expect("insert first");
        scratch.insert(&world, middle, 2).expect("insert middle");
        scratch.insert(&world, last, 3).expect("insert last");

        assert_eq!(
            scratch.remove(&world, first).expect("remove first"),
            Some(1)
        );
        assert_eq!(scratch.remove(&world, last).expect("remove moved"), Some(3));
        assert_eq!(
            scratch.remove(&world, middle).expect("remove middle"),
            Some(2)
        );
        assert!(scratch.is_empty());
    }
}
