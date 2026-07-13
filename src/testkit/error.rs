use super::step::StepIndex;

/// Unified replay failure source for driver and checked step overflow.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReplayRunError<E> {
    Source(E),
    StepOverflow,
    /// Driver returned evidence for a different checked step than requested.
    StepMismatch {
        reported: StepIndex,
        expected: StepIndex,
    },
}
