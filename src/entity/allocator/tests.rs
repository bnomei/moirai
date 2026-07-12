use super::*;
use rstest::rstest;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RefState {
    Free,
    #[allow(dead_code)]
    Reserved,
    Live,
    Retired,
}

struct RefModel {
    generations: Vec<u32>,
    states: Vec<RefState>,
    free: Vec<u32>,
    next: u32,
}

impl RefModel {
    fn new() -> Self {
        Self {
            generations: Vec::new(),
            states: Vec::new(),
            free: Vec::new(),
            next: 0,
        }
    }

    fn alloc(&mut self) -> EntityId {
        let slot = if let Some(slot) = self.free.pop() {
            slot as usize
        } else {
            let slot = self.next;
            self.next += 1;
            self.generations.push(INITIAL_GENERATION);
            self.states.push(RefState::Free);
            slot as usize
        };
        self.states[slot] = RefState::Live;
        EntityId::from_parts(slot as u32, self.generations[slot])
    }

    fn free(&mut self, id: EntityId) -> bool {
        let slot = id.slot() as usize;
        if slot >= self.generations.len()
            || self.states[slot] != RefState::Live
            || self.generations[slot] != id.generation()
        {
            return false;
        }
        let next = self.generations[slot].checked_add(1);
        match next {
            Some(0) | None => {
                self.states[slot] = RefState::Retired;
                false
            }
            Some(gen) => {
                self.generations[slot] = gen;
                self.states[slot] = RefState::Free;
                self.free.push(slot as u32);
                true
            }
        }
    }

    fn is_alive(&self, id: EntityId) -> bool {
        let slot = id.slot() as usize;
        slot < self.generations.len()
            && self.states[slot] == RefState::Live
            && self.generations[slot] == id.generation()
    }
}

#[rstest]
fn initial_generation_is_one() {
    let mut alloc = EntityAllocator::new();
    let id = alloc.alloc();
    assert_eq!(id.generation(), INITIAL_GENERATION);
}

#[rstest]
fn deterministic_initial_allocation_order() {
    let mut alloc = EntityAllocator::new();
    let a = alloc.alloc();
    let b = alloc.alloc();
    assert_eq!(a.slot(), 0);
    assert_eq!(b.slot(), 1);
    assert_eq!(a.generation(), 1);
    assert_eq!(b.generation(), 1);
}

#[rstest]
fn reuse_bumps_generation() {
    let mut alloc = EntityAllocator::new();
    let a = alloc.alloc();
    alloc.free(a).expect("first free");
    assert!(!alloc.is_alive(a));
    let b = alloc.alloc();
    assert_eq!(a.slot(), b.slot());
    assert_eq!(b.generation(), 2);
}

#[rstest]
fn freed_but_not_reallocated_is_dead() {
    let mut alloc = EntityAllocator::new();
    let id = alloc.alloc();
    alloc.free(id).expect("free");
    assert!(!alloc.is_alive(id));
}

#[rstest]
fn double_free_is_non_destructive() {
    let mut alloc = EntityAllocator::new();
    let id = alloc.alloc();
    alloc.free(id).expect("first free");
    let before = alloc.counts();
    assert_eq!(alloc.free(id), Err(AllocatorError::StaleEntity));
    assert_eq!(alloc.counts(), before);
}

#[rstest]
fn reserved_is_not_alive() {
    let mut alloc = EntityAllocator::new();
    let id = alloc.reserve().expect("reserve");
    assert!(!alloc.is_alive(id));
    assert_eq!(alloc.counts().reserved, 1);
}

#[rstest]
fn generation_overflow_retires_slot() {
    let mut alloc = EntityAllocator::new();
    let id = alloc.alloc();
    alloc.set_generation_for_test(id, u32::MAX);
    let exhausted = EntityId::from_parts(id.slot(), u32::MAX);
    assert_eq!(alloc.free(exhausted), Err(AllocatorError::GenerationOverflow));
    assert!(!alloc.is_alive(exhausted));
    let counts = alloc.counts();
    assert_eq!(counts.retired, 1);
    assert_eq!(counts.free, 0);
}

#[rstest]
fn capacity_growth_preserves_live_handles() {
    let mut alloc = EntityAllocator::new();
    let mut live = Vec::new();
    for _ in 0..32 {
        live.push(alloc.alloc());
    }
    for id in &live {
        assert!(alloc.is_alive(*id));
    }
}

#[rstest]
fn randomized_trace_matches_reference_model() {
    let mut alloc = EntityAllocator::new();
    let mut model = RefModel::new();
    let mut live = Vec::new();
    let mut seed = 0xC0FFEE_u32;

    for step in 0..2_000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        if live.is_empty() || seed % 3 != 2 {
            let id = alloc.alloc();
            assert!(alloc.is_alive(id));
            let model_id = model.alloc();
            assert_eq!(id.slot(), model_id.slot());
            assert_eq!(id.generation(), model_id.generation());
            live.push(id);
        } else {
            let index = (seed as usize) % live.len();
            let id = live[index];
            let alloc_result = alloc.free(id);
            let model_result = model.free(id);
            assert_eq!(alloc_result.is_ok(), model_result);
            live.swap_remove(index);
        }

        let counts = alloc.counts();
        assert_eq!(counts.live as usize, live.len());
        for id in &live {
            assert!(alloc.is_alive(*id));
            assert!(model.is_alive(*id));
        }
        let _ = step;
    }
}