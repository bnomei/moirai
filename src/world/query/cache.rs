use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::{QueryCache, QueryError, QueryParams, QuerySpec};
use crate::world::World;

use super::collect::collect_query1_structural_members;
use super::spec::resolve_query1;

const RETIRED_GENERATION: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub(crate) struct MembershipCacheSlot {
    pub generation: u32,
    pub fingerprint: u64,
    pub topology_revision: u64,
    pub members: Vec<EntityId>,
}

impl MembershipCacheSlot {
    fn new(fingerprint: u64, topology_revision: u64, members: Vec<EntityId>) -> Self {
        Self {
            generation: 1,
            fingerprint,
            topology_revision,
            members,
        }
    }

    fn is_live(&self) -> bool {
        self.generation != 0 && self.generation != RETIRED_GENERATION
    }
}

impl World {
    pub fn build_query_cache<T: Clone + 'static>(
        &mut self,
        spec: QuerySpec,
    ) -> Result<QueryCache, QueryError> {
        if spec.exact_ids.is_some() {
            return Err(QueryError::UnsupportedCachePolicy {
                detail: alloc::string::String::from(
                    "membership cache does not support exact-id specs",
                ),
            });
        }
        let plan = resolve_query1::<T>(self, &spec)?;
        let members = collect_query1_structural_members(self, &plan);
        let slot = self.allocate_membership_cache_slot(plan.fingerprint, members)?;
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
        let needs_refresh =
            self.membership_caches[slot].topology_revision != self.query_topology_revision;
        if needs_refresh {
            let members = collect_query1_structural_members(self, plan);
            let revision = self.query_topology_revision;
            let entry = &mut self.membership_caches[slot];
            entry.members = members;
            entry.topology_revision = revision;
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
        members: Vec<EntityId>,
    ) -> Result<usize, QueryError> {
        if let Some(slot) = self
            .membership_caches
            .iter()
            .position(|entry| !entry.is_live())
        {
            self.membership_caches[slot] =
                MembershipCacheSlot::new(fingerprint, self.query_topology_revision, members);
            return Ok(slot);
        }
        let slot = self.membership_caches.len();
        self.membership_caches.push(MembershipCacheSlot::new(
            fingerprint,
            self.query_topology_revision,
            members,
        ));
        Ok(slot)
    }

    #[cfg(any(test, feature = "testkit"))]
    pub fn invalidate_query_cache(&mut self, cache: &QueryCache) {
        self.invalidate_membership_cache_handle(cache);
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
            if plan.added_index.is_some() || plan.changed_index.is_some() {
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

    #[derive(Clone, Copy)]
    struct Position(#[allow(dead_code)] i32);

    #[derive(Clone)]
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
            world.query::<Position>(QuerySpec::new(), params),
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
            .query::<Position>(spec, params)
            .expect("query")
            .count();
        assert_eq!(count, 1);
    }
}
