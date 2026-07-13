use crate::query::{QueryEntities, QueryError, QueryIds, QueryParams, QuerySpec};
use crate::world::World;

use super::filter::{entity_matches, validate_exact_ids};

impl World {
    pub(crate) fn entity_query_fingerprint(&mut self, spec: &QuerySpec) -> Result<u64, QueryError> {
        Ok(self.resolve_entity_plan(spec)?.fingerprint)
    }

    pub fn query_ids<'w, 'c>(
        &'w mut self,
        spec: &QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<QueryIds<'w, 'c>, QueryError> {
        let plan = self.resolve_entity_plan(spec)?;
        validate_exact_ids(self, &plan)?;
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;

        let ids = if let Some(cache) = params.result_cache {
            self.refresh_result_cache(cache, &plan, since, captured_now)?
                .to_vec()
        } else if let Some(cache) = params.membership_cache {
            let mut ids = self.refresh_membership_cache(cache, &plan)?.to_vec();
            ids.retain(|&entity| entity_matches(self, entity, &plan, since, captured_now));
            ids
        } else {
            super::collect::collect_query1_entities(self, &plan, since, captured_now)
        };

        Ok(QueryIds {
            world: self,
            ids,
            index: 0,
            exhausted: false,
            fingerprint: plan.fingerprint,
            captured_now,
            cursor_committed: false,
            cursor: params.cursor,
        })
    }

    pub fn query_entities<'w, 'c>(
        &'w mut self,
        spec: &QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<QueryEntities<'w, 'c>, QueryError> {
        Ok(QueryEntities {
            inner: self.query_ids(spec, params)?,
        })
    }
}
