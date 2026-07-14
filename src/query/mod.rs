//! Stable query facade.
//!
//! # Examples
//!
//! Immutable sparse query:
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
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
//! let mut query = world
//!     .prepare_query1::<Position>(spec, QueryPolicy::Prepared)
//!     .expect("prepare");
//! let count = query
//!     .iter(&mut world, QueryWindow::All)
//!     .expect("iterate")
//!     .count();
//! assert_eq!(count, 1);
//! ```
//!
//! Closure-scoped mutation:
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
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
//! let mut query = world
//!     .prepare_query1::<Velocity>(QuerySpec::new(), QueryPolicy::Prepared)
//!     .expect("prepare");
//! query
//!     .for_each_mut(&mut world, QueryWindow::All, |_, vel| {
//!         vel.0 += 1;
//!         Ok(())
//!     })
//!     .expect("mutate");
//! assert_eq!(world.get::<Velocity>(entity).expect("get").expect("present").0, 2);
//! ```

mod cache;
mod cursor;
mod effects;
#[allow(dead_code)]
mod entity;
mod error;
mod exact_id;
mod iter;
#[allow(dead_code)]
mod params;
mod prepared;
mod spec;

pub(crate) use cache::{QueryCache, QueryResultCache};
pub use cursor::QueryCursor;
pub use effects::{QueryCommands, QueryEffects};
#[allow(unused_imports)]
pub(crate) use entity::{EntityRef, QueryEntities, QueryIds};
pub use error::QueryError;
pub use exact_id::ExactIdPolicy;
pub(crate) use iter::Query1State;
pub use iter::{Query1, Query2};
pub(crate) use params::QueryParams;
pub use prepared::{PreparedQuery1, PreparedQuery2, QueryPolicy, QueryWindow};
pub use spec::QuerySpec;
