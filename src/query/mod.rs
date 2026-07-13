//! Stable query facade.
//!
//! # Examples
//!
//! Immutable sparse query:
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryParams, QuerySpec};
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Clone, Copy)]
//! struct Position(i32);
//!
//! let mut builder = WorldBuilder::new();
//! builder
//!     .register_component::<Position>(ComponentOptions::sparse())
//!     .expect("register");
//! let mut world = builder.build().expect("build");
//! let entity = world.spawn().expect("spawn");
//! world.insert(entity, Position(1)).expect("insert");
//!
//! let spec = QuerySpec::new();
//! let count = world
//!     .query::<Position>(&spec, QueryParams::new())
//!     .expect("query")
//!     .count();
//! assert_eq!(count, 1);
//! ```
//!
//! Closure-scoped mutation:
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryParams, QuerySpec};
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Clone, Copy)]
//! struct Velocity(i32);
//!
//! let mut builder = WorldBuilder::new();
//! builder
//!     .register_component::<Velocity>(ComponentOptions::sparse())
//!     .expect("register");
//! let mut world = builder.build().expect("build");
//! let entity = world.spawn().expect("spawn");
//! world.insert(entity, Velocity(1)).expect("insert");
//!
//! world
//!     .for_each_mut::<Velocity>(&QuerySpec::new(), QueryParams::new(), |_, vel| {
//!         vel.0 += 1;
//!         Ok(())
//!     })
//!     .expect("mutate");
//! assert_eq!(world.get::<Velocity>(entity).expect("get").expect("present").0, 2);
//! ```

mod cache;
mod cursor;
mod effects;
mod error;
mod exact_id;
mod iter;
mod params;
mod spec;

pub use cache::{QueryCache, QueryResultCache};
pub use cursor::QueryCursor;
pub use effects::{QueryCommands, QueryEffects};
pub use error::QueryError;
pub use exact_id::ExactIdPolicy;
pub(crate) use iter::Query1State;
pub use iter::{Query1, Query2};
pub use params::QueryParams;
pub use spec::QuerySpec;
