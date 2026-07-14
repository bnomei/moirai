//! # B04 — Run a fixed-timestep simulation
//!
//! **Goal:** execute deterministic fixed steps inside a longer host frame.
//!
//! ```
//! use core::time::Duration;
//! use moirai::{stage, AppBuilder, FixedConfig, System};
//!
//! #[derive(Debug, PartialEq)]
//! struct Steps(u32);
//!
//! let mut builder = AppBuilder::new();
//! builder.insert_resource(Steps(0));
//! builder.fixed(FixedConfig::new(Duration::from_millis(16)).unwrap());
//! builder.add_system(System::new("simulate", stage::FIXED_UPDATE, |world, _| {
//!     world.resource_mut::<Steps>().unwrap().unwrap().0 += 1;
//!     assert!(world.fixed_step().is_some());
//! })).unwrap();
//! let mut app = builder.build().unwrap();
//!
//! app.update(0.050).unwrap();
//! assert_eq!(app.world().resource::<Steps>().unwrap(), Some(&Steps(3)));
//! assert!(app.world().fixed_step().is_none());
//! ```
//!
//! The schedule accumulates host time, runs whole fixed substeps up to the configured
//! cap, and clears the transient [`crate::FixedStep`] after the fixed stage.
//!
//! **Next:** [`crate::examples::tier_c::c01_prepared_queries`].
