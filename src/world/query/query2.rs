use crate::entity::EntityId;
use crate::query::{Query2, QueryError, QueryParams, QuerySpec};
use crate::world::World;

use super::filter::validate_exact_ids;
use super::spec::resolve_query2;

impl World {
    pub fn query2<'w, 'c, A: Clone + 'static, B: Clone + 'static>(
        &'w mut self,
        spec: QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<Query2<'w, 'c, A, B>, QueryError> {
        let (plan, second_index, second_is_table) = resolve_query2::<A, B>(self, &spec)?;
        validate_exact_ids(self, &plan)?;
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;
        let cached_ids = if params.membership_cache.is_some() || params.result_cache.is_some() {
            let members = self
                .resolve_cached_entities(&params, &plan, since, captured_now)?
                .clone();
            Some(
                members
                    .into_iter()
                    .filter(|&entity| {
                        self.entity_has_query2_second(entity, second_index, second_is_table)
                    })
                    .collect(),
            )
        } else {
            None
        };
        Query2::new(
            self,
            plan,
            since,
            captured_now,
            params.cursor,
            cached_ids,
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
