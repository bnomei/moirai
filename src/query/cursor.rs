use crate::query::QueryError;
use crate::time::ChangeTick;
use crate::world::WorldOwner;

/// Owner- and spec-scoped last-observed tick for added/changed windows.
pub struct QueryCursor {
    owner: WorldOwner,
    fingerprint: u64,
    last_observed: ChangeTick,
}

impl QueryCursor {
    pub fn from_spec_start<T: 'static>(
        world: &mut crate::world::World,
        spec: &crate::query::QuerySpec,
    ) -> Result<Self, QueryError> {
        let fingerprint = world.query_fingerprint::<T>(spec)?;
        Ok(Self::from_start(world, fingerprint))
    }

    pub fn from_spec_now<T: 'static>(
        world: &mut crate::world::World,
        spec: &crate::query::QuerySpec,
    ) -> Result<Self, QueryError> {
        let fingerprint = world.query_fingerprint::<T>(spec)?;
        Self::from_now(world, fingerprint)
    }

    pub fn from_spec2_start<A: 'static, B: 'static>(
        world: &mut crate::world::World,
        spec: &crate::query::QuerySpec,
    ) -> Result<Self, QueryError> {
        let (plan, _, _) = world.resolve_query2_plan::<A, B>(spec)?;
        Ok(Self::from_start(world, plan.fingerprint))
    }

    pub fn from_spec2_now<A: 'static, B: 'static>(
        world: &mut crate::world::World,
        spec: &crate::query::QuerySpec,
    ) -> Result<Self, QueryError> {
        let (plan, _, _) = world.resolve_query2_plan::<A, B>(spec)?;
        Self::from_now(world, plan.fingerprint)
    }

    #[cfg(test)]
    pub(crate) fn for_entities_from_start(
        world: &mut crate::world::World,
        spec: &crate::query::QuerySpec,
    ) -> Result<Self, QueryError> {
        let fingerprint = world.entity_query_fingerprint(spec)?;
        Ok(Self::from_start(world, fingerprint))
    }

    pub fn fork(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            fingerprint: self.fingerprint,
            last_observed: self.last_observed,
        }
    }

    pub(crate) fn from_start(world: &crate::world::World, fingerprint: u64) -> Self {
        Self {
            owner: world.owner_token(),
            fingerprint,
            last_observed: ChangeTick::ZERO,
        }
    }

    pub(crate) fn from_now(
        world: &crate::world::World,
        fingerprint: u64,
    ) -> Result<Self, QueryError> {
        Ok(Self {
            owner: world.owner_token(),
            fingerprint,
            last_observed: world.change_tick(),
        })
    }

    pub(crate) fn validate(
        &self,
        world: &crate::world::World,
        fingerprint: u64,
    ) -> Result<(), QueryError> {
        if !self.owner.same(&world.owner_token()) {
            return Err(QueryError::WrongOwner);
        }
        if self.fingerprint != fingerprint {
            return Err(QueryError::WrongQuery {
                detail: alloc::string::String::from("cursor fingerprint does not match query spec"),
            });
        }
        Ok(())
    }

    pub fn since(&self) -> ChangeTick {
        self.last_observed
    }

    pub(crate) fn commit(&mut self, captured_now: ChangeTick) {
        self.last_observed = captured_now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{QueryParams, QuerySpec};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Position(#[allow(dead_code)] i32);

    #[test]
    fn query_cursor_commits_on_exhaustion() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");

        let spec = QuerySpec::new().added::<Position>();
        let mut cursor =
            QueryCursor::from_spec_start::<Position>(&mut world, &spec).expect("cursor");
        let params = QueryParams::new().cursor(&mut cursor);
        let mut query = world.query::<Position>(&spec, params).expect("query");
        assert!(query.next().is_some());
        assert!(query.next().is_none());
        drop(query);
        assert!(cursor.since().raw() > 0);
    }

    #[test]
    fn query_cursor_skips_commit_on_partial_iteration() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let a = world.spawn().expect("spawn");
        let b = world.spawn().expect("spawn");
        world.insert(a, Position(1)).expect("insert");
        world.insert(b, Position(2)).expect("insert");

        let spec = QuerySpec::new().added::<Position>();
        let mut cursor =
            QueryCursor::from_spec_start::<Position>(&mut world, &spec).expect("cursor");
        let before = cursor.since();
        let params = QueryParams::new().cursor(&mut cursor);
        let mut query = world.query::<Position>(&spec, params).expect("query");
        let _ = query.next();
        drop(query);
        assert_eq!(cursor.since(), before);
    }

    #[test]
    fn from_spec_now_and_from_now_capture_change_tick() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");
        let spec = QuerySpec::new();
        let fingerprint = world.query_fingerprint::<Position>(&spec).expect("fp");
        let cursor = QueryCursor::from_spec_now::<Position>(&mut world, &spec).expect("now");
        assert_eq!(cursor.since(), world.change_tick());
        let direct = QueryCursor::from_now(&world, fingerprint).expect("direct");
        assert_eq!(direct.since(), world.change_tick());
    }

    #[test]
    fn validate_rejects_foreign_owner() {
        let mut builder_a = WorldBuilder::new();
        builder_a
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world_a = builder_a.build().expect("a");
        let spec = QuerySpec::new();
        let mut cursor =
            QueryCursor::from_spec_start::<Position>(&mut world_a, &spec).expect("cursor");
        let mut builder_b = WorldBuilder::new();
        builder_b
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("register");
        let mut world_b = builder_b.build().expect("b");
        let fingerprint = world_b.query_fingerprint::<Position>(&spec).expect("fp");
        assert!(matches!(
            cursor.validate(&world_b, fingerprint),
            Err(QueryError::WrongOwner)
        ));
        let _ = &mut cursor;
    }
}
