//! # D05 — Capture deterministic replay evidence
//!
//! **Goal:** run the same app twice and compare exact post-flush snapshots.
//!
//! This lesson requires the `testkit` feature.
//!
//! ```
//! use moirai::testkit::{replay_app, reports_match, CapturePolicy, ReplayConfig};
//! use moirai::{stage, AppBuilder, System};
//!
//! #[derive(Clone, Debug, Eq, PartialEq)]
//! struct Tick(u64);
//!
//! fn app() -> moirai::App {
//!     let mut builder = AppBuilder::new();
//!     builder.add_system(System::new("step", stage::UPDATE, |_, _| {})).unwrap();
//!     builder.build().unwrap()
//! }
//!
//! let config = ReplayConfig::new(42, 3, CapturePolicy::EveryStep).unwrap();
//! let first = replay_app(&mut app(), config.clone(), 1.0 / 60.0,
//!     |world| Tick(world.world_tick().raw()), |_| Vec::new()).unwrap();
//! let second = replay_app(&mut app(), config, 1.0 / 60.0,
//!     |world| Tick(world.world_tick().raw()), |_| Vec::new()).unwrap();
//!
//! assert!(reports_match(&first, &second));
//! assert_eq!(first.snapshots().collect::<Vec<_>>(), vec![&Tick(1), &Tick(2), &Tick(3)]);
//! ```
//!
//! `replay_app` captures after the final structural flush of each update. The host
//! supplies exact snapshots and metrics; Moirai supplies finite stepping and partial
//! failure evidence without reflecting the type-erased world.
//!
//! **Next:** return to [`crate::examples`] or use the crate's API reference for your host.
