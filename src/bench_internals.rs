//! Unstable measurement seams for repository benchmarks.
//!
//! This module exists only with the non-default `bench-internals` feature. It is
//! not a compatibility API and may change or disappear with benchmark needs.

use crate::query::{QueryError, QueryParams, QuerySpec};
use crate::world::World;

/// Resolves and executes the retained ad-hoc one-component path once.
pub fn adhoc_query1_count<T: 'static>(
    world: &mut World,
    spec: &QuerySpec,
) -> Result<usize, QueryError> {
    Ok(world.query::<T>(spec, QueryParams::new())?.count())
}

/// Resolves and executes the retained ad-hoc two-component path once.
pub fn adhoc_query2_count<A: 'static, B: 'static>(
    world: &mut World,
    spec: &QuerySpec,
) -> Result<usize, QueryError> {
    Ok(world.query2::<A, B>(spec, QueryParams::new())?.count())
}
