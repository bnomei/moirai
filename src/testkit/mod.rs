//! Neutral deterministic replay primitives for host verification.
//!
//! Testkit depends only on core Moirai and `alloc`. It never reflects or serializes an
//! arbitrary type-erased `World`; hosts supply exact `S: Eq` snapshots and canonical ordering.
//!
//! # Example
//!
//! ```
//! use moirai::schedule::{stage, System};
//! use moirai::testkit::{replay_app, CapturePolicy, MetricSample, ReplayConfig};
//! use moirai::AppBuilder;
//!
//! #[derive(Clone, Debug, Eq, PartialEq)]
//! struct TickSnapshot(u64);
//!
//! let mut builder = AppBuilder::new();
//! builder
//!     .add_system(System::new("noop", stage::UPDATE, |_world, _dt| {}))
//!     .expect("system");
//! let mut app = builder.build().expect("app");
//!
//! let config = ReplayConfig::new(7, 2, CapturePolicy::EveryStep).expect("config");
//! let report = replay_app(
//!     &mut app,
//!     config,
//!     1.0 / 60.0,
//!     |world| TickSnapshot(world.world_tick().raw()),
//!     |_world| Vec::new(),
//! )
//! .expect("replay");
//!
//! assert_eq!(report.step_snapshots().len(), 2);
//! assert_eq!(report.step_snapshots()[0].snapshot(), &TickSnapshot(1));
//! assert_eq!(report.step_snapshots()[1].snapshot(), &TickSnapshot(2));
//! ```

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
