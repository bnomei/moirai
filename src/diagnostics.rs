//! Platform-neutral diagnostic observer contracts for [`crate::App`] execution.
//!
//! Hosts implement [`Observer`] to receive [`DiagnosticEvent`] notifications at pass, stage, system,
//! flush, fixed-debt, and fault boundaries without coupling to a specific logging backend.

/// Synchronous diagnostic observer invoked by [`crate::App`] at execution boundaries.
pub trait Observer {
    /// Handles one diagnostic event emitted during Update or Render.
    fn observe(&mut self, event: DiagnosticEvent<'_>);
}

/// Non-exhaustive diagnostic event vocabulary surfaced by [`crate::App`].
#[non_exhaustive]
pub enum DiagnosticEvent<'a> {
    /// Update pass started with the host-provided delta.
    UpdateStart {
        /// Elapsed seconds for this Update pass.
        delta_seconds: f32,
    },
    /// Update pass finished after frame cleanup.
    UpdateFinish,
    /// Render pass started with the host-provided delta.
    RenderStart {
        /// Elapsed seconds for this Render pass.
        delta_seconds: f32,
    },
    /// Render pass finished after frame cleanup.
    RenderFinish,
    /// One schedule stage began execution.
    StageStart {
        /// Compiled stage label.
        name: &'a str,
    },
    /// One schedule stage finished execution.
    StageFinish {
        /// Compiled stage label.
        name: &'a str,
    },
    /// One system body began execution.
    SystemStart {
        /// Compiled system name.
        name: &'a str,
    },
    /// One system body finished execution.
    SystemFinish {
        /// Compiled system name.
        name: &'a str,
    },
    /// Deferred commands were flushed at a schedule boundary.
    FlushComplete,
    /// Fixed debt exceeded the substep cap and whole intervals were dropped.
    FixedDebtDropped {
        /// Number of whole intervals discarded.
        steps: u128,
    },
    /// Overdue fixed intervals were combined into one coalesced run.
    FixedDebtCoalesced {
        /// Number of whole intervals represented.
        steps: u128,
    },
    /// A terminal execution fault was recorded.
    Fault {
        /// Stage label active when the fault occurred, if known.
        stage: Option<&'a str>,
        /// System name active when the fault occurred, if known.
        system: Option<&'a str>,
    },
}
