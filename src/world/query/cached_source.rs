//! Resolve cached entity ids from membership or result cache handles.

#![allow(dead_code)]

use crate::entity::EntityId;
use crate::query::{QueryCache, QueryError, QueryResultCache};
use crate::world::World;

/// Owner-scoped cache handle selected for query iteration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QueryCachedSource {
    Membership(QueryCache),
    Result(QueryResultCache),
}

impl World {
    pub(crate) fn cached_query_entities(
        &self,
        source: &QueryCachedSource,
        fingerprint: u64,
    ) -> Result<&[EntityId], QueryError> {
        match source {
            QueryCachedSource::Membership(cache) => {
                let slot = self.validate_membership_cache(cache, fingerprint)?;
                Ok(&self.membership_caches[slot].members)
            }
            QueryCachedSource::Result(cache) => {
                let slot = self.validate_result_cache(cache, fingerprint)?;
                Ok(&self.result_caches[slot].ids)
            }
        }
    }
}
