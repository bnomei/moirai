use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::{Query1, QueryError, QueryParams, QuerySpec};
use crate::storage::TypedSparseStorage;
use crate::time::ChangeTick;
use crate::world::World;

use super::filter::{entity_matches, validate_exact_ids};
use super::plan::{ResolvedPlan, TraversalSource};
use super::spec::resolve_query1;

impl World {
    pub fn query_fingerprint<T: Clone + 'static>(
        &self,
        spec: &QuerySpec,
    ) -> Result<u64, QueryError> {
        Ok(resolve_query1::<T>(self, spec)?.fingerprint)
    }

    pub fn query<'w, 'c, T: Clone + 'static>(
        &'w mut self,
        spec: QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<Query1<'w, 'c, T>, QueryError> {
        let plan = resolve_query1::<T>(self, &spec)?;
        validate_exact_ids(self, &plan)?;
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;
        let cached_ids = if params.membership_cache.is_some() || params.result_cache.is_some() {
            Some(
                self.resolve_cached_entities(&params, &plan, since, captured_now)?
                    .clone(),
            )
        } else {
            None
        };
        Query1::new(self, plan, since, captured_now, params.cursor, cached_ids)
    }

    pub(crate) fn query1_state<T: Clone + 'static>(
        &self,
        plan: &ResolvedPlan,
        cached_ids: Option<Vec<EntityId>>,
    ) -> Result<crate::query::Query1State<'_, T>, QueryError> {
        if let Some(ids) = cached_ids {
            return Ok(crate::query::Query1State::Cached { ids, index: 0 });
        }
        match &plan.traversal {
            TraversalSource::Sparse { component_index } => {
                let store = self.sparse_store_by_index::<T>(*component_index)?;
                Ok(crate::query::Query1State::Sparse { store, index: 0 })
            }
            TraversalSource::Table { component_index } => {
                let archetypes = self
                    .archetypes
                    .archetypes_with_component(*component_index as u32);
                Ok(crate::query::Query1State::Table {
                    archetypes,
                    archetype_index: 0,
                    row: 0,
                })
            }
            TraversalSource::Exact { ids } => Ok(crate::query::Query1State::Exact {
                ids: ids.clone(),
                index: 0,
            }),
        }
    }

    #[allow(clippy::needless_lifetimes)]
    pub(crate) fn query1_match_sparse<'w, T: 'static>(
        &'w self,
        entity: EntityId,
        plan: &ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
        store: &'w TypedSparseStorage<T>,
    ) -> Option<&'w T> {
        if !entity_matches(self, entity, plan, since, captured_now) {
            return None;
        }
        store.get(entity)
    }

    pub(crate) fn query1_match_table<T: Clone + 'static>(
        &self,
        entity: EntityId,
        plan: &ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
    ) -> Option<&T> {
        if !entity_matches(self, entity, plan, since, captured_now) {
            return None;
        }
        self.archetypes.get_table(entity, plan.primary_index as u32)
    }

    pub(crate) fn query1_match_any_storage<T: Clone + 'static>(
        &self,
        entity: EntityId,
        plan: &ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
    ) -> Option<&T> {
        if !entity_matches(self, entity, plan, since, captured_now) {
            return None;
        }
        if plan.primary_is_table {
            self.archetypes.get_table(entity, plan.primary_index as u32)
        } else {
            self.sparse_store_by_index::<T>(plan.primary_index)
                .ok()?
                .get(entity)
        }
    }

    pub(crate) fn entity_from_slot(&self, slot: u32) -> EntityId {
        let generation = self.allocator.generation_for_slot(slot as usize);
        EntityId::from_parts(slot, generation)
    }

    pub(crate) fn archetype_entity_slots(&self, archetype: usize) -> &[u32] {
        self.archetypes.entity_slots(archetype)
    }
}
