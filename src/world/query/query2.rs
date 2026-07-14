//! Two-component query execution with mixed sparse and archetype storage.

use crate::entity::EntityId;
use crate::query::{Query2, QueryError, QueryParams, QuerySpec};
use crate::world::World;

use super::cached_source::QueryCachedSource;
use super::filter::validate_exact_ids;
use super::plan::TraversalSource;

impl World {
    #[allow(dead_code)]
    pub(crate) fn query2<'w, 'c, A: 'static, B: 'static>(
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
        let table_archetypes = table_component.map(|index| {
            self.table_archetype_cache[index]
                .as_deref()
                .expect("table archetypes prepared")
        });

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

    pub(crate) fn query_component<T: 'static>(
        &self,
        entity: EntityId,
        component_index: usize,
        is_table: bool,
    ) -> Option<&T> {
        if is_table {
            self.archetypes.get_table(entity, component_index as u32)
        } else {
            self.sparse_store_by_index::<T>(component_index)
                .ok()?
                .get(entity)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::component::ComponentOptions;
    use crate::query::{QueryParams, QuerySpec};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Sparse(i32);

    #[derive(Clone, Copy)]
    struct Table(i32);

    #[test]
    fn internal_query2_executes_uncached_membership_result_and_table_driver_paths() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        builder
            .register_component::<Table>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Sparse(2)).expect("sparse");
        world.insert(entity, Table(3)).expect("table");
        let sparse_only = world.spawn().expect("sparse only");
        world.insert(sparse_only, Sparse(7)).expect("extra sparse");
        let spec = QuerySpec::new();

        let values = world
            .query2::<Sparse, Table>(&spec, QueryParams::new())
            .expect("uncached")
            .map(|(_, sparse, table)| sparse.0 + table.0)
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(values, alloc::vec![5]);

        let membership = world
            .build_query2_cache::<Sparse, Table>(spec.clone())
            .expect("membership");
        let result = world
            .build_query2_result_cache::<Sparse, Table>(spec.clone())
            .expect("result");
        assert_eq!(
            world
                .query2::<Sparse, Table>(&spec, QueryParams::new().membership_cache(&membership),)
                .expect("membership query")
                .count(),
            1
        );
        assert_eq!(
            world
                .query2::<Sparse, Table>(&spec, QueryParams::new().result_cache(&result))
                .expect("result query")
                .count(),
            1
        );

        let sparse_index = world.component_index::<Sparse>().expect("sparse index");
        let table_index = world.component_index::<Table>().expect("table index");
        assert_eq!(
            world
                .query_component::<Sparse>(entity, sparse_index, false)
                .map(|value| value.0),
            Some(2)
        );
        assert_eq!(
            world
                .query_component::<Table>(entity, table_index, true)
                .map(|value| value.0),
            Some(3)
        );
        assert!(world
            .query_component::<Sparse>(entity, table_index, false)
            .is_none());
    }
}
