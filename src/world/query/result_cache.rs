//! Fully materialized query result cache slots.
//!
//! Stores filtered entity ids for static queries without added/changed windows.

#![allow(dead_code)]

use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::{QueryError, QueryResultCache, QuerySpec};
use crate::world::World;

use super::cache::QueryTopologySnapshot;
use super::collect::collect_query1_entities;
use super::filter::validate_exact_ids;

const RETIRED_GENERATION: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub(crate) struct ResultCacheSlot {
    pub generation: u32,
    pub fingerprint: u64,
    pub topology: QueryTopologySnapshot,
    pub ids: Vec<EntityId>,
}

impl ResultCacheSlot {
    fn new(fingerprint: u64, topology: QueryTopologySnapshot, ids: Vec<EntityId>) -> Self {
        Self {
            generation: 1,
            fingerprint,
            topology,
            ids,
        }
    }

    fn is_live(&self) -> bool {
        self.generation != 0 && self.generation != RETIRED_GENERATION
    }
}

impl World {
    pub(crate) fn build_entity_query_result_cache(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryResultCache, QueryError> {
        let exact_plan = if spec.exact_ids.is_some() {
            let plan = self.resolve_entity_plan(&spec)?;
            validate_exact_ids(self, &plan)?;
            Some(plan)
        } else {
            None
        };
        if !spec.added.is_empty()
            || !spec.added_ids.is_empty()
            || !spec.changed.is_empty()
            || !spec.changed_ids.is_empty()
        {
            return Err(QueryError::MovingChangeWindow);
        }
        if exact_plan.is_some() {
            return Err(QueryError::ExactIdOrderConflict);
        }
        let plan = self.resolve_entity_plan(&spec)?;
        let captured_now = self.change_tick();
        let ids = collect_query1_entities(self, &plan, crate::time::ChangeTick::ZERO, captured_now);
        let topology = QueryTopologySnapshot::capture(self, &plan);
        let slot = self.allocate_result_cache_slot(plan.fingerprint, topology, ids)?;
        Ok(QueryResultCache {
            owner: self.owner_token(),
            slot: slot as u32,
            generation: self.result_cache_slot(slot).generation,
        })
    }

    pub(crate) fn build_query_result_cache<T: 'static>(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryResultCache, QueryError> {
        let exact_plan = if spec.exact_ids.is_some() {
            let plan = self.resolve_query1_plan::<T>(&spec)?;
            validate_exact_ids(self, &plan)?;
            Some(plan)
        } else {
            None
        };
        if !spec.added.is_empty()
            || !spec.added_ids.is_empty()
            || !spec.changed.is_empty()
            || !spec.changed_ids.is_empty()
        {
            return Err(QueryError::MovingChangeWindow);
        }
        if exact_plan.is_some() {
            return Err(QueryError::ExactIdOrderConflict);
        }
        let plan = self.resolve_query1_plan::<T>(&spec)?;
        let captured_now = self.change_tick();
        let ids = collect_query1_entities(self, &plan, crate::time::ChangeTick::ZERO, captured_now);
        let topology = QueryTopologySnapshot::capture(self, &plan);
        let slot = self.allocate_result_cache_slot(plan.fingerprint, topology, ids)?;
        Ok(QueryResultCache {
            owner: self.owner_token(),
            slot: slot as u32,
            generation: self.result_cache_slot(slot).generation,
        })
    }

