//! Shared stage-operation classification for schedule, event retention, and frame cleanup.
//!
//! [`StageOperation`] partitions Update and Render passes so frame-scoped events and flush policy
//! can target the correct host operation.

/// Which [`crate::App`] operation owns a stage, frame channel, or structural command surface.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum StageOperation {
    /// Variable-timestep simulation pass driven by [`crate::App::update`].
    Update,
    /// Presentation pass driven by [`crate::App::render`].
    Render,
}
