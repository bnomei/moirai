use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::{QueryError, QueryResultCache, QuerySpec};
use crate::world::World;

use super::collect::collect_query1_entities;
use super::spec::resolve_query1;

const RETIRED_GENERATION: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub(crate) struct ResultCacheSlot {
    pub generation: u32,
    pub fingerprint: u64,
    pub topology_revision: u64,
    pub ids: Vec<EntityId>,
}

impl ResultCacheSlot {
    fn new(fingerprint: u64, topology_revision: u64, ids: Vec<EntityId>) -> Self {
        Self {
            generation: 1,
            fingerprint,
            topology_revision,
            ids,
        }
    }

    fn is_live(&self) -> bool {
        self.generation != 0 && self.generation != RETIRED_GENERATION
    }
}

impl World {
    pub fn build_query_result_cache<T: Clone + 'static>(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryResultCache, QueryError> {
        if spec.added.is_some() || spec.changed.is_some() {
            return Err(QueryError::MovingChangeWindow);
        }
        if spec.exact_ids.is_some() {
            return Err(QueryError::ExactIdOrderConflict);
        }
        let plan = resolve_query1::<T>(self, &spec)?;
        let captured_now = self.change_tick();
        let ids = collect_query1_entities(self, &plan, crate::time::ChangeTick::ZERO, captured_now);
        let slot = self.allocate_result_cache_slot(plan.fingerprint, ids)?;
        Ok(QueryResultCache {
            owner: self.owner_token(),
            slot: slot as u32,
            generation: self.result_cache_slot(slot).generation,
        })
    }

    pub(crate) fn result_cache_slot(&self, slot: usize) -> &ResultCacheSlot {
        &self.result_caches[slot]
    }

    pub(crate) fn refresh_result_cache(
        &mut self,
        cache: &QueryResultCache,
        plan: &super::plan::ResolvedPlan,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
    ) -> Result<&[EntityId], QueryError> {
        let slot = self.validate_result_cache(cache, plan.fingerprint)?;
        let needs_refresh =
            self.result_caches[slot].topology_revision != self.query_topology_revision;
        if needs_refresh {
            let ids = collect_query1_entities(self, plan, since, captured_now);
            let revision = self.query_topology_revision;
            let entry = &mut self.result_caches[slot];
            entry.ids = ids;
            entry.topology_revision = revision;
        }
        Ok(&self.result_caches[slot].ids)
    }

    pub(crate) fn validate_result_cache(
        &self,
        cache: &QueryResultCache,
        fingerprint: u64,
    ) -> Result<usize, QueryError> {
        if !cache.owner.same(&self.owner_token()) {
            return Err(QueryError::WrongOwner);
        }
        let slot = cache.slot as usize;
        let entry = self
            .result_caches
            .get(slot)
            .filter(|entry| entry.is_live() && entry.generation == cache.generation)
            .ok_or(QueryError::StaleCache)?;
        if entry.fingerprint != fingerprint {
            return Err(QueryError::WrongQuery {
                detail: alloc::string::String::from("cache fingerprint does not match query spec"),
            });
        }
        Ok(slot)
    }

    fn allocate_result_cache_slot(
        &mut self,
        fingerprint: u64,
        ids: Vec<EntityId>,
    ) -> Result<usize, QueryError> {
        if let Some(slot) = self.result_caches.iter().position(|entry| !entry.is_live()) {
            self.result_caches[slot] =
                ResultCacheSlot::new(fingerprint, self.query_topology_revision, ids);
            return Ok(slot);
        }
        let slot = self.result_caches.len();
        self.result_caches.push(ResultCacheSlot::new(
            fingerprint,
            self.query_topology_revision,
            ids,
        ));
        Ok(slot)
    }

    #[cfg(any(test, feature = "testkit"))]
    pub fn invalidate_query_result_cache(&mut self, cache: &QueryResultCache) {
        self.invalidate_result_cache_handle(cache);
    }

    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn invalidate_result_cache_handle(&mut self, cache: &QueryResultCache) {
        if !cache.owner.same(&self.owner_token()) {
            return;
        }
        let Some(entry) = self.result_caches.get_mut(cache.slot as usize) else {
            return;
        };
        if entry.generation != cache.generation {
            return;
        }
        if entry.generation == u32::MAX - 1 {
            entry.generation = RETIRED_GENERATION;
        } else {
            entry.generation = 0;
        }
    }
}
