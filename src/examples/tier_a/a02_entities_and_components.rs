//! # A02 — Register components and spawn entities
//!
//! **Goal:** define the world's schema before storing typed component values.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Debug, PartialEq)]
//! struct Position { x: i32, y: i32 }
//!
//! let mut builder = WorldBuilder::new();
//! builder
//!     .register_component::<Position>(ComponentOptions::sparse())
//!     .expect("register Position");
//! let mut world = builder.build().expect("build world");
//!
//! let entity = world.spawn().expect("spawn entity");
//! world.insert(entity, Position { x: 3, y: 4 }).expect("insert");
//!
//! assert!(world.is_alive(entity));
//! assert_eq!(world.get::<Position>(entity).unwrap(), Some(&Position { x: 3, y: 4 }));
//! ```
//!
//! Registration fixes each component's storage policy before the world starts.
//! Entity handles are world-owned and generational, so every access is checked.
//!
//! **Next:** [`a03_bundles_and_tags`](super::a03_bundles_and_tags).
