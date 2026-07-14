use crate::query::{QueryEntities, QueryError, QueryIds, QueryParams, QuerySpec};
use crate::world::World;

use super::filter::{entity_matches, validate_exact_ids};

impl World {
    pub(crate) fn entity_query_fingerprint(&mut self, spec: &QuerySpec) -> Result<u64, QueryError> {
        Ok(self.resolve_entity_plan(spec)?.fingerprint)
    }

    pub(crate) fn query_ids<'w, 'c>(
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

    pub(crate) fn query_entities<'w, 'c>(
        &'w mut self,
        spec: &QuerySpec,
        params: QueryParams<'c>,
    ) -> Result<QueryEntities<'w, 'c>, QueryError> {
        Ok(QueryEntities {
            inner: self.query_ids(spec, params)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{ExactIdPolicy, QueryCursor};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Position(i32);

    #[derive(Clone, Copy)]
    struct Player;

    fn build_world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("position");
        builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("player");
        builder.build().expect("world")
    }

    #[test]
    fn internal_entity_queries_cover_live_ids_tags_and_optional_reads() {
        let mut world = build_world();
        let empty = world.spawn().expect("empty");
        let tagged = world.spawn().expect("tagged");
        world.insert(tagged, Position(7)).expect("position");
        world.insert(tagged, Player).expect("tag");

        assert_eq!(
            world
                .query_ids(&QuerySpec::new(), QueryParams::new())
                .expect("ids")
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![empty, tagged]
        );
        let refs: alloc::vec::Vec<_> = world
            .query_entities(&QuerySpec::new().with_tag::<Player>(), QueryParams::new())
            .expect("tagged refs")
            .collect();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id(), tagged);
        assert!(refs[0].has::<Player>().expect("tag membership"));
        assert_eq!(
            refs[0].get::<Position>().expect("position").map(|p| p.0),
            Some(7)
        );
    }

    #[test]
    fn internal_exact_ids_preserve_order_and_validate_error_and_owner() {
        let mut world = build_world();
        let first = world.spawn().expect("first");
        let second = world.spawn().expect("second");
        let stale = world.spawn().expect("stale");
        world.despawn(stale).expect("despawn");

        assert_eq!(
            world
                .query_ids(
                    &QuerySpec::new().exact_ids(
                        alloc::vec![second, first],
                        ExactIdPolicy::ErrorOnUnavailable
                    ),
                    QueryParams::new(),
                )
                .expect("ordered")
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![second, first]
        );
        assert!(matches!(
            world.query_ids(
                &QuerySpec::new().exact_ids(
                    alloc::vec![stale],
                    ExactIdPolicy::ErrorOnUnavailable,
                ),
                QueryParams::new(),
            ),
            Err(QueryError::MissingExactId { entity }) if entity == stale
        ));

        let mut foreign = build_world();
        let foreign_id = foreign.spawn().expect("foreign");
        assert!(matches!(
            world.query_ids(
                &QuerySpec::new()
                    .exact_ids(alloc::vec![foreign_id], ExactIdPolicy::SkipUnavailable),
                QueryParams::new(),
            ),
            Err(QueryError::WrongOwner)
        ));
    }

    #[test]
    fn internal_entity_cursor_commits_only_after_full_iteration() {
        let mut world = build_world();
        let first = world.spawn().expect("first");
        let second = world.spawn().expect("second");
        world.insert(first, Position(1)).expect("first position");
        world.insert(second, Position(2)).expect("second position");
        let spec = QuerySpec::new().added::<Position>();
        let mut cursor = QueryCursor::for_entities_from_start(&mut world, &spec).expect("cursor");
        let before = cursor.since();

        {
            let mut ids = world
                .query_ids(&spec, QueryParams::new().cursor(&mut cursor))
                .expect("partial");
            assert_eq!(ids.next(), Some(first));
        }
        assert_eq!(cursor.since(), before);

        world
            .query_ids(&spec, QueryParams::new().cursor(&mut cursor))
            .expect("complete")
            .for_each(drop);
        assert!(cursor.since() > before);
    }
}
