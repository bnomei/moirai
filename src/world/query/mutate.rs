use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;

use crate::entity::EntityId;
use crate::query::{QueryEffects, QueryError, QueryParams, QuerySpec};
use crate::storage::{SparseStore, TypedSparseStorage};
use crate::time::ChangeTick;
use crate::world::World;

use super::collect::{collect_query1_entities, collect_query2_entities};
use super::filter::validate_exact_ids;
use super::spec::{resolve_query1, resolve_query2};

impl World {
    pub fn for_each_mut<T>(
        &mut self,
        spec: QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut T) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: Clone + 'static,
    {
        self.for_each_mut_inner(spec, params, |entity, value, _| f(entity, value))
    }

    pub fn for_each_mut_with_effects<T>(
        &mut self,
        spec: QuerySpec,
        params: QueryParams<'_>,
        f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: Clone + 'static,
    {
        self.for_each_mut_inner(spec, params, f)
    }

    pub fn for_each2_mut<A, B>(
        &mut self,
        spec: QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut A, &mut B) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: Clone + 'static,
        B: Clone + 'static,
    {
        self.for_each2_mut_inner(spec, params, |entity, a, b, _| f(entity, a, b))
    }

    pub fn for_each2_mut_with_effects<A, B>(
        &mut self,
        spec: QuerySpec,
        params: QueryParams<'_>,
        f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: Clone + 'static,
        B: Clone + 'static,
    {
        self.for_each2_mut_inner(spec, params, f)
    }

    fn for_each_mut_inner<T>(
        &mut self,
        spec: QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        T: Clone + 'static,
    {
        let mut params = params;
        let plan = resolve_query1::<T>(self, &spec)?;
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
        spec: QuerySpec,
        params: QueryParams<'_>,
        mut f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>
    where
        A: Clone + 'static,
        B: Clone + 'static,
    {
        let mut params = params;
        let (plan, second_index, second_is_table) = resolve_query2::<A, B>(self, &spec)?;
        validate_exact_ids(self, &plan)?;
        if plan.primary_index == second_index {
            return Err(QueryError::DuplicateMutableComponent {
                name: String::from(type_name::<A>()),
            });
        }
        self.validate_query_params_caches(&params, &plan)?;
        let captured_now = self.change_tick();
        let since = params.since_tick(plan.fingerprint, self)?;
        let entities = if params.membership_cache.is_some() || params.result_cache.is_some() {
            self.resolve_cached_entities(&params, &plan, since, captured_now)?
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

    pub(crate) fn resolve_cached_entities(
        &mut self,
        params: &QueryParams<'_>,
        plan: &super::plan::ResolvedPlan,
        since: ChangeTick,
        captured_now: ChangeTick,
    ) -> Result<Vec<EntityId>, QueryError> {
        if let Some(cache) = params.membership_cache {
            return Ok(self.refresh_membership_cache(cache, plan)?.to_vec());
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
        T: Clone + 'static,
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
        T: Clone + 'static,
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
        A: Clone + 'static,
        B: Clone + 'static,
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
