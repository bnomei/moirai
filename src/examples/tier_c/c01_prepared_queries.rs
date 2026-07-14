//! # C01 — Reuse a prepared query
//!
//! **Goal:** read and mutate a component through one owner-bound query plan.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Debug, PartialEq)]
//! struct Position(i32);
//!
//! let mut builder = WorldBuilder::new();
//! builder.register_component::<Position>(ComponentOptions::sparse()).unwrap();
//! let mut world = builder.build().unwrap();
//! let entity = world.spawn().unwrap();
//! world.insert(entity, Position(1)).unwrap();
//!
//! let mut query = world
//!     .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Prepared)
//!     .unwrap();
//! assert_eq!(query.iter(&mut world, QueryWindow::All).unwrap().count(), 1);
//! query.for_each_mut(&mut world, QueryWindow::All, |_, position| {
//!     position.0 += 4;
//!     Ok(())
//! }).unwrap();
//! assert_eq!(world.get::<Position>(entity).unwrap(), Some(&Position(5)));
//! ```
//!
//! Preparation resolves component storage and ownership once. Each execution still
//! validates the world and window before yielding references.
//!
//! **Next:** [`c02_filters_and_tags`](super::c02_filters_and_tags).
