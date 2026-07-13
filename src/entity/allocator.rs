use alloc::vec::Vec;

use super::EntityId;

const INITIAL_GENERATION: u32 = 1;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum SlotState {
    Free,
    Reserved,
    Live,
    Retired,
}

/// Generational entity allocator with explicit occupancy states.
#[allow(dead_code)]
pub(crate) struct EntityAllocator {
    generations: Vec<u32>,
    states: Vec<SlotState>,
    free: Vec<u32>,
    next: u32,
    live_count: u32,
    reserved_count: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum AllocatorError {
    StaleEntity,
    DoubleFree,
    NotLive,
    SlotRetired,
    GenerationOverflow,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) struct AllocatorCounts {
    pub live: u32,
    pub reserved: u32,
    pub free: u32,
    pub retired: u32,
}

#[allow(dead_code)]
impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            states: Vec::new(),
            free: Vec::new(),
            next: 0,
            live_count: 0,
            reserved_count: 0,
        }
    }

    pub fn counts(&self) -> AllocatorCounts {
        let mut retired = 0u32;
        for state in &self.states {
            if *state == SlotState::Retired {
                retired += 1;
            }
        }
        AllocatorCounts {
            live: self.live_count,
            reserved: self.reserved_count,
            free: self.free.len() as u32,
            retired,
        }
    }

    pub fn alloc(&mut self) -> EntityId {
        self.alloc_live()
    }

    pub fn reserve(&mut self) -> Result<EntityId, AllocatorError> {
        let (slot, generation) = self.acquire_slot(SlotState::Reserved)?;
        self.reserved_count += 1;
        Ok(EntityId::from_parts(slot, generation))
    }

    pub fn commit_reserved(&mut self, id: EntityId) -> Result<(), AllocatorError> {
        let slot = id.slot() as usize;
        self.ensure_state(slot, SlotState::Reserved, id.generation())?;
        self.states[slot] = SlotState::Live;
        self.reserved_count -= 1;
        self.live_count += 1;
        Ok(())
    }

    pub fn release_reserved(&mut self, id: EntityId) -> Result<(), AllocatorError> {
        let slot = id.slot() as usize;
        self.ensure_state(slot, SlotState::Reserved, id.generation())?;
        self.reserved_count -= 1;
        self.recycle_slot(slot)
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        let slot = id.slot() as usize;
        matches!(
            self.slot_state(slot),
            Some((SlotState::Live, generation)) if generation == id.generation()
        )
    }

    pub fn is_reserved(&self, id: EntityId) -> bool {
        let slot = id.slot() as usize;
        matches!(
            self.slot_state(slot),
            Some((SlotState::Reserved, generation)) if generation == id.generation()
        )
    }

    pub fn free(&mut self, id: EntityId) -> Result<(), AllocatorError> {
        let slot = id.slot() as usize;
        self.ensure_state(slot, SlotState::Live, id.generation())?;
        self.live_count -= 1;
        self.recycle_slot(slot)
    }

    fn alloc_live(&mut self) -> EntityId {
        let (slot, generation) = self
            .acquire_slot(SlotState::Live)
            .expect("live allocation cannot fail");
        self.live_count += 1;
        EntityId::from_parts(slot, generation)
    }

    fn acquire_slot(&mut self, state: SlotState) -> Result<(u32, u32), AllocatorError> {
        if let Some(slot) = self.free.pop() {
            let slot = slot as usize;
            let generation = self.generations[slot];
            self.states[slot] = state;
            return Ok((slot as u32, generation));
        }

        let slot = self.next;
        self.next = self.next.checked_add(1).expect("slot index overflow");
        self.generations.push(INITIAL_GENERATION);
        self.states.push(state);
        Ok((slot, INITIAL_GENERATION))
    }

    fn recycle_slot(&mut self, slot: usize) -> Result<(), AllocatorError> {
        match self.generations[slot].checked_add(1) {
            Some(next_generation) => {
                self.generations[slot] = next_generation;
                self.states[slot] = SlotState::Free;
                self.free.push(slot as u32);
                Ok(())
            }
            None => {
                self.states[slot] = SlotState::Retired;
                Err(AllocatorError::GenerationOverflow)
            }
        }
    }

    fn ensure_state(
        &self,
        slot: usize,
        expected: SlotState,
        generation: u32,
    ) -> Result<(), AllocatorError> {
        match self.slot_state(slot) {
            Some((SlotState::Retired, _)) => Err(AllocatorError::SlotRetired),
            Some((state, gen)) if state == expected && gen == generation => Ok(()),
            Some((_, gen)) if gen != generation => Err(AllocatorError::StaleEntity),
            Some((SlotState::Live, _)) if expected != SlotState::Live => {
                Err(AllocatorError::NotLive)
            }
            Some((SlotState::Free | SlotState::Reserved, _)) if expected == SlotState::Live => {
                Err(AllocatorError::StaleEntity)
            }
            Some(_) => Err(AllocatorError::NotLive),
            None => Err(AllocatorError::StaleEntity),
        }
    }

    fn slot_state(&self, slot: usize) -> Option<(SlotState, u32)> {
        let generation = *self.generations.get(slot)?;
        Some((self.states[slot], generation))
    }

    pub(crate) fn slot_capacity(&self) -> usize {
        self.generations.len()
    }

    pub(crate) fn generation_for_slot(&self, slot: usize) -> u32 {
        self.generations.get(slot).copied().unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn set_generation_for_test(&mut self, id: EntityId, generation: u32) {
        self.generations[id.slot() as usize] = generation;
    }

    #[cfg(test)]
    #[allow(private_interfaces)]
    pub(crate) fn ensure_state_for_test(
        &self,
        slot: usize,
        expected: SlotState,
        generation: u32,
    ) -> Result<(), AllocatorError> {
        self.ensure_state(slot, expected, generation)
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