    pub(crate) fn build_query2_result_cache<A: 'static, B: 'static>(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryResultCache, QueryError> {
        let exact_plan = if spec.exact_ids.is_some() {
            let (plan, _, _) = self.resolve_query2_plan::<A, B>(&spec)?;
            validate_exact_ids(self, &plan)?;
            Some(plan)
        } else {
            None
        };
        if !spec.added.is_empty()
            || !spec.added_ids.is_empty()
            || !spec.changed.is_empty()
            || !spec.changed_ids.is_empty()
        {
            return Err(QueryError::MovingChangeWindow);
        }
        if exact_plan.is_some() {
            return Err(QueryError::ExactIdOrderConflict);
        }
        let (plan, second_index, second_is_table) = self.resolve_query2_plan::<A, B>(&spec)?;
        let captured_now = self.change_tick();
        let ids = super::collect::collect_query2_entities(
            self,
            &plan,
            crate::time::ChangeTick::ZERO,
            captured_now,
            second_index,
            second_is_table,
        );
        let topology = QueryTopologySnapshot::capture(self, &plan);
        let slot = self.allocate_result_cache_slot(plan.fingerprint, topology, ids)?;
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
        let global_revision = self.query_topology_revision;
        let observed_global_revision = self.result_caches[slot].topology.observed_global_revision();
        let needs_refresh = if observed_global_revision == global_revision {
            false
        } else if self.result_caches[slot]
            .topology
            .dependencies_are_current(self)
        {
            self.result_caches[slot]
                .topology
                .observe_global_revision(global_revision);
            false
        } else {
            true
        };
        if needs_refresh {
            let ids = collect_query1_entities(self, plan, since, captured_now);
            let topology = QueryTopologySnapshot::capture(self, plan);
            let entry = &mut self.result_caches[slot];
            entry.ids = ids;
            entry.topology = topology;
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
        topology: QueryTopologySnapshot,
        ids: Vec<EntityId>,
    ) -> Result<usize, QueryError> {
        if let Some(slot) = self.result_caches.iter().position(|entry| !entry.is_live()) {
            self.result_caches[slot] = ResultCacheSlot::new(fingerprint, topology, ids);
            return Ok(slot);
        }
        let slot = self.result_caches.len();
        self.result_caches
            .push(ResultCacheSlot::new(fingerprint, topology, ids));
        Ok(slot)
    }

    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn invalidate_query_result_cache(&mut self, cache: &QueryResultCache) {
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

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{QueryParams, QuerySpec};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Pos(i32);

    fn world_with_entity() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        world
    }

    #[test]
    fn entity_result_cache_builds_and_rejects_temporal_and_exact_specs() {
        let mut world = world_with_entity();
        let cache = world
            .build_entity_query_result_cache(QuerySpec::new().with::<Pos>())
            .expect("entity result cache");
        assert_eq!(world.result_cache_slot(cache.slot as usize).ids.len(), 1);

        assert!(matches!(
            world.build_entity_query_result_cache(QuerySpec::new().added::<Pos>()),
            Err(QueryError::MovingChangeWindow)
        ));

        let entity = world.spawn().expect("spawn");
        let exact =
            QuerySpec::new().exact_ids(vec![entity], crate::query::ExactIdPolicy::SkipUnavailable);
        assert!(matches!(
            world.build_entity_query_result_cache(exact),
            Err(QueryError::ExactIdOrderConflict)
        ));
    }

    #[test]
    fn typed_result_cache_rejects_exact_order_and_builds_query2_results() {
        #[derive(Clone, Copy)]
        struct Vel;

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::sparse())
            .expect("vel");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel).expect("vel");

        let exact =
            QuerySpec::new().exact_ids(vec![entity], crate::query::ExactIdPolicy::SkipUnavailable);
        assert!(matches!(
            world.build_query_result_cache::<Pos>(exact),
            Err(QueryError::ExactIdOrderConflict)
        ));

        let cache = world
            .build_query2_result_cache::<Pos, Vel>(QuerySpec::new())
            .expect("query2 result cache");
        assert_eq!(
            world.result_cache_slot(cache.slot as usize).ids,
            vec![entity]
        );
    }

    #[test]
    fn result_cache_distinguishes_irrelevant_topology_changes() {
        #[derive(Clone, Copy)]
        struct Vel;

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::sparse())
            .expect("vel");
        let mut world = builder.build().expect("build");
        let first = world.spawn().expect("first");
        world.insert(first, Pos(1)).expect("pos");
        let unrelated = world.spawn().expect("unrelated");

        let spec = QuerySpec::new();
        let cache = world
            .build_query_result_cache::<Pos>(spec.clone())
            .expect("cache");
        let plan = world.resolve_query1_plan::<Pos>(&spec).expect("plan");
        let original_ids = world.result_cache_slot(cache.slot as usize).ids.clone();
        world.insert(unrelated, Vel).expect("irrelevant topology");
        let irrelevant_revision = world.query_topology_revision;

