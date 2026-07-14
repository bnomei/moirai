//! Host-owned replay driver contract for [`run_replay`](super::replay::run_replay).

use super::record::StepRecord;
use super::step::StepIndex;

/// Host fixture that advances deterministic replay one checked step at a time.
pub trait ReplayDriver {
    /// Exact snapshot type the fixture records for report comparison.
    type Snapshot: Eq;
    /// Error returned when the fixture cannot complete the requested step.
    type Error;

    /// Advance one replay step and return a [`StepRecord`] whose [`StepRecord::step`] matches
    /// the checked `step` argument.
    fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error>;
}
