//! Internal query execution against world storage and topology revisions.
//!
//! Resolves [`QuerySpec`](crate::query::QuerySpec) into cached plans, collects entity
//! membership from sparse, table, and archetype sources, and drives mutable query
//! traversal with change-tick preflight.

pub(crate) mod cache;
pub(crate) mod cached_source;
pub(crate) mod collect;
#[allow(dead_code)]
mod entities;
pub(crate) mod filter;
mod ids;
pub(crate) mod mutate;
pub(crate) mod plan;
pub(crate) mod plan_cache;
mod query1;
mod query2;
pub(crate) mod result_cache;
mod spec;