        assert_eq!(
            world
                .refresh_result_cache(
                    &cache,
                    &plan,
                    crate::time::ChangeTick::ZERO,
                    world.change_tick(),
                )
                .expect("refresh"),
            original_ids.as_slice()
        );
        assert_eq!(
            world
                .result_cache_slot(cache.slot as usize)
                .topology
                .observed_global_revision(),
            irrelevant_revision
        );
    }

    #[test]
    fn result_cache_validation_rejects_foreign_owner() {
        let mut world = world_with_entity();
        let cache = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("cache");
        let foreign = QueryResultCache {
            owner: crate::world::WorldOwner::new(),
            slot: cache.slot,
            generation: cache.generation,
        };
        let fingerprint = world.result_cache_slot(cache.slot as usize).fingerprint;

        assert!(matches!(
            world.validate_result_cache(&foreign, fingerprint),
            Err(QueryError::WrongOwner)
        ));
    }

    #[test]
    fn allocate_reuses_retired_slot() {
        let mut world = world_with_entity();
        let first = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("first");
        world.invalidate_query_result_cache(&first);
        let second = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("second");
        assert_eq!(first.slot, second.slot);
        assert!(second.generation > 0);
    }

    #[test]
    fn validate_rejects_stale_generation() {
        let mut world = world_with_entity();
        let cache = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("cache");
        world.invalidate_query_result_cache(&cache);
        let plan = world
            .resolve_query1_plan::<Pos>(&QuerySpec::new())
            .expect("plan");
        assert!(matches!(
            world.refresh_result_cache(
                &cache,
                &plan,
                crate::time::ChangeTick::ZERO,
                world.change_tick()
            ),
            Err(QueryError::StaleCache)
        ));
    }

    #[test]
    fn refresh_updates_on_topology_revision_change() {
        let mut world = world_with_entity();
        let spec = QuerySpec::new();
        let cache = world
            .build_query_result_cache::<Pos>(spec.clone())
            .expect("cache");
        let plan = world.resolve_query1_plan::<Pos>(&spec).expect("plan");
        let before = world
            .refresh_result_cache(
                &cache,
                &plan,
                crate::time::ChangeTick::ZERO,
                world.change_tick(),
            )
            .expect("before")
            .len();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(2)).expect("insert");
        let plan = world.resolve_query1_plan::<Pos>(&spec).expect("plan");
        let after = world
            .refresh_result_cache(
                &cache,
                &plan,
                crate::time::ChangeTick::ZERO,
                world.change_tick(),
            )
            .expect("after")
            .len();
        assert_eq!(before, 1);
        assert_eq!(after, 2);
    }

    #[test]
    fn build_rejects_moving_change_window() {
        let mut world = world_with_entity();
        let spec = QuerySpec::new().added::<Pos>();
        assert!(matches!(
            world.build_query_result_cache::<Pos>(spec),
            Err(QueryError::MovingChangeWindow)
        ));
    }

    #[test]
    fn build_query2_rejects_moving_change_window_and_exact_ids() {
        let mut world = world_with_entity();
        assert!(matches!(
            world.build_query2_result_cache::<Pos, Pos>(QuerySpec::new().changed::<Pos>()),
            Err(QueryError::MovingChangeWindow)
        ));
        let entity = world.spawn().expect("spawn");
        assert!(matches!(
            world.build_query2_result_cache::<Pos, Pos>(
                QuerySpec::new()
                    .exact_ids(vec![entity], crate::query::ExactIdPolicy::SkipUnavailable,)
            ),
            Err(QueryError::ExactIdOrderConflict)
        ));
    }

    #[test]
    fn validate_rejects_fingerprint_mismatch() {
        let mut world = world_with_entity();
        let cache = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("cache");
        let wrong = world.result_caches[cache.slot as usize]
            .fingerprint
            .wrapping_add(1);
        assert!(matches!(
            world.validate_result_cache(&cache, wrong),
            Err(QueryError::WrongQuery { .. })
        ));
    }

    #[test]
    fn invalidate_guards_skip_wrong_owner_missing_slot_and_stale_generation() {
        let mut world = world_with_entity();
        let cache = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("cache");
        let other_owner = crate::world::WorldOwner::new();
        let foreign = QueryResultCache {
            owner: other_owner,
            slot: cache.slot,
            generation: cache.generation,
        };
        world.invalidate_result_cache_handle(&foreign);
        assert!(world.result_caches[cache.slot as usize].is_live());

        let missing_slot = QueryResultCache {
            owner: world.owner_token(),
            slot: 99,
            generation: 1,
        };
        world.invalidate_result_cache_handle(&missing_slot);

        world.invalidate_result_cache_handle(&cache);
        world.invalidate_result_cache_handle(&cache);
        assert!(!world.result_caches[cache.slot as usize].is_live());
    }

    #[test]
    fn invalidate_retires_generation_at_max_minus_one() {
        let mut world = world_with_entity();
        let cache = world
            .build_query_result_cache::<Pos>(QuerySpec::new())
            .expect("cache");
        world.result_caches[cache.slot as usize].generation = u32::MAX - 1;
        let cache = QueryResultCache {
            owner: cache.owner,
            slot: cache.slot,
            generation: u32::MAX - 1,
        };
        world.invalidate_result_cache_handle(&cache);
        assert_eq!(
            world.result_caches[cache.slot as usize].generation,
            RETIRED_GENERATION
        );
    }

    #[test]
    fn public_query_path_uses_cached_ids() {
        let mut world = world_with_entity();
        let spec = QuerySpec::new();
        let cache = world
            .build_query_result_cache::<Pos>(spec.clone())
            .expect("cache");
        let values: Vec<_> = world
            .query::<Pos>(&spec, QueryParams::new().result_cache(&cache))
            .expect("query")
            .map(|(_, p)| p.0)
            .collect();
        assert_eq!(values, vec![1]);
    }
}
