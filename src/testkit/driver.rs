use super::record::StepRecord;
use super::step::StepIndex;

/// Host-owned deterministic replay step contract.
pub trait ReplayDriver {
    type Snapshot: Eq;
    type Error;

    fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error>;
}
