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
    pub(crate) additional_covered_required: Option<usize>,
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
    Borrowed {
        ids: &'w [EntityId],
        index: usize,
        apply_temporal: bool,
    },
    Done,
}

/// Immutable two-component query iterator.
pub struct Query2<'w, 'c, A: 'static, B: 'static> {
    pub(crate) world: &'w crate::world::World,
    pub(crate) plan: Rc<crate::world::query::plan::ResolvedPlan>,
    pub(crate) params_fingerprint: u64,
    pub(crate) captured_now: crate::time::ChangeTick,
    pub(crate) since: crate::time::ChangeTick,
    pub(crate) cursor_committed: bool,
    pub(crate) cursor: Option<&'c mut crate::query::QueryCursor>,
    pub(crate) state: Query2State<'w>,
    pub(crate) second_index: usize,
    pub(crate) second_is_table: bool,
    pub(crate) marker: core::marker::PhantomData<fn() -> (A, B)>,
}

pub(crate) enum Query2State<'w> {
    Sparse {
        slots: &'w [u32],
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
    Borrowed {
        ids: &'w [EntityId],
        index: usize,
        apply_temporal: bool,
    },
    Done,
}

impl<'w, 'c, T: 'static> Query1<'w, 'c, T> {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn new(
        world: &'w crate::world::World,
        plan: Rc<crate::world::query::plan::ResolvedPlan>,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        cached: Option<QueryCachedSource>,
        table_archetypes: Option<&'w [usize]>,
        additional_covered_required: Option<usize>,
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
            additional_covered_required,
            state,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_prepared(
        world: &'w crate::world::World,
        plan: Rc<crate::world::query::plan::ResolvedPlan>,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        materialized: Option<(&'w [EntityId], bool)>,
        table_archetypes: Option<&'w [usize]>,
    ) -> Result<Self, crate::query::QueryError> {
        let state = if let Some((ids, apply_temporal)) = materialized {
            Query1State::Borrowed {
                ids,
                index: 0,
                apply_temporal,
            }
        } else {
            world.query1_state::<T>(&plan, None, table_archetypes)?
        };
        Ok(Self {
            world,
            params_fingerprint: plan.fingerprint,
            plan,
            captured_now,
            since,
            cursor_committed: false,
            cursor,
            additional_covered_required: None,
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
                        let dense_index = *index;
                        let slot = slots[dense_index];
                        *index += 1;
                        let entity = self.world.entity_from_slot(slot);
                        if let Some(additional) = self.additional_covered_required {
                            if !self.world.query1_accept_source_covered(
                                entity,
                                &self.plan,
                                self.since,
                                self.captured_now,
                                additional,
                            ) {
                                continue;
                            }
                            let value = store
                                .dense_value(dense_index)
                                .expect("sparse dense slot and value vectors stay aligned");
                            return Some((entity, value));
                        }
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
                                self.additional_covered_required,
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
                Query1State::Borrowed {
                    ids,
                    index,
                    apply_temporal,
                } => {
                    while *index < ids.len() {
                        let entity = ids[*index];
                        *index += 1;
                        if *apply_temporal
                            && !crate::world::query::filter::entity_matches_temporal(
                                self.world,
                                entity,
                                &self.plan,
                                self.since,
                                self.captured_now,
                            )
                        {
                            continue;
                        }
                        let value = self.world.query1_match_cached::<T>(entity, &self.plan);
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
    #[allow(clippy::too_many_arguments, dead_code)]
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
        let state = Self::state(world, &plan, cached, None, table_archetypes)?;
        Ok(Self {
            world,
            params_fingerprint: plan.fingerprint,
            plan,
            captured_now,
            since,
            cursor_committed: false,
            cursor,
            state,
            second_index,
            second_is_table,
            marker: core::marker::PhantomData,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_prepared(
        world: &'w crate::world::World,
        plan: Rc<crate::world::query::plan::ResolvedPlan>,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        materialized: Option<(&'w [EntityId], bool)>,
        table_archetypes: Option<&'w [usize]>,
        second_index: usize,
        second_is_table: bool,
    ) -> Result<Self, crate::query::QueryError> {
        let state = Self::state(world, &plan, None, materialized, table_archetypes)?;
        Ok(Self {
            world,
            params_fingerprint: plan.fingerprint,
            plan,
            captured_now,
            since,
            cursor_committed: false,
            cursor,
            state,
            second_index,
            second_is_table,
            marker: core::marker::PhantomData,
        })
    }

    fn state(
        world: &'w crate::world::World,
        plan: &crate::world::query::plan::ResolvedPlan,
        cached: Option<QueryCachedSource>,
        materialized: Option<(&'w [EntityId], bool)>,
        table_archetypes: Option<&'w [usize]>,
    ) -> Result<Query2State<'w>, crate::query::QueryError> {
        if let Some((ids, apply_temporal)) = materialized {
            return Ok(Query2State::Borrowed {
                ids,
                index: 0,
                apply_temporal,
            });
        }
        if let Some(source) = cached {
            return Ok(Query2State::Cached { source, index: 0 });
        }
        match &plan.traversal {
            crate::world::query::plan::TraversalSource::All => {
                Err(crate::query::QueryError::WrongQuery {
                    detail: alloc::string::String::from(
                        "entity-only plan cannot back a typed query",
                    ),
                })
            }
            crate::world::query::plan::TraversalSource::Sparse { component_index } => {
                let slots = world.sparse_dense_slots(*component_index).ok_or_else(|| {
                    crate::query::QueryError::WrongStorageKind {
                        name: alloc::format!("component {component_index}"),
                    }
                })?;
                Ok(Query2State::Sparse { slots, index: 0 })
            }
            crate::world::query::plan::TraversalSource::Table { .. } => Ok(Query2State::Table {
                archetypes: table_archetypes.expect("table archetypes prepared"),
                archetype_index: 0,
                row: 0,
            }),
            crate::world::query::plan::TraversalSource::Exact { ids } => Ok(Query2State::Exact {
                ids: ids.clone(),
                index: 0,
            }),
        }
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

    fn match_entity(&self, entity: EntityId, filter: CandidateFilter) -> Option<(&'w A, &'w B)> {
        let matches = match filter {
            CandidateFilter::Full => crate::world::query::filter::entity_matches(
                self.world,
                entity,
                &self.plan,
                self.since,
                self.captured_now,
            ),
            CandidateFilter::Temporal => crate::world::query::filter::entity_matches_temporal(
                self.world,
                entity,
                &self.plan,
                self.since,
                self.captured_now,
            ),
            CandidateFilter::Trusted => true,
        };
        if !matches {
            return None;
        }
        let first = self.world.query_component::<A>(
            entity,
            self.plan.primary_index,
            self.plan.primary_is_table,
        )?;
        let second =
            self.world
                .query_component::<B>(entity, self.second_index, self.second_is_table)?;
        Some((first, second))
    }
}

impl<'w, 'c, A: 'static, B: 'static> Iterator for Query2<'w, 'c, A, B> {
    type Item = (EntityId, &'w A, &'w B);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let candidate = match &mut self.state {
                Query2State::Done => {
                    self.commit_cursor_if_needed();
                    return None;
                }
                Query2State::Sparse { slots, index } => {
                    let entity = slots
                        .get(*index)
                        .copied()
                        .map(|slot| self.world.entity_from_slot(slot));
                    *index += usize::from(entity.is_some());
                    entity.map(|entity| (entity, CandidateFilter::Full))
                }
                Query2State::Table {
                    archetypes,
                    archetype_index,
                    row,
                } => {
                    let mut entity = None;
                    while *archetype_index < archetypes.len() && entity.is_none() {
                        let slots = self
                            .world
                            .archetype_entity_slots(archetypes[*archetype_index]);
                        if let Some(slot) = slots.get(*row).copied() {
                            *row += 1;
                            entity = Some(self.world.entity_from_slot(slot));
                        } else {
                            *archetype_index += 1;
                            *row = 0;
                        }
                    }
                    entity.map(|entity| (entity, CandidateFilter::Full))
                }
                Query2State::Exact { ids, index } => {
                    let entity = ids.get(*index).copied();
                    *index += usize::from(entity.is_some());
                    entity.map(|entity| (entity, CandidateFilter::Full))
                }
                Query2State::Cached { source, index } => {
                    let ids = match self
                        .world
                        .cached_query_entities(source, self.params_fingerprint)
                    {
                        Ok(ids) => ids,
                        Err(_) => {
                            self.state = Query2State::Done;
                            continue;
                        }
                    };
                    let entity = ids.get(*index).copied();
                    *index += usize::from(entity.is_some());
                    entity.map(|entity| (entity, CandidateFilter::Full))
                }
                Query2State::Borrowed {
                    ids,
                    index,
                    apply_temporal,
                } => {
                    let entity = ids.get(*index).copied();
                    *index += usize::from(entity.is_some());
                    let filter = if *apply_temporal {
                        CandidateFilter::Temporal
                    } else {
                        CandidateFilter::Trusted
                    };
                    entity.map(|entity| (entity, filter))
                }
            };
            let Some((entity, filter)) = candidate else {
                self.state = Query2State::Done;
                continue;
            };
            if let Some((first, second)) = self.match_entity(entity, filter) {
                return Some((entity, first, second));
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CandidateFilter {
    Full,
    Temporal,
    Trusted,
}

impl<'w, 'c, A: 'static, B: 'static> Drop for Query2<'w, 'c, A, B> {
    fn drop(&mut self) {
        if matches!(self.state, Query2State::Done) {
            self.commit_cursor_if_needed();
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
            additional_covered_required: None,
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
