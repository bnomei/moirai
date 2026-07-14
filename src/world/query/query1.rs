use crate::entity::EntityId;
use crate::query::{Query1, QueryError, QueryParams, QuerySpec};
use crate::storage::TypedSparseStorage;
use crate::time::ChangeTick;
use crate::world::World;

use super::cached_source::QueryCachedSource;
use super::filter::{entity_matches, validate_exact_ids};
use super::plan::{ResolvedPlan, TraversalSource};

impl World {
    pub(crate) fn query_fingerprint<T: 'static>(
        &mut self,
        spec: &QuerySpec,
    ) -> Result<u64, QueryError> {
        Ok(self.resolve_query1_plan::<T>(spec)?.fingerprint)
    }

    #[allow(dead_code)]
    pub(crate) fn query<'w, 'c, T: 'static>(
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

    pub(crate) fn table_archetypes(&self, component_index: usize) -> Option<&[usize]> {
        self.table_archetype_cache
            .get(component_index)
            .and_then(|entry| entry.as_deref())
    }

    pub(crate) fn query_topology_revision(&self) -> u64 {
        self.query_topology_revision
    }

    pub(crate) fn register_query_delta_cursor(&mut self) -> alloc::rc::Rc<core::cell::Cell<u64>> {
        let cursor = alloc::rc::Rc::new(core::cell::Cell::new(self.query_delta_next_sequence));
        self.query_delta_cursors
            .push(alloc::rc::Rc::downgrade(&cursor));
        cursor
    }

    pub(crate) fn collect_query_delta_entities(
        &self,
        cursor: &core::cell::Cell<u64>,
        plan: &ResolvedPlan,
        entities: &mut alloc::vec::Vec<EntityId>,
        reverse: &mut alloc::vec::Vec<Option<usize>>,
    ) {
        for entity in entities.drain(..) {
            reverse[entity.slot() as usize] = None;
        }

        let since = cursor.get();
        let retained_start = self
            .query_delta_changes
            .front()
            .map_or(self.query_delta_next_sequence, |(sequence, _, _)| *sequence);
        // Entries are assigned contiguous sequence numbers. Convert the cursor
        // directly into a VecDeque index so a current query does not rescan a
        // long prefix retained for another, lagging query. Clamp deliberately:
        // an older cursor consumes everything retained, while a newer cursor
        // consumes nothing and is brought back to the world's current edge.
        let offset = if since <= retained_start {
            0
        } else if since >= self.query_delta_next_sequence {
            self.query_delta_changes.len()
        } else {
            usize::try_from(since - retained_start)
                .unwrap_or(self.query_delta_changes.len())
                .min(self.query_delta_changes.len())
        };
        for &(_, entity, component_index) in self.query_delta_changes.range(offset..) {
            if !plan_depends_on_component(plan, component_index) {
                continue;
            }

            let slot = entity.slot() as usize;
            if reverse.len() <= slot {
                reverse.resize(slot + 1, None);
            }
            if let Some(index) = reverse[slot] {
                // Slot reuse can put multiple generations in the retained log.
                // Only the most recent generation can determine final membership.
                entities[index] = entity;
            } else {
                reverse[slot] = Some(entities.len());
                entities.push(entity);
            }
        }
        cursor.set(self.query_delta_next_sequence);
    }

    pub(crate) fn query_component_population(
        &self,
        component_index: usize,
        is_table: bool,
    ) -> usize {
        if is_table {
            self.archetypes.component_population(component_index as u32)
        } else {
            self.sparse_dense_slots(component_index)
                .map_or(0, <[u32]>::len)
        }
    }

    pub(crate) fn query_component_topology_revision(&self, component_index: usize) -> u64 {
        self.query_component_revisions[component_index]
    }

    #[cfg(test)]
    pub(crate) fn query_delta_log_len_for_test(&self) -> usize {
        self.query_delta_changes.len()
    }

    #[cfg(test)]
    pub(crate) fn seed_query_delta_exhaustion_for_test(
        &mut self,
        cursor: &core::cell::Cell<u64>,
        entity: EntityId,
        component_index: usize,
    ) {
        cursor.set(u64::MAX - 1);
        self.query_delta_changes
            .push_back((u64::MAX - 1, entity, component_index));
        self.query_delta_next_sequence = u64::MAX;
    }

    #[cfg(test)]
    pub(crate) fn query_delta_sequences_for_test(&self) -> alloc::vec::Vec<u64> {
        self.query_delta_changes
            .iter()
            .map(|(sequence, _, _)| *sequence)
            .collect()
    }
}

