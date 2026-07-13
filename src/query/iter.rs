use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::world::query::cached_source::QueryCachedSource;

/// Immutable single-component query iterator.
pub struct Query1<'w, 'c, T: 'static> {
    pub(crate) world: &'w crate::world::World,
    pub(crate) plan: Rc<crate::world::query::plan::ResolvedPlan>,
    pub(crate) params_fingerprint: u64,
    pub(crate) captured_now: crate::time::ChangeTick,
    pub(crate) since: crate::time::ChangeTick,
    pub(crate) cursor_committed: bool,
    pub(crate) cursor: Option<&'c mut crate::query::QueryCursor>,
    pub(crate) state: Query1State<'w, T>,
}

pub(crate) enum Query1State<'w, T: 'static> {
    Sparse {
        store: &'w crate::storage::TypedSparseStorage<T>,
        index: usize,
    },
    Table {
        archetypes: &'w [usize],
        archetype_index: usize,
        row: usize,
    },
    Exact {
        ids: Vec<EntityId>,
        index: usize,
    },
    Cached {
        source: QueryCachedSource,
        index: usize,
    },
    Done,
}

/// Immutable two-component query iterator.
pub struct Query2<'w, 'c, A: 'static, B: 'static> {
    pub(crate) inner: Query1<'w, 'c, A>,
    pub(crate) second_index: usize,
    pub(crate) second_is_table: bool,
    pub(crate) _marker: core::marker::PhantomData<fn() -> B>,
}

impl<'w, 'c, T: 'static> Query1<'w, 'c, T> {
    pub(crate) fn new(
        world: &'w crate::world::World,
        plan: Rc<crate::world::query::plan::ResolvedPlan>,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        cached: Option<QueryCachedSource>,
        table_archetypes: Option<&'w [usize]>,
    ) -> Result<Self, crate::query::QueryError> {
        let state = world.query1_state::<T>(&plan, cached, table_archetypes)?;
        Ok(Self {
            world,
            params_fingerprint: plan.fingerprint,
            plan,
            captured_now,
            since,
            cursor_committed: false,
            cursor,
            state,
        })
    }

    fn commit_cursor_if_needed(&mut self) {
        if self.cursor_committed {
            return;
        }
        if let Some(cursor) = self.cursor.as_mut() {
            if cursor.validate(self.world, self.params_fingerprint).is_ok() {
                cursor.commit(self.captured_now);
            }
        }
        self.cursor_committed = true;
    }
}

impl<'w, 'c, T: 'static> Iterator for Query1<'w, 'c, T> {
    type Item = (EntityId, &'w T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match &mut self.state {
                Query1State::Done => {
                    self.commit_cursor_if_needed();
                    return None;
                }
                Query1State::Sparse { store, index } => {
                    let slots = store.dense_slots();
                    while *index < slots.len() {
                        let slot = slots[*index];
                        *index += 1;
                        let entity = self.world.entity_from_slot(slot);
                        if let Some(value) = self.world.query1_match_sparse::<T>(
                            entity,
                            &self.plan,
                            self.since,
                            self.captured_now,
                            store,
                        ) {
                            return Some((entity, value));
                        }
                    }
                    self.state = Query1State::Done;
                }
                Query1State::Table {
                    archetypes,
                    archetype_index,
                    row,
                } => {
                    while *archetype_index < archetypes.len() {
                        let archetype = archetypes[*archetype_index];
                        let slots = self.world.archetype_entity_slots(archetype);
                        while *row < slots.len() {
                            let slot = slots[*row];
                            *row += 1;
                            let entity = self.world.entity_from_slot(slot);
                            if let Some(value) = self.world.query1_match_table::<T>(
                                entity,
                                &self.plan,
                                self.since,
                                self.captured_now,
                            ) {
                                return Some((entity, value));
                            }
                        }
                        *archetype_index += 1;
                        *row = 0;
                    }
                    self.state = Query1State::Done;
                }
                Query1State::Exact { ids, index } => {
                    while *index < ids.len() {
                        let entity = ids[*index];
                        *index += 1;
                        if let Some(value) = self.world.query1_match_any_storage::<T>(
                            entity,
                            &self.plan,
                            self.since,
                            self.captured_now,
                        ) {
                            return Some((entity, value));
                        }
                    }
                    self.state = Query1State::Done;
                }
                Query1State::Cached { source, index } => {
                    let ids = match self
                        .world
                        .cached_query_entities(source, self.params_fingerprint)
                    {
                        Ok(ids) => ids,
                        Err(_) => {
                            self.state = Query1State::Done;
                            continue;
                        }
                    };
                    while *index < ids.len() {
                        let entity = ids[*index];
                        *index += 1;
                        let value = if !self.plan.added_indices.is_empty()
                            || !self.plan.changed_indices.is_empty()
                        {
                            self.world.query1_match_any_storage::<T>(
                                entity,
                                &self.plan,
                                self.since,
                                self.captured_now,
                            )
                        } else {
                            self.world.query1_match_cached::<T>(entity, &self.plan)
                        };
                        if let Some(value) = value {
                            return Some((entity, value));
                        }
                    }
                    self.state = Query1State::Done;
                }
            }
        }
    }
}

