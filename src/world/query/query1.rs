use crate::entity::EntityId;
use crate::query::{Query1, QueryError, QueryParams, QuerySpec};
use crate::storage::TypedSparseStorage;
use crate::time::ChangeTick;
use crate::world::World;

use super::cached_source::QueryCachedSource;
use super::filter::{entity_matches, validate_exact_ids};
use super::plan::{ResolvedPlan, TraversalSource};

impl World {
    pub fn query_fingerprint<T: 'static>(&mut self, spec: &QuerySpec) -> Result<u64, QueryError> {
        Ok(self.resolve_query1_plan::<T>(spec)?.fingerprint)
    }

    pub fn query<'w, 'c, T: 'static>(
        &'w mut self,
        spec: &QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<Query1<'w, 'c, T>, QueryError> {
        let plan = self.resolve_query1_plan::<T>(spec)?;
        validate_exact_ids(self, &plan)?;
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;

        let table_component = match &plan.traversal {
            TraversalSource::Table { component_index } => Some(*component_index),
            _ => None,
        };

        let cached = if let Some(cache) = params.result_cache {
            self.refresh_result_cache(cache, &plan, since, captured_now)?;
            Some(QueryCachedSource::Result(cache.clone()))
        } else if let Some(cache) = params.membership_cache {
            self.refresh_membership_cache(cache, &plan)?;
            Some(QueryCachedSource::Membership(cache.clone()))
        } else {
            None
        };

        if let Some(component_index) = table_component {
            self.ensure_table_archetypes(component_index);
        }
        let table_archetypes = table_component.map(|index| {
            self.table_archetype_cache[index]
                .as_deref()
                .expect("table archetypes prepared")
        });

        Query1::new(
            self,
            plan,
            since,
            captured_now,
            params.cursor,
            cached,
            table_archetypes,
            None,
        )
    }

    pub(crate) fn query1_state<'w, T: 'static>(
        &'w self,
        plan: &ResolvedPlan,
        cached: Option<QueryCachedSource>,
        table_archetypes: Option<&'w [usize]>,
    ) -> Result<crate::query::Query1State<'w, T>, QueryError> {
        if let Some(source) = cached {
            return Ok(crate::query::Query1State::Cached { source, index: 0 });
        }
        match &plan.traversal {
            TraversalSource::All => Err(QueryError::WrongQuery {
                detail: alloc::string::String::from("entity-only plan cannot back a typed query"),
            }),
            TraversalSource::Sparse { component_index } => {
                let store = self.sparse_store_by_index::<T>(*component_index)?;
                Ok(crate::query::Query1State::Sparse { store, index: 0 })
            }
            TraversalSource::Table { .. } => {
                let archetypes = table_archetypes.expect("table archetypes prepared");
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

    pub(crate) fn query1_accept_source_covered(
        &self,
        entity: EntityId,
        plan: &ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
        additional_covered_required: usize,
    ) -> bool {
        let primary = plan.primary_index;
        super::filter::entity_matches_with_covered(
            self,
            entity,
            plan,
            since,
            captured_now,
            &[primary, additional_covered_required],
        )
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

    pub(crate) fn query1_match_table<T: 'static>(
        &self,
        entity: EntityId,
        plan: &ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
        additional_covered_required: Option<usize>,
    ) -> Option<&T> {
        let matches = if let Some(additional) = additional_covered_required {
            self.query1_accept_source_covered(entity, plan, since, captured_now, additional)
        } else {
            entity_matches(self, entity, plan, since, captured_now)
        };
        if !matches {
            return None;
        }
        self.archetypes.get_table(entity, plan.primary_index as u32)
    }

    pub(crate) fn query1_match_cached<T: 'static>(
        &self,
        entity: EntityId,
        plan: &ResolvedPlan,
    ) -> Option<&T> {
        if plan.primary_is_table {
            self.archetypes.get_table(entity, plan.primary_index as u32)
        } else {
            self.sparse_store_by_index::<T>(plan.primary_index)
                .ok()?
                .get(entity)
        }
    }

    pub(crate) fn query1_match_any_storage<T: 'static>(
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
        EntityId::from_owned_parts(self.owner_token().token(), slot, generation)
    }

    pub(crate) fn archetype_entity_slots(&self, archetype: usize) -> &[u32] {
        self.archetypes.entity_slots(archetype)
    }
}
