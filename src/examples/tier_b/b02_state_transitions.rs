//! # B02 — Apply state transitions at a schedule boundary
//!
//! **Goal:** request a host state change and commit it in a named system.
//!
//! ```
//! use moirai::state::{apply, State};
//! use moirai::{stage, AppBuilder};
//!
//! #[derive(Debug, Eq, PartialEq)]
//! enum Mode { Menu, Playing }
//!
//! let mut builder = AppBuilder::new();
//! builder.insert_state(Mode::Menu);
//! builder.add_system(apply::<Mode>("apply mode", stage::UPDATE)).unwrap();
//! let mut app = builder.build().unwrap();
//!
//! app.world_mut()
//!     .resource_mut::<State<Mode>>().unwrap().unwrap()
//!     .request(Mode::Playing).unwrap();
//! app.update(0.0).unwrap();
//!
//! let state = app.world().resource::<State<Mode>>().unwrap().unwrap();
//! assert_eq!(state.current(), &Mode::Playing);
//! assert_eq!(state.previous(), Some(&Mode::Menu));
//! assert!(state.transition_tick().is_some());
//! ```
//!
//! `State::request` records intent; [`crate::state::apply`] commits it during the
//! schedule and records the change tick used by state-aware conditions.
//!
//! **Next:** [`b03_typed_events`](super::b03_typed_events).