impl<'w, 'c, T: 'static> Drop for Query1<'w, 'c, T> {
    fn drop(&mut self) {
        if matches!(self.state, Query1State::Done) {
            self.commit_cursor_if_needed();
        }
    }
}

impl<'w, 'c, A: 'static, B: 'static> Query2<'w, 'c, A, B> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        world: &'w crate::world::World,
        plan: Rc<crate::world::query::plan::ResolvedPlan>,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        cached: Option<QueryCachedSource>,
        table_archetypes: Option<&'w [usize]>,
        second_index: usize,
        second_is_table: bool,
    ) -> Result<Self, crate::query::QueryError> {
        Ok(Self {
            inner: Query1::new(
                world,
                plan,
                since,
                captured_now,
                cursor,
                cached,
                table_archetypes,
            )?,
            second_index,
            second_is_table,
            _marker: core::marker::PhantomData,
        })
    }
}

impl<'w, 'c, A: 'static, B: 'static> Iterator for Query2<'w, 'c, A, B> {
    type Item = (EntityId, &'w A, &'w B);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (entity, first) = self.inner.next()?;
            if let Some(second) =
                self.inner
                    .world
                    .query2_second::<B>(entity, self.second_index, self.second_is_table)
            {
                return Some((entity, first, second));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{QueryError, QuerySpec};
    use crate::world::query::plan::{ResolvedPlan, TraversalSource};
    use crate::world::WorldBuilder;
    use alloc::rc::Rc;

    #[derive(Clone, Copy)]
    struct Pos(i32);

    #[derive(Clone, Copy)]
    struct Vel(#[allow(dead_code)] i32);

    fn sparse_world() -> crate::world::World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::sparse())
            .expect("vel");
        builder.build().expect("build")
    }

    #[test]
    fn cached_iterator_stops_when_cache_lookup_fails_mid_iteration() {
        use crate::world::query::cached_source::QueryCachedSource;

        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        let plan = world
            .resolve_query1_plan::<Pos>(&QuerySpec::new())
            .expect("plan");
        let cache = world
            .build_query_cache::<Pos>(QuerySpec::new())
            .expect("cache");
        let stale = cache.clone();
        world.invalidate_query_cache(&cache);
        let mut iter = Query1::<Pos> {
            world: &world,
            plan: plan.clone(),
            params_fingerprint: plan.fingerprint,
            captured_now: world.change_tick(),
            since: crate::time::ChangeTick::ZERO,
            cursor_committed: false,
            cursor: None,
            state: Query1State::Cached {
                source: QueryCachedSource::Membership(stale),
                index: 0,
            },
        };
        assert!(iter.next().is_none());
        assert!(matches!(iter.state, Query1State::Done));
    }

    #[test]
    fn query2_new_propagates_query1_resolution_errors() {
        let world = sparse_world();
        let plan = Rc::new(crate::world::query::plan::ResolvedPlan {
            fingerprint: 1,
            primary_index: 0,
            primary_is_table: false,
            traversal: crate::world::query::plan::TraversalSource::Sparse {
                component_index: 99,
            },
            required_indices: alloc::vec![99],
            without_indices: alloc::vec![],
            with_tag_indices: alloc::vec![],
            without_tag_indices: alloc::vec![],
            added_indices: alloc::vec![],
            changed_indices: alloc::vec![],
            exact_id_policy: None,
        });
        assert!(matches!(
            Query2::<Pos, Vel>::new(
                &world,
                plan,
                crate::time::ChangeTick::ZERO,
                crate::time::ChangeTick::ZERO,
                None,
                None,
                None,
                1,
                false,
            ),
            Err(QueryError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn query2_iterator_skips_entities_missing_second_component() {
        let mut world = sparse_world();
        let partial = world.spawn().expect("partial");
        let matched = world.spawn().expect("matched");
        world.insert(partial, Pos(1)).expect("partial");
        world.insert(matched, Pos(2)).expect("matched pos");
        world.insert(matched, Vel(9)).expect("matched vel");
        let plan = Rc::new(ResolvedPlan {
            fingerprint: 1,
            primary_index: 0,
            primary_is_table: false,
            traversal: TraversalSource::Sparse { component_index: 0 },
            required_indices: alloc::vec![0],
            without_indices: alloc::vec![],
            with_tag_indices: alloc::vec![],
            without_tag_indices: alloc::vec![],
            added_indices: alloc::vec![],
            changed_indices: alloc::vec![],
            exact_id_policy: None,
        });
        let mut iter = Query2::<Pos, Vel>::new(
            &world,
            plan,
            crate::time::ChangeTick::ZERO,
            world.change_tick(),
            None,
            None,
            None,
            1,
            false,
        )
        .expect("query2");
        assert_eq!(iter.next().map(|(_, pos, _)| pos.0), Some(2));
        assert!(iter.next().is_none());
    }
}
