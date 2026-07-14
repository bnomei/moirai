//! Deterministic replay, step records, drivers, and report comparison for host tests.
//!
//! Testkit runs finite checked [`StepIndex`] loops over either an [`App`](crate::app::App)
//! ([`replay_app`]) or a host [`ReplayDriver`] built from a seed fixture. Host fixtures supply
//! `S: Eq` snapshots and optional metrics; Moirai never reflects the type-erased
//! [`World`](crate::world::World).
//!
//! Learn the full replay flow in [`crate::examples::tier_d::d05_deterministic_replay`].

mod app;
mod config;
mod driver;
mod error;
mod record;
mod replay;
mod report;
mod step;

pub use app::replay_app;
pub use config::{CapturePolicy, ReplayConfig, ReplayConfigError};
pub use driver::ReplayDriver;
pub use error::ReplayRunError;
pub use record::{MetricSample, StepRecord, StepSnapshot};
pub use replay::{reports_match, run_replay};
pub use report::{ReplayFailure, ReplayReport};
pub use step::{StepIndex, StepIndexError};
