use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::{QueryCache, QueryError, QueryParams, QuerySpec};
use crate::world::World;

use super::collect::collect_query1_structural_members;
use super::filter::validate_exact_ids;
use super::plan::{ResolvedPlan, TraversalSource};

const RETIRED_GENERATION: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub(crate) struct MembershipCacheSlot {
    pub generation: u32,
    pub fingerprint: u64,
    pub topology: QueryTopologySnapshot,
    pub members: Vec<EntityId>,
}

impl MembershipCacheSlot {
    fn new(fingerprint: u64, topology: QueryTopologySnapshot, members: Vec<EntityId>) -> Self {
        Self {
            generation: 1,
            fingerprint,
            topology,
            members,
        }
    }

    fn is_live(&self) -> bool {
        self.generation != 0 && self.generation != RETIRED_GENERATION
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QueryTopologySnapshot {
    global_revision: u64,
    entity_revision: Option<u64>,
    component_revisions: Vec<(usize, u64)>,
}

impl QueryTopologySnapshot {
    pub(crate) fn capture(world: &World, plan: &ResolvedPlan) -> Self {
        let entity_revision =
            matches!(plan.traversal, TraversalSource::All).then_some(world.query_entity_revision);
        let mut components = Vec::new();
        components.extend_from_slice(&plan.required_indices);
        components.extend_from_slice(&plan.without_indices);
        components.extend_from_slice(&plan.with_tag_indices);
        components.extend_from_slice(&plan.without_tag_indices);
        if let TraversalSource::Sparse { component_index }
        | TraversalSource::Table { component_index } = plan.traversal
        {
            components.push(component_index);
        }
        components.sort_unstable();
        components.dedup();
        let component_revisions = components
            .into_iter()
            .map(|index| (index, world.query_component_revisions[index]))
            .collect();
        Self {
            global_revision: world.query_topology_revision,
            entity_revision,
            component_revisions,
        }
    }

    pub(crate) fn observed_global_revision(&self) -> u64 {
        self.global_revision
    }

    pub(crate) fn observe_global_revision(&mut self, revision: u64) {
        self.global_revision = revision;
    }

    pub(crate) fn dependencies_are_current(&self, world: &World) -> bool {
        self.entity_revision
            .map_or(true, |revision| revision == world.query_entity_revision)
            && self.component_revisions.iter().all(|&(index, revision)| {
                world.query_component_revisions.get(index).copied() == Some(revision)
            })
    }
}

impl World {
    pub fn build_entity_query_cache(&mut self, spec: QuerySpec) -> Result<QueryCache, QueryError> {
        let plan = self.resolve_entity_plan(&spec)?;
        if spec.exact_ids.is_some() {
            validate_exact_ids(self, &plan)?;
            return Err(QueryError::UnsupportedCachePolicy {
                detail: alloc::string::String::from(
                    "membership cache does not support exact-id specs",
                ),
            });
        }
        let members = collect_query1_structural_members(self, &plan);
        let topology = QueryTopologySnapshot::capture(self, &plan);
        let slot = self.allocate_membership_cache_slot(plan.fingerprint, topology, members)?;
        Ok(QueryCache {
            owner: self.owner_token(),
            slot: slot as u32,
            generation: self.membership_cache_slot(slot).generation,
        })
    }

    pub fn build_query_cache<T: 'static>(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryCache, QueryError> {
        let plan = self.resolve_query1_plan::<T>(&spec)?;
        if spec.exact_ids.is_some() {
            validate_exact_ids(self, &plan)?;
            return Err(QueryError::UnsupportedCachePolicy {
                detail: alloc::string::String::from(
                    "membership cache does not support exact-id specs",
                ),
            });
        }
        let members = collect_query1_structural_members(self, &plan);
        let topology = QueryTopologySnapshot::capture(self, &plan);
        let slot = self.allocate_membership_cache_slot(plan.fingerprint, topology, members)?;
        Ok(QueryCache {
            owner: self.owner_token(),
            slot: slot as u32,
            generation: self.membership_cache_slot(slot).generation,
        })
    }

    pub fn build_query2_cache<A: 'static, B: 'static>(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryCache, QueryError> {
        let (plan, _, _) = self.resolve_query2_plan::<A, B>(&spec)?;
        if spec.exact_ids.is_some() {
            validate_exact_ids(self, &plan)?;
            return Err(QueryError::UnsupportedCachePolicy {
                detail: alloc::string::String::from(
                    "membership cache does not support exact-id specs",
                ),
            });
        }
        let members = collect_query1_structural_members(self, &plan);
        let topology = QueryTopologySnapshot::capture(self, &plan);
        let slot = self.allocate_membership_cache_slot(plan.fingerprint, topology, members)?;
        Ok(QueryCache {
            owner: self.owner_token(),
            slot: slot as u32,
            generation: self.membership_cache_slot(slot).generation,
        })
    }

    pub(crate) fn membership_cache_slot(&self, slot: usize) -> &MembershipCacheSlot {
        &self.membership_caches[slot]
    }

    pub(crate) fn refresh_membership_cache(
        &mut self,
        cache: &QueryCache,
        plan: &super::plan::ResolvedPlan,
    ) -> Result<&[EntityId], QueryError> {
        let slot = self.validate_membership_cache(cache, plan.fingerprint)?;
        let global_revision = self.query_topology_revision;
        let observed_global_revision = self.membership_caches[slot]
            .topology
            .observed_global_revision();
        let needs_refresh = if observed_global_revision == global_revision {
            false
        } else if self.membership_caches[slot]
            .topology
            .dependencies_are_current(self)
        {
            self.membership_caches[slot]
                .topology
                .observe_global_revision(global_revision);
            false
        } else {
            true
        };
        if needs_refresh {
            let members = collect_query1_structural_members(self, plan);
            let topology = QueryTopologySnapshot::capture(self, plan);
            let entry = &mut self.membership_caches[slot];
            entry.members = members;
            entry.topology = topology;
        }
        Ok(&self.membership_caches[slot].members)
    }

    pub(crate) fn validate_membership_cache(
        &self,
        cache: &QueryCache,
        fingerprint: u64,
    ) -> Result<usize, QueryError> {
        if !cache.owner.same(&self.owner_token()) {
            return Err(QueryError::WrongOwner);
        }
        let slot = cache.slot as usize;
        let entry = self
            .membership_caches
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

    fn allocate_membership_cache_slot(
        &mut self,
        fingerprint: u64,
        topology: QueryTopologySnapshot,
        members: Vec<EntityId>,
    ) -> Result<usize, QueryError> {
        if let Some(slot) = self
            .membership_caches
            .iter()
            .position(|entry| !entry.is_live())
        {
            self.membership_caches[slot] = MembershipCacheSlot::new(fingerprint, topology, members);
            return Ok(slot);
        }
        let slot = self.membership_caches.len();
        self.membership_caches
            .push(MembershipCacheSlot::new(fingerprint, topology, members));
        Ok(slot)
    }

    #[cfg(any(test, feature = "testkit"))]
    pub fn invalidate_query_cache(&mut self, cache: &QueryCache) {
        self.invalidate_membership_cache_handle(cache);
    }

    #[cfg(test)]
    pub(crate) fn set_membership_cache_generation_for_test(
        &mut self,
        cache: &QueryCache,
        generation: u32,
    ) {
        self.membership_caches[cache.slot as usize].generation = generation;
    }

    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn invalidate_membership_cache_handle(&mut self, cache: &QueryCache) {
        if !cache.owner.same(&self.owner_token()) {
            return;
        }
        let Some(entry) = self.membership_caches.get_mut(cache.slot as usize) else {
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

    pub(crate) fn validate_query_params_caches(
        &self,
        params: &QueryParams<'_>,
        plan: &super::plan::ResolvedPlan,
    ) -> Result<(), QueryError> {
        if params.membership_cache.is_some() && params.result_cache.is_some() {
            return Err(QueryError::UnsupportedCachePolicy {
                detail: alloc::string::String::from(
                    "membership and result caches are mutually exclusive",
                ),
            });
        }
        if let Some(cache) = params.result_cache {
            if !plan.added_indices.is_empty() || !plan.changed_indices.is_empty() {
                return Err(QueryError::MovingChangeWindow);
            }
            if matches!(plan.traversal, super::plan::TraversalSource::Exact { .. }) {
                return Err(QueryError::ExactIdOrderConflict);
            }
            self.validate_result_cache(cache, plan.fingerprint)?;
        }
        if let Some(cache) = params.membership_cache {
            self.validate_membership_cache(cache, plan.fingerprint)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::event::EventOptions;
    use crate::operation::StageOperation;
    use crate::query::{QueryParams, QuerySpec};
    use crate::world::WorldBuilder;
    use alloc::vec;

    #[derive(Clone, Copy)]
    struct Position(#[allow(dead_code)] i32);

    #[derive(Clone, Copy)]
    struct FrameEvent(#[allow(dead_code)] u8);

    #[test]
    fn stale_cache_handle_is_rejected() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let cache = world
            .build_query_cache::<Position>(QuerySpec::new())
            .expect("cache");
        world.invalidate_query_cache(&cache);
        let params = QueryParams::new().membership_cache(&cache);
        assert!(matches!(
            world.query::<Position>(&QuerySpec::new(), params),
            Err(QueryError::StaleCache)
        ));
    }

    #[test]
    fn user_event_clear_does_not_break_cache_coherence() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        builder
            .add_event::<FrameEvent>(EventOptions::frame(StageOperation::Update))
            .expect("event");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");

        let spec = QuerySpec::new();
        let cache = world
            .build_query_cache::<Position>(spec.clone())
            .expect("cache");
        let params = QueryParams::new().membership_cache(&cache);

        world.send(FrameEvent(1)).expect("send");
        world.clear_frame_events(StageOperation::Update);

        let count = world
            .query::<Position>(&spec, params)
            .expect("query")
            .count();
        assert_eq!(count, 1);
    }

    #[derive(Clone, Copy)]
    struct Velocity(#[allow(dead_code)] i32);

    #[test]
    fn membership_cache_rejects_exact_id_specs() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        builder
            .register_component::<Velocity>(ComponentOptions::sparse())
            .expect("register velocity");
        let mut world = builder.build().expect("build");
        let spec = QuerySpec::new().exact_ids(vec![], crate::query::ExactIdPolicy::SkipUnavailable);
        assert!(matches!(
            world.build_query_cache::<Position>(spec),
            Err(QueryError::UnsupportedCachePolicy { .. })
        ));
        assert!(matches!(
            world.build_query2_cache::<Position, Velocity>(
                QuerySpec::new().exact_ids(vec![], crate::query::ExactIdPolicy::SkipUnavailable,)
            ),
            Err(QueryError::UnsupportedCachePolicy { .. })
        ));
    }

    #[test]
    fn membership_cache_rejects_fingerprint_mismatch() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        builder
            .register_component::<Velocity>(ComponentOptions::sparse())
            .expect("register velocity");
        let mut world = builder.build().expect("build");
        let cache = world
            .build_query_cache::<Position>(QuerySpec::new())
            .expect("cache");
        let other = QuerySpec::new().without::<Velocity>();
        assert!(matches!(
            world.query::<Position>(&other, QueryParams::new().membership_cache(&cache)),
            Err(QueryError::WrongQuery { .. })
        ));
    }

    #[test]
    fn membership_cache_reuses_retired_slot() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let first = world
            .build_query_cache::<Position>(QuerySpec::new())
            .expect("first");
        world.invalidate_query_cache(&first);
        let second = world
            .build_query_cache::<Position>(QuerySpec::new())
            .expect("second");
        assert_eq!(first.slot, second.slot);
    }

    #[test]
    fn invalidate_ignores_foreign_owner_and_stale_generation() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let cache = world
            .build_query_cache::<Position>(QuerySpec::new())
            .expect("cache");
        let foreign = QueryCache {
            owner: crate::world::WorldOwner::new(),
            slot: cache.slot,
            generation: cache.generation,
        };
        world.invalidate_query_cache(&foreign);
        let mut stale = cache.clone();
        stale.generation = cache.generation.wrapping_add(1);
        world.invalidate_query_cache(&stale);
        assert!(world
            .query::<Position>(
                &QuerySpec::new(),
                QueryParams::new().membership_cache(&cache)
            )
            .is_ok());
    }

    #[test]
    fn validate_query_params_rejects_dual_caches_and_moving_window() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");
        let spec = QuerySpec::new();
        let membership = world
            .build_query_cache::<Position>(spec.clone())
            .expect("membership");
        let result = world
            .build_query_result_cache::<Position>(spec.clone())
            .expect("result");
        let plan = world.resolve_query1_plan::<Position>(&spec).expect("plan");
        let dual = QueryParams {
            since: None,
            cursor: None,
            membership_cache: Some(&membership),
            result_cache: Some(&result),
        };
        assert!(matches!(
            world.validate_query_params_caches(&dual, &plan),
            Err(QueryError::UnsupportedCachePolicy { .. })
        ));

        let added = QuerySpec::new().added::<Position>();
        let added_plan = world
            .resolve_query1_plan::<Position>(&added)
            .expect("added plan");
        assert!(matches!(
            world.validate_query_params_caches(
                &QueryParams::new().result_cache(&result),
                &added_plan
            ),
            Err(QueryError::MovingChangeWindow)
        ));
    }

    #[test]
    fn invalidate_ignores_missing_cache_slot() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let missing = QueryCache {
            owner: world.owner_token(),
            slot: 99,
            generation: 1,
        };
        world.invalidate_query_cache(&missing);
    }

    #[test]
    fn invalidate_retires_generation_at_max_minus_one() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let cache = world
            .build_query_cache::<Position>(QuerySpec::new())
            .expect("cache");
        world.set_membership_cache_generation_for_test(&cache, u32::MAX - 1);
        let mut retiring = cache.clone();
        retiring.generation = u32::MAX - 1;
        world.invalidate_query_cache(&retiring);
        assert_eq!(
            world.membership_cache_slot(cache.slot as usize).generation,
            RETIRED_GENERATION
        );
    }

    #[test]
    fn result_cache_rejects_exact_id_specs() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");
        let result = world
            .build_query_result_cache::<Position>(QuerySpec::new())
            .expect("result");
        let spec =
            QuerySpec::new().exact_ids(vec![entity], crate::query::ExactIdPolicy::SkipUnavailable);
        let plan = world.resolve_query1_plan::<Position>(&spec).expect("plan");
        assert!(matches!(
            world.validate_query_params_caches(&QueryParams::new().result_cache(&result), &plan),
            Err(QueryError::ExactIdOrderConflict)
        ));
    }
}
