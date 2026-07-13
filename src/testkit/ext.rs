use crate::schedule::{FlushMode, Schedule};
use crate::time::ChangeTick;
use crate::world::{World, WorldError};

/// Test-only controls for exhausting checked world counters without exposing raw runtime ids.
pub trait WorldTestExt {
    fn set_change_tick_for_test(&mut self, tick: ChangeTick);

    fn set_world_tick_for_test(&mut self, raw: u64);

    fn set_event_sequence_for_test<E: Clone + 'static>(
        &mut self,
        next_sequence: u64,
        closed: bool,
    ) -> Result<(), WorldError>;
}

impl WorldTestExt for World {
    fn set_change_tick_for_test(&mut self, tick: ChangeTick) {
        crate::world::set_change_tick_for_test(self, tick);
    }

    fn set_world_tick_for_test(&mut self, raw: u64) {
        crate::world::set_world_tick_for_test(self, raw);
    }

    fn set_event_sequence_for_test<E: Clone + 'static>(
        &mut self,
        next_sequence: u64,
        closed: bool,
    ) -> Result<(), WorldError> {
        crate::world::set_event_sequence_for_test::<E>(self, next_sequence, closed)
    }
}

/// Test-only inspection of compiled schedule configuration.
pub trait ScheduleTestExt {
    fn stage_flush_mode_for_test(&self, label: &str) -> Option<FlushMode>;
}

impl ScheduleTestExt for Schedule {
    fn stage_flush_mode_for_test(&self, label: &str) -> Option<FlushMode> {
        crate::schedule::stage_flush_mode_for_test(self, label)
    }
}
