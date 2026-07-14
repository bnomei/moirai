//! # A03 — Spawn bundles and add tags
//!
//! **Goal:** create an entity from several values and classify it with a zero-sized tag.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Debug, PartialEq)]
//! struct Position(i32, i32);
//! #[derive(Debug, PartialEq)]
//! struct Health(u16);
//! struct Player;
//!
//! let mut builder = WorldBuilder::new();
//! builder.register_component::<Position>(ComponentOptions::table()).unwrap();
//! builder.register_component::<Health>(ComponentOptions::sparse()).unwrap();
//! let player = builder.register_component::<Player>(ComponentOptions::tag()).unwrap();
//! let mut world = builder.build().unwrap();
//!
//! let entity = world.spawn_bundle((Position(8, 13), Health(100))).unwrap();
//! assert!(world.add_tag(entity, &player).unwrap());
//!
//! assert_eq!(world.get::<Position>(entity).unwrap(), Some(&Position(8, 13)));
//! assert_eq!(world.get::<Health>(entity).unwrap(), Some(&Health(100)));
//! assert!(world.has_tag(entity, &player).unwrap());
//! ```
//!
//! A tuple implements [`crate::Bundle`], so the insertion is validated as one
//! structural operation. Tags carry membership only and therefore use a handle.
//!
//! **Next:** [`a04_deferred_commands`](super::a04_deferred_commands).
