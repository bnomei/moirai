//! Stable query facade.
//!
//! Learn the query surface in the ordered lessons beginning with
//! [`crate::examples::tier_c::c01_prepared_queries`].

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
