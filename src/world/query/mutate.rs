//! Mutable query traversal across sparse and archetype component storage.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;

use crate::entity::EntityId;
use crate::query::{QueryEffects, QueryError, QueryParams, QuerySpec};
use crate::storage::{SparseStore, TypedSparseStorage};
use crate::time::ChangeTick;
use crate::world::World;

use super::collect::{
    collect_query1_entities, collect_query1_entities_into, collect_query2_entities,
};
use super::filter::{entity_matches, entity_matches_temporal, validate_exact_ids};

enum PreparedCandidates<'a> {
    Borrowed(&'a [EntityId]),
    Scratch(&'a [EntityId]),
}

impl PreparedCandidates<'_> {
    fn as_slice(&self) -> &[EntityId] {
        match self {
            Self::Borrowed(ids) | Self::Scratch(ids) => ids,
        }
    }
}

impl World {
    pub(crate) fn for_each_mut<T>(
        &mut self,
        spec: &QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut T) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: 'static,
    {
        self.for_each_mut_inner(spec, params, |entity, value, _| f(entity, value))
    }

    pub(crate) fn for_each_mut_with_effects<T>(
        &mut self,
        spec: &QuerySpec,
        params: QueryParams<'_>,
        f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: 'static,
    {
        self.for_each_mut_inner(spec, params, f)
    }

    pub(crate) fn for_each2_mut<A, B>(
        &mut self,
        spec: &QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut A, &mut B) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: 'static,
        B: 'static,
    {
        self.for_each2_mut_inner(spec, params, |entity, a, b, _| f(entity, a, b))
    }

    pub(crate) fn for_each2_mut_with_effects<A, B>(
        &mut self,
        spec: &QuerySpec,
        params: QueryParams<'_>,
        f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: 'static,
        B: 'static,
    {
        self.for_each2_mut_inner(spec, params, f)
    }

    fn for_each_mut_inner<T>(
        &mut self,
        spec: &QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: 'static,
    {
        let mut params = params;
        let plan = self.resolve_query1_plan::<T>(spec)?;
        validate_exact_ids(self, &plan)?;
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;
        let entities = self.resolve_cached_entities(&params, &plan, since, captured_now)?;

        self.preflight_change_ticks(entities.len())?;

        let primary_index = plan.primary_index;
        let primary_is_table = plan.primary_is_table;

        for entity in entities {
            let tick = self.issue_change_tick_query()?;
            let visit = if primary_is_table {
                self.visit_table_mut(primary_index as u32, entity, tick, |value, effects| {
                    f(entity, value, effects)
                })
            } else {
                self.visit_sparse_mut::<T>(primary_index, entity, tick, |value, effects| {
                    f(entity, value, effects)
                })
            };
            visit?;
        }

        params.commit_cursor(plan.fingerprint, self, captured_now)?;
        Ok(())
    }

    fn for_each2_mut_inner<A, B>(
        &mut self,
        spec: &QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: 'static,
        B: 'static,
    {
        let mut params = params;
        let (plan, second_index, second_is_table) = self.resolve_query2_plan::<A, B>(spec)?;
        validate_exact_ids(self, &plan)?;
        if plan.primary_index == second_index {
            return Err(QueryError::DuplicateMutableComponent {
                name: String::from(type_name::<A>()),
            });
        }
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;
        let entities = if params.result_cache.is_some() {
            self.resolve_cached_entities(&params, &plan, since, captured_now)?
        } else if let Some(cache) = params.membership_cache {
            let members: Vec<EntityId> = self.refresh_membership_cache(cache, &plan)?.to_vec();
            members
                .into_iter()
                .filter(|&entity| {
                    entity_matches(self, entity, &plan, since, captured_now)
                        && self.entity_has_query2_second(entity, second_index, second_is_table)
                })
                .collect()
        } else {
            collect_query2_entities(
                self,
                &plan,
                since,
                captured_now,
                second_index,
                second_is_table,
            )
        };

        self.preflight_change_ticks(entities.len())?;

        for entity in entities {
            let tick = self.issue_change_tick_query()?;
            let visit = self.visit_two_mut::<A, B>(
                plan.primary_index,
                plan.primary_is_table,
                second_index,
                second_is_table,
                entity,
                tick,
                |a, b, effects| f(entity, a, b, effects),
            );
            visit?;
        }

        params.commit_cursor(plan.fingerprint, self, captured_now)?;
        Ok(())
    }

    pub(crate) fn for_each_mut_resolved<T: 'static>(
        &mut self,
        plan: &super::plan::ResolvedPlan,
        materialized: Option<(&[EntityId], bool)>,
        scratch: &mut Vec<EntityId>,
        since: ChangeTick,
        captured_now: ChangeTick,
        mut f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let candidates = self.prepared_candidates(plan, materialized, since, captured_now, scratch);
        self.preflight_change_ticks(candidates.as_slice().len())?;
        for &entity in candidates.as_slice() {
            let tick = self.issue_change_tick_query()?;
            if plan.primary_is_table {
                self.visit_table_mut(plan.primary_index as u32, entity, tick, |value, effects| {
                    f(entity, value, effects)
                })?;
            } else {
                self.visit_sparse_mut::<T>(plan.primary_index, entity, tick, |value, effects| {
                    f(entity, value, effects)
                })?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_each2_mut_resolved<A: 'static, B: 'static>(
        &mut self,
        plan: &super::plan::ResolvedPlan,
        second_index: usize,
        second_is_table: bool,
        materialized: Option<(&[EntityId], bool)>,
        scratch: &mut Vec<EntityId>,
        since: ChangeTick,
        captured_now: ChangeTick,
        mut f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if plan.primary_index == second_index {
            return Err(QueryError::DuplicateMutableComponent {
                name: String::from(type_name::<A>()),
            });
        }
        let candidates = self.prepared_candidates(plan, materialized, since, captured_now, scratch);
        self.preflight_change_ticks(candidates.as_slice().len())?;
        for &entity in candidates.as_slice() {
            let tick = self.issue_change_tick_query()?;
            self.visit_two_mut::<A, B>(
                plan.primary_index,
                plan.primary_is_table,
                second_index,
                second_is_table,
                entity,
                tick,
                |a, b, effects| f(entity, a, b, effects),
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_each2_mut_read_resolved<A: 'static, B: 'static>(
        &mut self,
        plan: &super::plan::ResolvedPlan,
        second_index: usize,
        second_is_table: bool,
        materialized: Option<(&[EntityId], bool)>,
        scratch: &mut Vec<EntityId>,
        since: ChangeTick,
        captured_now: ChangeTick,
        mut f: impl FnMut(EntityId, &mut A, &B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if plan.primary_index == second_index {
            return Err(QueryError::DuplicateMutableComponent {
                name: String::from(type_name::<A>()),
            });
        }
        let candidates = self.prepared_candidates(plan, materialized, since, captured_now, scratch);
        self.preflight_change_ticks(candidates.as_slice().len())?;
        for &entity in candidates.as_slice() {
            let tick = self.issue_change_tick_query()?;
            self.visit_mut_read::<A, B>(
                plan.primary_index,
                plan.primary_is_table,
                second_index,
                second_is_table,
                entity,
                tick,
                |a, b, effects| f(entity, a, b, effects),
            )?;
        }
        Ok(())
    }

    fn prepared_candidates<'a>(
        &self,
        plan: &super::plan::ResolvedPlan,
        materialized: Option<(&'a [EntityId], bool)>,
        since: ChangeTick,
        captured_now: ChangeTick,
        scratch: &'a mut Vec<EntityId>,
    ) -> PreparedCandidates<'a> {
        let Some((ids, apply_temporal)) = materialized else {
            collect_query1_entities_into(self, plan, since, captured_now, scratch);
            return PreparedCandidates::Scratch(scratch);
        };
        if !apply_temporal {
            return PreparedCandidates::Borrowed(ids);
        }
        scratch.clear();
        scratch.extend(
            ids.iter()
                .copied()
                .filter(|&entity| entity_matches_temporal(self, entity, plan, since, captured_now)),
        );
        PreparedCandidates::Scratch(scratch)
    }

    pub(crate) fn resolve_cached_entities(
        &mut self,
        params: &QueryParams<'_>,
        plan: &super::plan::ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
    ) -> Result<Vec<EntityId>, QueryError> {
        if let Some(cache) = params.membership_cache {
            let members: Vec<EntityId> = self.refresh_membership_cache(cache, plan)?.to_vec();
            return Ok(members
                .into_iter()
                .filter(|&entity| entity_matches(self, entity, plan, since, captured_now))
                .collect());
        }
        if let Some(cache) = params.result_cache {
            return Ok(self
                .refresh_result_cache(cache, plan, since, captured_now)?
                .to_vec());
        }
        Ok(collect_query1_entities(self, plan, since, captured_now))
    }

    pub(crate) fn entity_has_query2_second(
        &self,
        entity: EntityId,
        second_index: usize,
        second_is_table: bool,
    ) -> bool {
        if second_is_table {
            self.archetype_has_component(entity, second_index as u32)
        } else {
            self.sparse_stores
                .get(second_index)
                .map(|store| store.contains_entity(entity))
                .unwrap_or(false)
        }
    }

    pub(crate) fn sparse_dense_slots(&self, index: usize) -> Option<&[u32]> {
        self.sparse_stores
            .get(index)
            .map(|store| store.dense_slots())
    }

    pub(crate) fn preflight_change_ticks(&self, count: usize) -> Result<(), QueryError> {
        if self.mutation_poisoned {
            return Err(QueryError::BorrowConflict {
                detail: String::from("world mutation is poisoned"),
            });
        }
        if !self.change_tick.can_advance_n(count) {
            return Err(QueryError::BorrowConflict {
                detail: String::from("insufficient change ticks for query mutation"),
            });
        }
        Ok(())
    }

    pub(crate) fn issue_change_tick_query(&mut self) -> Result<ChangeTick, QueryError> {
        self.issue_change_tick()
            .map_err(|_| QueryError::BorrowConflict {
                detail: String::from("change tick exhausted during query mutation"),
            })
    }

    fn visit_sparse_mut<T>(
        &mut self,
        index: usize,
        entity: EntityId,
        tick: ChangeTick,
        mut f: impl FnMut(&mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: 'static,
    {
        let run_guard = self.run_guard_state();
        let owner = self.owner_token();
        let command_queue = &mut self.command_queue;
        let allocator = &mut self.allocator;
        let events = &mut self.events;
        let store = self
            .sparse_stores
            .get_mut(index)
            .and_then(|store| store.typed_mut::<T>())
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index}"),
            })?;
        let mut effects =
            QueryEffects::from_parts(command_queue, allocator, events, run_guard, owner);
        let value =
            store
                .get_mut_with_tick(entity, tick)
                .ok_or_else(|| QueryError::TraversalAborted {
                    entity,
                    detail: alloc::format!("entity missing {}", type_name::<T>()),
                })?;
        f(value, &mut effects)
    }

    fn visit_table_mut<T>(
        &mut self,
        index: u32,
        entity: EntityId,
        tick: ChangeTick,
        mut f: impl FnMut(&mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: 'static,
    {
        let run_guard = self.run_guard_state();
        let owner = self.owner_token();
        let command_queue = &mut self.command_queue;
        let allocator = &mut self.allocator;
        let events = &mut self.events;
        let value = self
            .archetypes
            .get_table_mut(entity, index, tick)
            .ok_or_else(|| QueryError::TraversalAborted {
                entity,
                detail: alloc::format!("entity missing {}", type_name::<T>()),
            })?;
        let mut effects =
            QueryEffects::from_parts(command_queue, allocator, events, run_guard, owner);
        f(value, &mut effects)
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_two_mut<A, B>(
        &mut self,
        primary_index: usize,
        primary_is_table: bool,
        second_index: usize,
        second_is_table: bool,
        entity: EntityId,
        tick: ChangeTick,
        mut f: impl FnMut(&mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: 'static,
        B: 'static,
    {
        let run_guard = self.run_guard_state();
        let owner = self.owner_token();
        let command_queue = &mut self.command_queue;
        let allocator = &mut self.allocator;
        let events = &mut self.events;
        let mut effects =
            QueryEffects::from_parts(command_queue, allocator, events, run_guard, owner);

        match (primary_is_table, second_is_table) {
            (false, false) => {
                let (store_a, store_b) = split_sparse_stores_mut::<A, B>(
                    &mut self.sparse_stores,
                    primary_index,
                    second_index,
                )?;
                let a = store_a.get_mut_with_tick(entity, tick).ok_or_else(|| {
                    QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<A>()),
                    }
                })?;
                let b = store_b.get_mut_with_tick(entity, tick).ok_or_else(|| {
                    QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<B>()),
                    }
                })?;
                f(a, b, &mut effects)
            }
            (true, true) => {
                let (a, b) = self
                    .archetypes
                    .get_two_table_mut::<A, B>(
                        entity,
                        primary_index as u32,
                        second_index as u32,
                        tick,
                    )
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: String::from("entity missing query2 table components"),
                    })?;
                f(a, b, &mut effects)
            }
            (false, true) => {
                let store = self
                    .sparse_stores
                    .get_mut(primary_index)
                    .and_then(|store| store.typed_mut::<A>())
                    .ok_or_else(|| QueryError::WrongStorageKind {
                        name: alloc::format!("component {primary_index}"),
                    })?;
                let archetypes = &mut self.archetypes;
                let a = store.get_mut_with_tick(entity, tick).ok_or_else(|| {
                    QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<A>()),
                    }
                })?;
                let b = archetypes
                    .get_table_mut::<B>(entity, second_index as u32, tick)
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<B>()),
                    })?;
                f(a, b, &mut effects)
            }
            (true, false) => {
                let store = self
                    .sparse_stores
                    .get_mut(second_index)
                    .and_then(|store| store.typed_mut::<B>())
                    .ok_or_else(|| QueryError::WrongStorageKind {
                        name: alloc::format!("component {second_index}"),
                    })?;
                let archetypes = &mut self.archetypes;
                let a = archetypes
                    .get_table_mut::<A>(entity, primary_index as u32, tick)
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<A>()),
                    })?;
                let b = store.get_mut_with_tick(entity, tick).ok_or_else(|| {
                    QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<B>()),
                    }
                })?;
                f(a, b, &mut effects)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_mut_read<A, B>(
        &mut self,
        primary_index: usize,
        primary_is_table: bool,
        second_index: usize,
        second_is_table: bool,
        entity: EntityId,
        tick: ChangeTick,
        mut f: impl FnMut(&mut A, &B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: 'static,
        B: 'static,
    {
        let run_guard = self.run_guard_state();
        let owner = self.owner_token();
        let command_queue = &mut self.command_queue;
        let allocator = &mut self.allocator;
        let events = &mut self.events;
        let mut effects =
            QueryEffects::from_parts(command_queue, allocator, events, run_guard, owner);

        match (primary_is_table, second_is_table) {
            (false, false) => {
                let (store_a, store_b) = split_sparse_stores_mut_read::<A, B>(
                    &mut self.sparse_stores,
                    primary_index,
                    second_index,
                )?;
                let a = store_a.get_mut_with_tick(entity, tick).ok_or_else(|| {
                    QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<A>()),
                    }
                })?;
                let b = store_b
                    .get(entity)
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<B>()),
                    })?;
                f(a, b, &mut effects)
            }
            (true, true) => {
                let (a, b) = self
                    .archetypes
                    .get_mut_read_table::<A, B>(
                        entity,
                        primary_index as u32,
                        second_index as u32,
                        tick,
                    )
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: String::from("entity missing query2 table components"),
                    })?;
                f(a, b, &mut effects)
            }
            (false, true) => {
                let store = self
                    .sparse_stores
                    .get_mut(primary_index)
                    .and_then(|store| store.typed_mut::<A>())
                    .ok_or_else(|| QueryError::WrongStorageKind {
                        name: alloc::format!("component {primary_index}"),
                    })?;
                let a = store.get_mut_with_tick(entity, tick).ok_or_else(|| {
                    QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<A>()),
                    }
                })?;
                let b = self
                    .archetypes
                    .get_table::<B>(entity, second_index as u32)
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<B>()),
                    })?;
                f(a, b, &mut effects)
            }
            (true, false) => {
                let b = self
                    .sparse_stores
                    .get(second_index)
                    .and_then(|store| store.typed::<B>())
                    .and_then(|store| store.get(entity))
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<B>()),
                    })?;
                let a = self
                    .archetypes
                    .get_table_mut::<A>(entity, primary_index as u32, tick)
                    .ok_or_else(|| QueryError::TraversalAborted {
                        entity,
                        detail: alloc::format!("entity missing {}", type_name::<A>()),
                    })?;
                f(a, b, &mut effects)
            }
        }
    }
}

