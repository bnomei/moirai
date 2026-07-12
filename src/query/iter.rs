use alloc::vec::Vec;

use crate::entity::EntityId;

/// Immutable single-component query iterator.
pub struct Query1<'w, 'c, T: Clone + 'static> {
    pub(crate) world: &'w crate::world::World,
    pub(crate) plan: crate::world::query::plan::ResolvedPlan,
    pub(crate) params_fingerprint: u64,
    pub(crate) captured_now: crate::time::ChangeTick,
    pub(crate) since: crate::time::ChangeTick,
    pub(crate) cursor_committed: bool,
    pub(crate) cursor: Option<&'c mut crate::query::QueryCursor>,
    pub(crate) state: Query1State<'w, T>,
}

pub(crate) enum Query1State<'w, T: Clone + 'static> {
    Sparse {
        store: &'w crate::storage::TypedSparseStorage<T>,
        index: usize,
    },
    Table {
        archetypes: Vec<usize>,
        archetype_index: usize,
        row: usize,
    },
    Exact {
        ids: Vec<EntityId>,
        index: usize,
    },
    Cached {
        ids: Vec<EntityId>,
        index: usize,
    },
    Done,
}

/// Immutable two-component query iterator.
pub struct Query2<'w, 'c, A: Clone + 'static, B: Clone + 'static> {
    pub(crate) inner: Query1<'w, 'c, A>,
    pub(crate) second_index: usize,
    pub(crate) second_is_table: bool,
    pub(crate) _marker: core::marker::PhantomData<fn() -> B>,
}

impl<'w, 'c, T: Clone + 'static> Query1<'w, 'c, T> {
    pub(crate) fn new(
        world: &'w crate::world::World,
        plan: crate::world::query::plan::ResolvedPlan,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        cached_ids: Option<Vec<EntityId>>,
    ) -> Result<Self, crate::query::QueryError> {
        let state = world.query1_state::<T>(&plan, cached_ids)?;
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

impl<'w, 'c, T: Clone + 'static> Iterator for Query1<'w, 'c, T> {
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
                Query1State::Cached { ids, index } => {
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
            }
        }
    }
}

impl<'w, 'c, T: Clone + 'static> Drop for Query1<'w, 'c, T> {
    fn drop(&mut self) {
        if matches!(self.state, Query1State::Done) {
            self.commit_cursor_if_needed();
        }
    }
}

impl<'w, 'c, A: Clone + 'static, B: Clone + 'static> Query2<'w, 'c, A, B> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        world: &'w crate::world::World,
        plan: crate::world::query::plan::ResolvedPlan,
        since: crate::time::ChangeTick,
        captured_now: crate::time::ChangeTick,
        cursor: Option<&'c mut crate::query::QueryCursor>,
        cached_ids: Option<Vec<EntityId>>,
        second_index: usize,
        second_is_table: bool,
    ) -> Result<Self, crate::query::QueryError> {
        Ok(Self {
            inner: Query1::new(world, plan, since, captured_now, cursor, cached_ids)?,
            second_index,
            second_is_table,
            _marker: core::marker::PhantomData,
        })
    }
}

impl<'w, 'c, A: Clone + 'static, B: Clone + 'static> Iterator for Query2<'w, 'c, A, B> {
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
