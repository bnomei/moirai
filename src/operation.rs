//! Shared stage-operation classification for schedule and event policy.

/// Which App operation owns a stage, frame channel, or structural command surface.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum StageOperation {
    Update,
    Render,
}