#[allow(clippy::needless_lifetimes)]
fn split_sparse_stores_mut<'a, A: 'static, B: 'static>(
    stores: &'a mut [SparseStore],
    index_a: usize,
    index_b: usize,
) -> Result<(&'a mut TypedSparseStorage<A>, &'a mut TypedSparseStorage<B>), QueryError> {
    if index_a == index_b {
        return Err(QueryError::DuplicateMutableComponent {
            name: String::from("duplicate sparse component index"),
        });
    }
    if index_a < index_b {
        let (left, right) = stores.split_at_mut(index_b);
        let a = left[index_a]
            .typed_mut::<A>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_a}"),
            })?;
        let b = right[0]
            .typed_mut::<B>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_b}"),
            })?;
        Ok((a, b))
    } else {
        let (left, right) = stores.split_at_mut(index_a);
        let b = left[index_b]
            .typed_mut::<B>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_b}"),
            })?;
        let a = right[0]
            .typed_mut::<A>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_a}"),
            })?;
        Ok((a, b))
    }
}

#[allow(clippy::needless_lifetimes)]
fn split_sparse_stores_mut_read<'a, A: 'static, B: 'static>(
    stores: &'a mut [SparseStore],
    index_a: usize,
    index_b: usize,
) -> Result<(&'a mut TypedSparseStorage<A>, &'a TypedSparseStorage<B>), QueryError> {
    if index_a == index_b {
        return Err(QueryError::DuplicateMutableComponent {
            name: String::from("duplicate sparse component index"),
        });
    }
    if index_a < index_b {
        let (left, right) = stores.split_at_mut(index_b);
        let a = left[index_a]
            .typed_mut::<A>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_a}"),
            })?;
        let b = right[0]
            .typed::<B>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_b}"),
            })?;
        Ok((a, b))
    } else {
        let (left, right) = stores.split_at_mut(index_a);
        let b = left[index_b]
            .typed::<B>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_b}"),
            })?;
        let a = right[0]
            .typed_mut::<A>()
            .ok_or_else(|| QueryError::WrongStorageKind {
                name: alloc::format!("component {index_a}"),
            })?;
        Ok((a, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{QueryPolicy, QueryWindow};
    use crate::time::ChangeTick;
    use crate::world::{WorldBuilder, WorldError};

    fn noop_visit_two<A, B>(
        _: &mut A,
        _: &mut B,
        _: &mut QueryEffects<'_>,
    ) -> Result<(), QueryError> {
        Ok(())
    }

    fn noop_for_each2<A, B>(_: EntityId, _: &mut A, _: &mut B) -> Result<(), QueryError> {
        Ok(())
    }

    fn noop_for_each2_mut_read_effects<A, B>(
        _: EntityId,
        _: &mut A,
        _: &B,
        _: &mut QueryEffects<'_>,
    ) -> Result<(), QueryError> {
        Ok(())
    }

    fn noop_visit_mut_read<A, B>(
        _: &mut A,
        _: &B,
        _: &mut QueryEffects<'_>,
    ) -> Result<(), QueryError> {
        Ok(())
    }

    #[derive(Clone, Copy)]
    struct Pos(i32);

    #[derive(Clone, Copy)]
    struct Vel(i32);

    #[derive(Clone, Copy)]
    struct TableComp(i32);

    fn sparse_world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::sparse())
            .expect("vel");
        builder.build().expect("build")
    }

    fn mixed_world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("table");
        builder.build().expect("build")
    }

    #[test]
    fn issue_change_tick_query_maps_exhaustion() {
        let mut world = sparse_world();
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));
        assert!(matches!(
            world.issue_change_tick_query(),
            Err(QueryError::BorrowConflict { detail })
                if detail.contains("change tick exhausted")
        ));
    }

    #[test]
    fn preflight_rejects_poisoned_world() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("seed");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        world.insert(entity, Pos(2)).expect("consume");
        assert!(matches!(
            world.insert(entity, Pos(3)),
            Err(WorldError::ChangeTickExhausted)
        ));
        assert!(world.is_mutation_poisoned());
        assert!(matches!(
            world.preflight_change_ticks(1),
            Err(QueryError::BorrowConflict { detail })
                if detail.contains("world mutation is poisoned")
        ));
    }

    #[test]
    fn prepared_mutation_rejects_poisoned_world() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("seed");
        let mut query = world
            .prepare_query1::<Pos>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare");

        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        world.insert(entity, Pos(2)).expect("consume last tick");
        assert!(matches!(
            world.insert(entity, Pos(3)),
            Err(WorldError::ChangeTickExhausted)
        ));

        assert!(matches!(
            query.for_each_mut(&mut world, QueryWindow::All, |_, _| Ok(())),
            Err(QueryError::BorrowConflict { detail })
                if detail.contains("world mutation is poisoned")
        ));
    }

    #[test]
    fn prepared_pair_mutation_preflights_all_change_ticks() {
        let mut world = sparse_world();
        let a = world.spawn().expect("a");
        let b = world.spawn().expect("b");
        world.insert(a, Pos(1)).expect("a pos");
        world.insert(a, Vel(1)).expect("a vel");
        world.insert(b, Pos(2)).expect("b pos");
        world.insert(b, Vel(2)).expect("b vel");
        let mut query = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare");

        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));

        assert!(matches!(
            query.for_each_mut_mut(&mut world, QueryWindow::All, |_, _, _| Ok(())),
            Err(QueryError::BorrowConflict { detail })
                if detail.contains("insufficient change ticks for query mutation")
        ));
        assert_eq!(world.get::<Pos>(a).expect("get").expect("present").0, 1);
    }

    #[test]
    fn entity_has_query2_second_sparse_and_table() {
        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("sparse");
        world.insert(entity, TableComp(2)).expect("table");
        let pos_index = world.component_index::<Pos>().expect("pos");
        let table_index = world.component_index::<TableComp>().expect("table");
        assert!(world.entity_has_query2_second(entity, pos_index, false));
        assert!(world.entity_has_query2_second(entity, table_index, true));
        assert!(!world.entity_has_query2_second(entity, table_index, false));
    }

    #[test]
    fn sparse_dense_slots_returns_dense_indices() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        let index = world.component_index::<Pos>().expect("index");
        let slots = world.sparse_dense_slots(index).expect("slots");
        assert_eq!(slots.len(), 1);
    }

    #[test]
    fn resolve_cached_entities_without_cache_collects_live() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        let plan = world
            .resolve_query1_plan::<Pos>(&QuerySpec::new())
            .expect("plan");
        let params = QueryParams::new();
        let now = world.change_tick();
        let entities = world
            .resolve_cached_entities(&params, &plan, now, now)
            .expect("entities");
        assert_eq!(entities.len(), 1);
    }

    #[test]
    fn visit_two_mut_sparse_sparse_missing_second_aborts() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let vel_idx = world.component_index::<Vel>().expect("vel");
        assert!(matches!(
            world.visit_two_mut::<Pos, Vel>(
                pos_idx,
                false,
                vel_idx,
                false,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn visit_two_mut_mixed_table_sparse_paths_mutate() {
        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("sparse");
        world.insert(entity, TableComp(4)).expect("table");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let table_idx = world.component_index::<TableComp>().expect("table");

        world
            .visit_two_mut::<Pos, TableComp>(
                pos_idx,
                false,
                table_idx,
                true,
                entity,
                tick,
                |pos, table, _| {
                    pos.0 += table.0;
                    Ok(())
                },
            )
            .expect("sparse primary");
        assert_eq!(
            world.get::<Pos>(entity).expect("get").expect("present").0,
            5
        );

        let tick = world.issue_change_tick_query().expect("tick2");
        world
            .visit_two_mut::<TableComp, Pos>(
                table_idx,
                true,
                pos_idx,
                false,
                entity,
                tick,
                |table, pos, _| {
                    table.0 = pos.0;
                    Ok(())
                },
            )
            .expect("table primary");
        assert_eq!(
            world
                .get::<TableComp>(entity)
                .expect("get")
                .expect("present")
                .0,
            5
        );
    }

    #[derive(Clone, Copy)]
    struct Tag;

    fn tag_world() -> (World, crate::component::ComponentId) {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Tag>(ComponentOptions::tag())
            .expect("tag");
        (builder.build().expect("build"), tag)
    }

    #[test]
    fn visit_sparse_mut_wrong_storage_kind_and_missing_entity() {
        let (mut world, tag) = tag_world();
        let entity = world.spawn().expect("spawn");
        world.add_tag(entity, &tag).expect("tag");
        let tick = world.issue_change_tick_query().expect("tick");
        let tag_idx = tag.index();
        assert!(matches!(
            world.visit_sparse_mut::<Pos>(tag_idx, entity, tick, |_, _| Ok(())),
            Err(QueryError::WrongStorageKind { .. })
        ));

        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        assert!(matches!(
            world.visit_sparse_mut::<Pos>(pos_idx, entity, tick, |_, _| Ok(())),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn visit_table_mut_missing_component_aborts() {
        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        let tick = world.issue_change_tick_query().expect("tick");
        let table_idx = world.component_index::<TableComp>().expect("table") as u32;
        assert!(matches!(
            world.visit_table_mut::<TableComp>(table_idx, entity, tick, |_, _| Ok(())),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn visit_two_mut_sparse_sparse_missing_primary_aborts() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Vel(1)).expect("vel only");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let vel_idx = world.component_index::<Vel>().expect("vel");
        assert!(matches!(
            world.visit_two_mut::<Pos, Vel>(
                pos_idx,
                false,
                vel_idx,
                false,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn visit_two_mut_table_table_missing_component_aborts() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::table())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::table())
            .expect("vel");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos only");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let vel_idx = world.component_index::<Vel>().expect("vel");
        assert!(matches!(
            world.visit_two_mut::<Pos, Vel>(
                pos_idx,
                true,
                vel_idx,
                true,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn visit_two_mut_mixed_paths_abort_on_missing_components() {
        let mut world = mixed_world();
        let sparse_only = world.spawn().expect("sparse");
        world.insert(sparse_only, Pos(1)).expect("pos");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let table_idx = world.component_index::<TableComp>().expect("table");
        assert!(matches!(
            world.visit_two_mut::<Pos, TableComp>(
                pos_idx,
                false,
                table_idx,
                true,
                sparse_only,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { .. })
        ));

        let mut world = mixed_world();
        let table_only = world.spawn().expect("table");
        world.insert(table_only, TableComp(2)).expect("table");
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_two_mut::<TableComp, Pos>(
                table_idx,
                true,
                pos_idx,
                false,
                table_only,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn visit_two_mut_sparse_primary_missing_on_table_only_entity_aborts() {
        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(9)).expect("table only");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let table_idx = world.component_index::<TableComp>().expect("table");
        assert!(matches!(
            world.visit_two_mut::<Pos, TableComp>(
                pos_idx,
                false,
                table_idx,
                true,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { detail, .. }) if detail.contains("Pos")
        ));
    }

    fn pos_and_tag_world() -> (World, usize, usize) {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Tag>(ComponentOptions::tag())
            .expect("tag");
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        let world = builder.build().expect("build");
        let tag_idx = tag.index();
        let pos_idx = world.component_index::<Pos>().expect("pos");
        (world, tag_idx, pos_idx)
    }

    #[test]
    fn visit_two_mut_mixed_sparse_paths_reject_tag_storage() {
        let (mut world, tag_idx, pos_idx) = pos_and_tag_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_two_mut::<Pos, Tag>(
                pos_idx,
                false,
                tag_idx,
                false,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::WrongStorageKind { .. })
        ));

        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(1)).expect("table");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let table_idx = world.component_index::<TableComp>().expect("table");
        assert!(matches!(
            world.visit_two_mut::<TableComp, Pos>(
                table_idx,
                true,
                pos_idx,
                false,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
    }

    #[test]
    fn split_sparse_stores_mut_rejects_wrong_types_and_duplicate_index() {
        let (mut world, tag_idx, pos_idx) = pos_and_tag_world();
        let stores = &mut world.sparse_stores;
        assert!(matches!(
            split_sparse_stores_mut::<Pos, Pos>(stores, pos_idx, pos_idx),
            Err(QueryError::DuplicateMutableComponent { .. })
        ));
        assert!(matches!(
            split_sparse_stores_mut::<Pos, Vel>(stores, tag_idx, pos_idx),
            Err(QueryError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            split_sparse_stores_mut::<Pos, Vel>(stores, pos_idx, tag_idx),
            Err(QueryError::WrongStorageKind { .. })
        ));
    }

    #[derive(Clone, Copy)]
    struct Damage(#[allow(dead_code)] u32);

    #[test]
    fn query_effects_send_unregistered_event_aborts() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        world
            .begin_run(crate::operation::StageOperation::Update)
            .expect("run");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        assert!(matches!(
            world.visit_sparse_mut::<Pos>(pos_idx, entity, tick, |_pos, effects| {
                effects.send(Damage(1))
            }),
            Err(QueryError::WrongQuery { .. })
        ));
        world.end_run();
    }

    fn pos_first_tag_world() -> (World, usize, usize) {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        let tag = builder
            .register_component::<Tag>(ComponentOptions::tag())
            .expect("tag");
        let world = builder.build().expect("build");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let tag_idx = tag.index();
        (world, pos_idx, tag_idx)
    }

    #[test]
    fn visit_two_mut_sparse_sparse_invokes_noop_callback() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel(2)).expect("vel");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let vel_idx = world.component_index::<Vel>().expect("vel");
        world
            .visit_two_mut::<Pos, Vel>(pos_idx, false, vel_idx, false, entity, tick, noop_visit_two)
            .expect("noop callback");
    }

    #[test]
    fn split_sparse_stores_mut_hits_high_index_wrong_storage_branches() {
        let (mut world, pos_idx, tag_idx) = pos_first_tag_world();
        let stores = &mut world.sparse_stores;
        assert!(matches!(
            split_sparse_stores_mut::<Pos, Vel>(stores, pos_idx, tag_idx),
            Err(QueryError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            split_sparse_stores_mut::<Vel, Pos>(stores, tag_idx, pos_idx),
            Err(QueryError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn visit_two_mut_mixed_rejects_tag_sparse_storage_on_both_paths() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Tag>(ComponentOptions::tag())
            .expect("tag");
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TableComp(1)).expect("table");
        world.insert(entity, Pos(2)).expect("pos");
        let tick = world.issue_change_tick_query().expect("tick");
        let tag_idx = tag.index();
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let table_idx = world.component_index::<TableComp>().expect("table");

        assert!(matches!(
            world.visit_two_mut::<Tag, TableComp>(
                tag_idx,
                false,
                table_idx,
                true,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::WrongStorageKind { .. })
        ));

        let tick = world.issue_change_tick_query().expect("tick2");
        assert!(matches!(
            world.visit_two_mut::<TableComp, Tag>(
                table_idx,
                true,
                tag_idx,
                false,
                entity,
                tick,
                noop_visit_two
            ),
            Err(QueryError::WrongStorageKind { .. })
        ));

        let sparse_only = world.spawn().expect("sparse only");
        world.insert(sparse_only, Pos(3)).expect("pos");
        let tick = world.issue_change_tick_query().expect("tick3");
        assert!(matches!(
            world.visit_two_mut::<TableComp, Pos>(
                table_idx,
                true,
                pos_idx,
                false,
                sparse_only,
                tick,
                noop_visit_two
            ),
            Err(QueryError::TraversalAborted { detail, .. }) if detail.contains("TableComp")
        ));
    }

    use alloc::vec;

    use crate::query::{QueryParams, QuerySpec};

    fn pos_vel_world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::sparse())
            .expect("vel");
        builder.build().expect("build")
    }

    fn collect_mutated_positions(
        world: &mut World,
        spec: &QuerySpec,
        params: QueryParams<'_>,
    ) -> Vec<i32> {
        let mut values = Vec::new();
        world
            .for_each_mut::<Pos>(spec, params, |_, pos| {
                values.push(pos.0);
                pos.0 += 100;
                Ok(())
            })
            .expect("mutate");
        values
    }

    #[test]
    fn cached_for_each_mut_matches_uncached_without_temporal_filter() {
        let mut world = pos_vel_world();
        let a = world.spawn().expect("a");
        let b = world.spawn().expect("b");
        world.insert(a, Pos(1)).expect("a");
        world.insert(b, Pos(2)).expect("b");

        let spec = QuerySpec::new();
        let cache = world.build_query_cache::<Pos>(spec.clone()).expect("cache");

        let uncached = collect_mutated_positions(&mut world, &spec, QueryParams::new());
        world.insert(a, Pos(1)).expect("reset a");
        world.insert(b, Pos(2)).expect("reset b");
        let cached = collect_mutated_positions(
            &mut world,
            &spec,
            QueryParams::new().membership_cache(&cache),
        );
        assert_eq!(uncached, cached);
        assert_eq!(uncached, vec![1, 2]);
    }

    #[test]
    fn cached_for_each_mut_respects_added_window() {
        let mut world = pos_vel_world();
        let old = world.spawn().expect("old");
        world.insert(old, Pos(1)).expect("old");
        let since_after_old = world.change_tick();

        let new = world.spawn().expect("new");
        world.insert(new, Pos(2)).expect("new");

        let spec = QuerySpec::new().added::<Pos>();
        let cache = world.build_query_cache::<Pos>(spec.clone()).expect("cache");
        let params = QueryParams::new()
            .membership_cache(&cache)
            .since(since_after_old);

        let uncached = collect_mutated_positions(&mut world, &spec, params);
        world.insert(old, Pos(1)).expect("reset old");
        world.insert(new, Pos(2)).expect("reset new");
        let cached = collect_mutated_positions(
            &mut world,
            &spec,
            QueryParams::new()
                .membership_cache(&cache)
                .since(since_after_old),
        );
        assert_eq!(uncached, cached);
        assert_eq!(uncached, vec![2]);
        assert_eq!(
            world.get::<Pos>(old).expect("get").expect("present").0,
            1,
            "older structural member must not be mutated"
        );
    }

    #[test]
    fn cached_for_each_mut_respects_changed_window() {
        let mut world = pos_vel_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        let since_before_change = world.change_tick();
        world
            .get_mut::<Pos>(entity)
            .expect("get")
            .expect("present")
            .0 = 9;

        let spec = QuerySpec::new().changed::<Pos>();
        let cache = world.build_query_cache::<Pos>(spec.clone()).expect("cache");
        let params = QueryParams::new()
            .membership_cache(&cache)
            .since(since_before_change);

        let uncached = collect_mutated_positions(&mut world, &spec, params);
        world.insert(entity, Pos(9)).expect("reset");
        let cached = collect_mutated_positions(
            &mut world,
            &spec,
            QueryParams::new()
                .membership_cache(&cache)
                .since(since_before_change),
        );
        assert_eq!(uncached, cached);
        assert_eq!(uncached, vec![9]);
    }

    #[test]
    fn cached_for_each2_mut_respects_added_window() {
        let mut world = pos_vel_world();
        let old = world.spawn().expect("old");
        world.insert(old, Pos(1)).expect("old pos");
        world.insert(old, Vel(10)).expect("old vel");
        let since_after_old = world.change_tick();

        let new = world.spawn().expect("new");
        world.insert(new, Pos(2)).expect("new pos");
        world.insert(new, Vel(20)).expect("new vel");

        let spec = QuerySpec::new().added::<Pos>();
        let cache = world
            .build_query2_cache::<Pos, Vel>(spec.clone())
            .expect("cache");
        let params = QueryParams::new()
            .membership_cache(&cache)
            .since(since_after_old);

        let mut uncached = Vec::new();
        world
            .for_each2_mut::<Pos, Vel>(&spec, params, |_, pos, vel| {
                uncached.push((pos.0, vel.0));
                pos.0 += 1;
                Ok(())
            })
            .expect("uncached");

        world.insert(old, Pos(1)).expect("reset old pos");
        world.insert(new, Pos(2)).expect("reset new pos");

        let mut cached = Vec::new();
        world
            .for_each2_mut::<Pos, Vel>(
                &spec,
                QueryParams::new()
                    .membership_cache(&cache)
                    .since(since_after_old),
                |_, pos, vel| {
                    cached.push((pos.0, vel.0));
                    pos.0 += 1;
                    Ok(())
                },
            )
            .expect("cached");

        assert_eq!(uncached, cached);
        assert_eq!(uncached, vec![(2, 20)]);
        assert_eq!(
            world.get::<Pos>(old).expect("get").expect("present").0,
            1,
            "older structural member must not be mutated"
        );
    }

    #[test]
    fn visit_two_mut_table_table_pair_mutates() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::table())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::table())
            .expect("vel");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel(2)).expect("vel");
        let tick = world.issue_change_tick_query().expect("tick");
        let pos_idx = world.component_index::<Pos>().expect("pos");
        let vel_idx = world.component_index::<Vel>().expect("vel");
        world
            .visit_two_mut::<Pos, Vel>(pos_idx, true, vel_idx, true, entity, tick, |pos, vel, _| {
                pos.0 += vel.0;
                Ok(())
            })
            .expect("table pair");
        assert_eq!(
            world.get::<Pos>(entity).expect("get").expect("present").0,
            3
        );
    }

    #[test]
    fn adhoc_effect_wrappers_cover_table_q1_uncached_q2_and_duplicate_mutable_error() {
        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, TableComp(2)).expect("table");
        world
            .for_each_mut_with_effects::<TableComp>(
                &QuerySpec::new(),
                QueryParams::new(),
                |_, table, _| {
                    table.0 += 1;
                    Ok(())
                },
            )
            .expect("table q1");
        world
            .for_each2_mut_with_effects::<Pos, TableComp>(
                &QuerySpec::new(),
                QueryParams::new(),
                |_, pos, table, _| {
                    pos.0 += table.0;
                    table.0 += 1;
                    Ok(())
                },
            )
            .expect("uncached q2");
        assert_eq!(world.get::<Pos>(entity).expect("get").expect("pos").0, 4);
        assert!(matches!(
            world.for_each2_mut::<Pos, Pos>(&QuerySpec::new(), QueryParams::new(), noop_for_each2,),
            Err(QueryError::DuplicateMutableComponent { .. })
        ));
        world
            .for_each2_mut::<Pos, TableComp>(&QuerySpec::new(), QueryParams::new(), noop_for_each2)
            .expect("execute noop callback");
    }

    #[test]
    fn adhoc_mutation_uses_result_cache_for_q1_and_q2() {
        let mut world = pos_vel_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel(2)).expect("vel");
        let spec = QuerySpec::new();
        let q1_result = world
            .build_query_result_cache::<Pos>(spec.clone())
            .expect("q1 result");
        world
            .for_each_mut::<Pos>(
                &spec,
                QueryParams::new().result_cache(&q1_result),
                |_, pos| {
                    pos.0 += 1;
                    Ok(())
                },
            )
            .expect("q1 mutate");
        let q2_result = world
            .build_query2_result_cache::<Pos, Vel>(spec.clone())
            .expect("q2 result");
        world
            .for_each2_mut::<Pos, Vel>(
                &spec,
                QueryParams::new().result_cache(&q2_result),
                |_, pos, vel| {
                    pos.0 += vel.0;
                    Ok(())
                },
            )
            .expect("q2 mutate");
        assert_eq!(world.get::<Pos>(entity).expect("get").expect("pos").0, 4);
    }

    #[test]
    fn prepared_resolved_q2_mut_mut_and_mut_read_execute_and_reject_duplicate_types() {
        use crate::query::{QueryPolicy, QueryWindow};

        let mut world = pos_vel_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel(2)).expect("vel");
        let mut mut_mut = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare mut mut");
        mut_mut
            .for_each_mut_mut(&mut world, QueryWindow::All, |_, pos, vel| {
                pos.0 += vel.0;
                vel.0 += 1;
                Ok(())
            })
            .expect("mut mut");
        let mut mut_read = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare mut read");
        mut_read
            .for_each_mut_read_with_effects(
                &mut world,
                QueryWindow::All,
                noop_for_each2_mut_read_effects,
            )
            .expect("mut read");
        assert_eq!(world.get::<Pos>(entity).expect("get").expect("pos").0, 3);

        let plan = world
            .resolve_query1_plan::<Pos>(&QuerySpec::new())
            .expect("plan");
        let mut scratch = Vec::new();
        let now = world.change_tick();
        assert!(matches!(
            world.for_each2_mut_read_resolved::<Pos, Pos>(
                &plan,
                plan.primary_index,
                plan.primary_is_table,
                None,
                &mut scratch,
                ChangeTick::ZERO,
                now,
                noop_for_each2_mut_read_effects,
            ),
            Err(QueryError::DuplicateMutableComponent { .. })
        ));
    }

    #[test]
    fn visit_mut_read_covers_all_storage_pairings() {
        let mut world = sparse_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel(2)).expect("vel");
        let pos = world.component_index::<Pos>().expect("pos index");
        let vel = world.component_index::<Vel>().expect("vel index");
        let tick = world.issue_change_tick_query().expect("tick");
        let mut sparse_sparse_visits = 0;
        world
            .visit_mut_read::<Pos, Vel>(pos, false, vel, false, entity, tick, |pos, vel, _| {
                sparse_sparse_visits += 1;
                pos.0 += vel.0;
                Ok(())
            })
            .expect("sparse sparse");
        assert_eq!(sparse_sparse_visits, 1);
        assert_eq!(world.get::<Pos>(entity).expect("get").expect("pos").0, 3);
        assert_eq!(world.get::<Vel>(entity).expect("get").expect("vel").0, 2);

        let mut world = mixed_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(3)).expect("pos");
        world.insert(entity, TableComp(4)).expect("table");
        let pos = world.component_index::<Pos>().expect("pos index");
        let table = world.component_index::<TableComp>().expect("table index");
        let tick = world.issue_change_tick_query().expect("tick");
        let mut sparse_table_visits = 0;
        world
            .visit_mut_read::<Pos, TableComp>(
                pos,
                false,
                table,
                true,
                entity,
                tick,
                |pos, table, _| {
                    sparse_table_visits += 1;
                    pos.0 += table.0;
                    Ok(())
                },
            )
            .expect("sparse table");
        let tick = world.issue_change_tick_query().expect("tick");
        let mut table_sparse_visits = 0;
        world
            .visit_mut_read::<TableComp, Pos>(
                table,
                true,
                pos,
                false,
                entity,
                tick,
                |table, pos, _| {
                    table_sparse_visits += 1;
                    table.0 += pos.0;
                    Ok(())
                },
            )
            .expect("table sparse");
        assert_eq!(sparse_table_visits, 1);
        assert_eq!(table_sparse_visits, 1);
        assert_eq!(world.get::<Pos>(entity).expect("get").expect("pos").0, 7);
        assert_eq!(
            world
                .get::<TableComp>(entity)
                .expect("get")
                .expect("table")
                .0,
            11
        );

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::table())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::table())
            .expect("vel");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(5)).expect("pos");
        world.insert(entity, Vel(6)).expect("vel");
        let pos = world.component_index::<Pos>().expect("pos index");
        let vel = world.component_index::<Vel>().expect("vel index");
        let tick = world.issue_change_tick_query().expect("tick");
        let mut table_table_visits = 0;
        world
            .visit_mut_read::<Pos, Vel>(pos, true, vel, true, entity, tick, |pos, vel, _| {
                table_table_visits += 1;
                pos.0 += vel.0;
                Ok(())
            })
            .expect("table table");
        assert_eq!(table_table_visits, 1);
        assert_eq!(world.get::<Pos>(entity).expect("get").expect("pos").0, 11);
        assert_eq!(world.get::<Vel>(entity).expect("get").expect("vel").0, 6);

        let tick = world.issue_change_tick_query().expect("tick");
        world
            .visit_mut_read::<Pos, Vel>(pos, true, vel, true, entity, tick, noop_visit_mut_read)
            .expect("table table noop callback");
    }

    #[test]
    fn prepared_resolved_q2_propagates_mut_mut_and_mut_read_callback_errors() {
        use crate::query::{QueryPolicy, QueryWindow};

        let mut world = pos_vel_world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        world.insert(entity, Vel(2)).expect("vel");

        let mut mut_mut = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare mut mut");
        assert!(matches!(
            mut_mut.for_each_mut_mut(&mut world, QueryWindow::All, |entity, _, _| {
                Err(QueryError::TraversalAborted {
                    entity,
                    detail: String::from("mut-mut callback stopped"),
                })
            }),
            Err(QueryError::TraversalAborted { detail, .. })
                if detail == "mut-mut callback stopped"
        ));

        let mut mut_read = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare mut read");
        assert!(matches!(
            mut_read.for_each_mut_read(&mut world, QueryWindow::All, |entity, _, _| {
                Err(QueryError::TraversalAborted {
                    entity,
                    detail: String::from("mut-read callback stopped"),
                })
            }),
            Err(QueryError::TraversalAborted { detail, .. })
                if detail == "mut-read callback stopped"
        ));
    }

    #[test]
    fn split_sparse_stores_mut_read_covers_duplicate_and_wrong_type_branches() {
        let mut world = sparse_world();
        let pos = world.component_index::<Pos>().expect("pos");
        assert!(matches!(
            split_sparse_stores_mut_read::<Pos, Pos>(&mut world.sparse_stores, pos, pos),
            Err(QueryError::DuplicateMutableComponent { .. })
        ));

        let (mut world, tag, pos) = pos_and_tag_world();
        assert!(matches!(
            split_sparse_stores_mut_read::<Tag, Pos>(&mut world.sparse_stores, tag, pos),
            Err(QueryError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            split_sparse_stores_mut_read::<Pos, Tag>(&mut world.sparse_stores, pos, tag),
            Err(QueryError::WrongStorageKind { .. })
        ));
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("pos");
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<Tag, Pos>(
                tag,
                false,
                pos,
                false,
                entity,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::WrongStorageKind { .. })
        ));

        let mut world = sparse_world();
        let pos = world.component_index::<Pos>().expect("pos");
        let vel = world.component_index::<Vel>().expect("vel");
        split_sparse_stores_mut_read::<Vel, Pos>(&mut world.sparse_stores, vel, pos)
            .expect("reverse-index success");

        let (mut world, pos, tag) = pos_first_tag_world();
        assert!(matches!(
            split_sparse_stores_mut_read::<Pos, Tag>(&mut world.sparse_stores, pos, tag),
            Err(QueryError::WrongStorageKind { .. })
        ));
        assert!(matches!(
            split_sparse_stores_mut_read::<Tag, Pos>(&mut world.sparse_stores, tag, pos),
            Err(QueryError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn visit_mut_read_reports_missing_components_for_every_storage_pairing() {
        let mut world = sparse_world();
        let pos_only = world.spawn().expect("pos only");
        world.insert(pos_only, Pos(1)).expect("pos");
        let vel_only = world.spawn().expect("vel only");
        world.insert(vel_only, Vel(2)).expect("vel");
        let pos = world.component_index::<Pos>().expect("pos index");
        let vel = world.component_index::<Vel>().expect("vel index");
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<Pos, Vel>(
                pos,
                false,
                vel,
                false,
                pos_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<Pos, Vel>(
                pos,
                false,
                vel,
                false,
                vel_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));

        let mut world = mixed_world();
        let sparse_only = world.spawn().expect("sparse only");
        world.insert(sparse_only, Pos(3)).expect("pos");
        let table_only = world.spawn().expect("table only");
        world.insert(table_only, TableComp(4)).expect("table");
        let pos = world.component_index::<Pos>().expect("pos index");
        let table = world.component_index::<TableComp>().expect("table index");
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<Pos, TableComp>(
                pos,
                false,
                table,
                true,
                sparse_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<Pos, TableComp>(
                pos,
                false,
                table,
                true,
                table_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<TableComp, Pos>(
                table,
                true,
                pos,
                false,
                table_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<TableComp, Pos>(
                table,
                true,
                pos,
                false,
                sparse_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));

        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Tag>(ComponentOptions::tag())
            .expect("tag");
        builder
            .register_component::<TableComp>(ComponentOptions::table())
            .expect("table");
        let mut wrong_storage = builder.build().expect("world");
        let entity = wrong_storage.spawn().expect("spawn");
        wrong_storage.insert(entity, TableComp(9)).expect("table");
        let table = wrong_storage
            .component_index::<TableComp>()
            .expect("table index");
        let tick = wrong_storage.issue_change_tick_query().expect("tick");
        assert!(matches!(
            wrong_storage.visit_mut_read::<Tag, TableComp>(
                tag.index(),
                false,
                table,
                true,
                entity,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::WrongStorageKind { .. })
        ));

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Pos>(ComponentOptions::table())
            .expect("pos");
        builder
            .register_component::<Vel>(ComponentOptions::table())
            .expect("vel");
        let mut world = builder.build().expect("world");
        let pos_only = world.spawn().expect("spawn");
        world.insert(pos_only, Pos(5)).expect("pos");
        let pos = world.component_index::<Pos>().expect("pos index");
        let vel = world.component_index::<Vel>().expect("vel index");
        let tick = world.issue_change_tick_query().expect("tick");
        assert!(matches!(
            world.visit_mut_read::<Pos, Vel>(
                pos,
                true,
                vel,
                true,
                pos_only,
                tick,
                noop_visit_mut_read,
            ),
            Err(QueryError::TraversalAborted { .. })
        ));
    }
}
