//! Query selection, traversal, caching, and safe mutation.
//!
//! **Semantic map**
//! - [`QuerySpec`]: structural filters (`with`, `without`, tags, `added`, `changed`, exact ids).
//! - `QueryParams` / [`QueryWindow`]: execution window (`since`, [`QueryCursor`], caches).
//! - [`Query1`] / [`Query2`]: read iterators over sparse, table, cached, or exact sources.
//! - [`PreparedQuery1`] / [`PreparedQuery2`]: reusable plans with [`QueryPolicy`] materialization.
//! - [`QueryCursor`]: owner-bound change-detection window for added/changed filters.
//! - `QueryCache` / `QueryResultCache`: membership and result acceleration handles.
//! - [`QueryEffects`] / [`QueryCommands`]: deferred structural changes during traversal.
//! - [`QueryError`]: configuration, ownership, borrow, and cache diagnostics.

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
