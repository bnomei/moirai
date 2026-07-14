//! # C02 — Filter a query with tags
//!
//! **Goal:** select only positioned entities classified as players.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
//! use moirai::world::WorldBuilder;
//!
//! struct Position(i32);
//! struct Player;
//!
//! let mut builder = WorldBuilder::new();
//! builder.register_component::<Position>(ComponentOptions::sparse()).unwrap();
//! let player = builder.register_component::<Player>(ComponentOptions::tag()).unwrap();
//! let mut world = builder.build().unwrap();
//! let tagged = world.spawn().unwrap();
//! let other = world.spawn().unwrap();
//! world.insert(tagged, Position(10)).unwrap();
//! world.insert(other, Position(20)).unwrap();
//! world.add_tag(tagged, &player).unwrap();
//!
//! let spec = QuerySpec::new().with_tag::<Player>();
//! let mut query = world.prepare_query1::<Position>(spec, QueryPolicy::Prepared).unwrap();
//! let values: Vec<_> = query
//!     .iter(&mut world, QueryWindow::All).unwrap()
//!     .map(|(_, position)| position.0)
//!     .collect();
//! assert_eq!(values, vec![10]);
//! ```
//!
//! `QuerySpec` describes structural membership independently from the component being
//! yielded. Typed tag filters are resolved against the same checked world schema.
//!
//! **Next:** [`c03_change_cursors`](super::c03_change_cursors).
