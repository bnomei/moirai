//! # C03 — Observe changes once with a cursor
//!
//! **Goal:** process a changed component, then advance the query window.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryCursor, QueryPolicy, QuerySpec, QueryWindow};
//! use moirai::world::WorldBuilder;
//!
//! struct Position(i32);
//!
//! let mut builder = WorldBuilder::new();
//! builder.register_component::<Position>(ComponentOptions::sparse()).unwrap();
//! let mut world = builder.build().unwrap();
//! let entity = world.spawn().unwrap();
//! world.insert(entity, Position(1)).unwrap();
//! let spec = QuerySpec::new().changed::<Position>();
//! let mut cursor = QueryCursor::from_spec_now::<Position>(&mut world, &spec).unwrap();
//! world.get_mut::<Position>(entity).unwrap().unwrap().0 = 2;
//!
//! let mut query = world
//!     .prepare_query1::<Position>(spec, QueryPolicy::Prepared)
//!     .unwrap();
//! assert_eq!(query.iter(&mut world, QueryWindow::Cursor(&mut cursor)).unwrap().count(), 1);
//! assert_eq!(query.iter(&mut world, QueryWindow::Cursor(&mut cursor)).unwrap().count(), 0);
//! ```
//!
//! The cursor is bound to both the world and the query fingerprint. Exhausting an
//! iterator commits its captured change tick, so the next execution sees only newer work.
//!
//! **Next:** [`c04_query_effects`](super::c04_query_effects).
