//! # A04 — Defer structural changes
//!
//! **Goal:** reserve an entity, queue its components, and commit the batch explicitly.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Debug, PartialEq)]
//! struct Health(u16);
//!
//! let mut builder = WorldBuilder::new();
//! builder.register_component::<Health>(ComponentOptions::sparse()).unwrap();
//! let mut world = builder.build().unwrap();
//!
//! let entity = world.commands().unwrap().spawn().unwrap();
//! world.commands().unwrap().insert(entity, Health(9)).unwrap();
//! assert!(!world.is_alive(entity));
//!
//! let report = world.flush().expect("commit command batch");
//! assert_eq!(report.commands_applied, 2);
//! assert!(world.is_alive(entity));
//! assert_eq!(world.get::<Health>(entity).unwrap(), Some(&Health(9)));
//! ```
//!
//! Command reservation makes a future handle available without exposing a partially
//! spawned entity. `flush` validates and applies the queued structural batch.
//!
//! **Next:** [`crate::examples::tier_b::b01_system_ordering`].
