//! Neutral deterministic replay primitives for host verification.
//!
//! Testkit depends only on core Moirai and `alloc`. It never reflects or serializes an
//! arbitrary type-erased `World`; hosts supply exact `S: Eq` snapshots and canonical ordering.
//!
//! Learn the complete replay flow in
//! [`crate::examples::tier_d::d05_deterministic_replay`].

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
