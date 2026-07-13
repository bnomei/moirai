use crate::entity::EntityId;
use crate::query::{Query2, QueryError, QueryParams, QuerySpec};
use crate::world::World;

use super::cached_source::QueryCachedSource;
use super::filter::validate_exact_ids;
use super::plan::TraversalSource;

impl World {
    pub fn query2<'w, 'c, A: Clone + 'static, B: Clone + 'static>(
        &'w mut self,
        spec: &QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<Query2<'w, 'c, A, B>, QueryError> {
        let (plan, second_index, second_is_table) = self.resolve_query2_plan::<A, B>(spec)?;
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
        let table_archetypes =
            table_component.map(|index| self.table_archetype_cache[index].as_slice());

        Query2::new(
            self,
            plan,
            since,
            captured_now,
            params.cursor,
            cached,
            table_archetypes,
            second_index,
            second_is_table,
        )
    }

    pub(crate) fn query2_second<B: Clone + 'static>(
        &self,
        entity: EntityId,
        second_index: usize,
        second_is_table: bool,
    ) -> Option<&B> {
        if second_is_table {
            self.archetypes.get_table(entity, second_index as u32)
        } else {
            self.sparse_store_by_index::<B>(second_index)
                .ok()?
                .get(entity)
        }
    }
}
