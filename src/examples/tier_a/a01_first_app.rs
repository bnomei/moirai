//! # A01 — Run your first app
//!
//! **Goal:** create an application with one resource and one update system.
//!
//! ```
//! use moirai::prelude::*;
//! use moirai::stage;
//!
//! #[derive(Debug, PartialEq)]
//! struct Counter(u32);
//!
//! let mut builder = AppBuilder::new();
//! builder.insert_resource(Counter(0));
//! builder
//!     .add_system(System::new("increment", stage::UPDATE, |world, _dt| {
//!         world
//!             .resource_mut::<Counter>()
//!             .expect("registered resource")
//!             .expect("seeded resource")
//!             .0 += 1;
//!     }))
//!     .expect("valid system");
//!
//! let mut app = builder.build().expect("valid app");
//! app.update(1.0 / 60.0).expect("update");
//!
//! assert_eq!(app.world().world_tick().raw(), 1);
//! assert_eq!(app.world().resource::<Counter>().unwrap(), Some(&Counter(1)));
//! ```
//!
//! `AppBuilder` validates the world and schedule together. `App::update` advances
//! the world tick and runs the built-in update stages in their checked order.
//!
//! **Next:** [`a02_entities_and_components`](super::a02_entities_and_components).
