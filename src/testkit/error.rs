//! Replay run failure sources shared by app and driver paths.

use super::step::StepIndex;

/// Unified replay failure source for driver and checked step overflow.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReplayRunError<E> {
    /// Underlying host, driver, or app error from the current replay step.
    Source(E),
    /// Checked [`StepIndex`](super::step::StepIndex) overflow while advancing the replay loop.
    StepOverflow,
    /// Driver returned evidence for a different checked step than requested.
    StepMismatch {
        reported: StepIndex,
        expected: StepIndex,
    },
}