fn plan_depends_on_component(plan: &ResolvedPlan, component_index: usize) -> bool {
    plan.required_indices.contains(&component_index)
        || plan.without_indices.contains(&component_index)
        || plan.with_tag_indices.contains(&component_index)
        || plan.without_tag_indices.contains(&component_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{ExactIdPolicy, QueryParams, QuerySpec};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Sparse(i32);

    #[derive(Clone, Copy)]
    struct Table(i32);

    fn world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        builder
            .register_component::<Table>(ComponentOptions::table())
            .expect("table");
        builder.build().expect("world")
    }

    #[test]
    fn internal_query1_executes_sparse_table_exact_and_cache_sources() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Sparse(3)).expect("sparse");
        world.insert(entity, Table(4)).expect("table");

        assert_eq!(
            world
                .query::<Sparse>(&QuerySpec::new(), QueryParams::new())
                .expect("sparse query")
                .map(|(_, value)| value.0)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![3]
        );
        assert_eq!(
            world
                .query::<Table>(&QuerySpec::new(), QueryParams::new())
                .expect("table query")
                .map(|(_, value)| value.0)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![4]
        );

        let exact =
            QuerySpec::new().exact_ids(alloc::vec![entity], ExactIdPolicy::ErrorOnUnavailable);
        assert_eq!(
            world
                .query::<Sparse>(&exact, QueryParams::new())
                .expect("exact query")
                .count(),
            1
        );

        let spec = QuerySpec::new();
        let membership = world
            .build_query_cache::<Sparse>(spec.clone())
            .expect("membership");
        let result = world
            .build_query_result_cache::<Sparse>(spec.clone())
            .expect("result");
        assert_eq!(
            world
                .query::<Sparse>(&spec, QueryParams::new().membership_cache(&membership))
                .expect("membership query")
                .count(),
            1
        );
        assert_eq!(
            world
                .query::<Sparse>(&spec, QueryParams::new().result_cache(&result))
                .expect("result query")
                .count(),
            1
        );
    }

    #[test]
    fn query1_private_match_helpers_cover_both_storage_kinds_and_covered_requirements() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Sparse(5)).expect("sparse");
        world.insert(entity, Table(6)).expect("table");
        let now = world.change_tick();

        let sparse_plan = world
            .resolve_query1_plan::<Sparse>(&QuerySpec::new())
            .expect("sparse plan");
        let sparse_store = world
            .sparse_store_by_index::<Sparse>(sparse_plan.primary_index)
            .expect("sparse store");
        assert_eq!(
            world
                .query1_match_sparse(entity, &sparse_plan, ChangeTick::ZERO, now, sparse_store)
                .map(|value| value.0),
            Some(5)
        );
        assert_eq!(
            world
                .query1_match_cached::<Sparse>(entity, &sparse_plan)
                .map(|value| value.0),
            Some(5)
        );
        assert_eq!(
            world
                .query1_match_any_storage::<Sparse>(entity, &sparse_plan, ChangeTick::ZERO, now,)
                .map(|value| value.0),
            Some(5)
        );

        let sparse_index = world.component_index::<Sparse>().expect("sparse index");
        let table_plan = world
            .resolve_query1_plan::<Table>(&QuerySpec::new().with::<Sparse>())
            .expect("table plan");
        assert_eq!(
            world
                .query1_match_table::<Table>(
                    entity,
                    &table_plan,
                    ChangeTick::ZERO,
                    now,
                    Some(sparse_index),
                )
                .map(|value| value.0),
            Some(6)
        );
        assert_eq!(
            world
                .query1_match_cached::<Table>(entity, &table_plan)
                .map(|value| value.0),
            Some(6)
        );
        assert_eq!(
            world
                .query1_match_any_storage::<Table>(entity, &table_plan, ChangeTick::ZERO, now,)
                .map(|value| value.0),
            Some(6)
        );

        let missing = world.spawn().expect("missing");
        assert!(world
            .query1_match_any_storage::<Sparse>(missing, &sparse_plan, ChangeTick::ZERO, now,)
            .is_none());
        assert!(world
            .query1_match_table::<Table>(
                missing,
                &table_plan,
                ChangeTick::ZERO,
                now,
                Some(sparse_index),
            )
            .is_none());

        let all_plan = world
            .resolve_entity_plan(&QuerySpec::new())
            .expect("entity plan");
        assert!(matches!(
            world.query1_state::<Sparse>(&all_plan, None, None),
            Err(QueryError::WrongQuery { .. })
        ));
    }
}
