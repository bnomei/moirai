use alloc::rc::Rc;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::entity::EntityId;
use crate::query::{Query1, Query2, QueryCursor, QueryEffects, QueryError, QuerySpec};
use crate::time::ChangeTick;
use crate::world::query::cache::QueryTopologySnapshot;
use crate::world::query::collect::{collect_query1_entities, collect_query1_structural_members};
use crate::world::query::filter::validate_exact_ids;
use crate::world::query::plan::{ResolvedPlan, TraversalSource};
use crate::world::{World, WorldOwner};

/// Execution policy for a prepared query.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum QueryPolicy {
    /// Reuse only the resolved query plan and traverse storage directly.
    #[default]
    Prepared,
    /// Materialize structural membership and apply the active window per execution.
    Membership,
    /// Maintain structural membership incrementally through a reverse slot index.
    DeltaMembership,
    /// Materialize the complete result set. Temporal selectors are unsupported.
    Result,
}

/// Temporal window used for one prepared-query execution.
pub enum QueryWindow<'a> {
    All,
    Since(ChangeTick),
    Cursor(&'a mut QueryCursor),
}

impl<'a> QueryWindow<'a> {
    pub const fn all() -> Self {
        Self::All
    }

    pub const fn since(tick: ChangeTick) -> Self {
        Self::Since(tick)
    }

    pub fn cursor(cursor: &mut QueryCursor) -> QueryWindow<'_> {
        QueryWindow::Cursor(cursor)
    }

    pub(crate) fn into_parts(
        self,
        world: &World,
        fingerprint: u64,
    ) -> Result<(ChangeTick, Option<&'a mut QueryCursor>), QueryError> {
        match self {
            Self::All => Ok((ChangeTick::ZERO, None)),
            Self::Since(tick) => Ok((tick, None)),
            Self::Cursor(cursor) => {
                cursor.validate(world, fingerprint)?;
                Ok((cursor.since(), Some(cursor)))
            }
        }
    }
}

/// Reusable, owner-scoped single-component query plan.
pub struct PreparedQuery1<T: 'static> {
    pub(crate) owner: WorldOwner,
    pub(crate) plan: Rc<ResolvedPlan>,
    materialization: Materialization,
    mutation_scratch: Vec<EntityId>,
    marker: PhantomData<fn() -> T>,
}

/// Reusable, owner-scoped two-component query plan.
pub struct PreparedQuery2<A: 'static, B: 'static> {
    pub(crate) owner: WorldOwner,
    pub(crate) plan: Rc<ResolvedPlan>,
    pub(crate) second_index: usize,
    pub(crate) second_is_table: bool,
    driver_revisions: (u64, u64),
    materialization: Materialization,
    mutation_scratch: Vec<EntityId>,
    marker: PhantomData<fn() -> (A, B)>,
}

enum Materialization {
    Prepared,
    Membership(MaterializedSet),
    DeltaMembership(DeltaSet),
    Result(MaterializedSet),
}

struct MaterializedSet {
    topology: QueryTopologySnapshot,
    ids: Vec<EntityId>,
}

struct DeltaSet {
    topology: QueryTopologySnapshot,
    ids: Vec<EntityId>,
    reverse: Vec<Option<usize>>,
    changed: Vec<EntityId>,
    changed_reverse: Vec<Option<usize>>,
    cursor: Rc<core::cell::Cell<u64>>,
}

impl Materialization {
    fn build(
        world: &mut World,
        plan: &ResolvedPlan,
        policy: QueryPolicy,
    ) -> Result<Self, QueryError> {
        if !matches!(policy, QueryPolicy::Prepared)
            && matches!(plan.traversal, TraversalSource::Exact { .. })
        {
            return Err(QueryError::UnsupportedCachePolicy {
                detail: alloc::string::String::from(
                    "materialized policies do not support exact-id query order",
                ),
            });
        }
        if matches!(policy, QueryPolicy::Result)
            && (!plan.added_indices.is_empty() || !plan.changed_indices.is_empty())
        {
            return Err(QueryError::MovingChangeWindow);
        }

        let topology = || QueryTopologySnapshot::capture(world, plan);
        Ok(match policy {
            QueryPolicy::Prepared => Self::Prepared,
            QueryPolicy::Membership => Self::Membership(MaterializedSet {
                topology: topology(),
                ids: collect_query1_structural_members(world, plan),
            }),
            QueryPolicy::DeltaMembership => {
                let ids = collect_query1_structural_members(world, plan);
                let reverse = build_reverse(&ids);
                Self::DeltaMembership(DeltaSet {
                    topology: topology(),
                    ids,
                    reverse,
                    changed: Vec::new(),
                    changed_reverse: Vec::new(),
                    cursor: world.register_query_delta_cursor(),
                })
            }
            QueryPolicy::Result => Self::Result(MaterializedSet {
                topology: topology(),
                ids: collect_query1_entities(world, plan, ChangeTick::ZERO, world.change_tick()),
            }),
        })
    }

    fn refresh(&mut self, world: &World, plan: &ResolvedPlan) {
        match self {
            Self::Prepared => {}
            Self::Membership(set) => {
                if topology_changed(&mut set.topology, world) {
                    set.ids = collect_query1_structural_members(world, plan);
                    set.topology = QueryTopologySnapshot::capture(world, plan);
                }
            }
            Self::DeltaMembership(set) => {
                if set.topology.observed_global_revision() != world.query_topology_revision() {
                    world.collect_query_delta_entities(
                        &set.cursor,
                        plan,
                        &mut set.changed,
                        &mut set.changed_reverse,
                    );
                    for index in 0..set.changed.len() {
                        let entity = set.changed[index];
                        update_delta_entity(set, world, plan, entity);
                    }
                    set.topology = QueryTopologySnapshot::capture(world, plan);
                }
            }
            Self::Result(set) => {
                if topology_changed(&mut set.topology, world) {
                    set.ids =
                        collect_query1_entities(world, plan, ChangeTick::ZERO, world.change_tick());
                    set.topology = QueryTopologySnapshot::capture(world, plan);
                }
            }
        }
    }

    fn ids_and_temporal_filter(&self, plan: &ResolvedPlan) -> Option<(&[EntityId], bool)> {
        let apply_temporal = !plan.added_indices.is_empty() || !plan.changed_indices.is_empty();
        match self {
            Self::Prepared => None,
            Self::Membership(set) => Some((&set.ids, apply_temporal)),
            Self::DeltaMembership(set) => Some((&set.ids, apply_temporal)),
            Self::Result(set) => Some((&set.ids, false)),
        }
    }
}

fn topology_changed(topology: &mut QueryTopologySnapshot, world: &World) -> bool {
    let revision = world.query_topology_revision();
    if topology.observed_global_revision() == revision {
        return false;
    }
    if topology.dependencies_are_current(world) {
        topology.observe_global_revision(revision);
        false
    } else {
        true
    }
}

fn build_reverse(ids: &[EntityId]) -> Vec<Option<usize>> {
    let Some(max_slot) = ids.iter().map(|entity| entity.slot() as usize).max() else {
        return Vec::new();
    };
    let mut reverse = alloc::vec![None; max_slot + 1];
    for (index, entity) in ids.iter().enumerate() {
        reverse[entity.slot() as usize] = Some(index);
    }
    reverse
}

fn update_delta_entity(set: &mut DeltaSet, world: &World, plan: &ResolvedPlan, entity: EntityId) {
    let slot = entity.slot() as usize;
    let existing = set.reverse.get(slot).and_then(|index| *index);
    let matches = crate::world::query::filter::entity_matches_structural(world, entity, plan);

    if let Some(index) = existing {
        if set.ids.get(index) == Some(&entity) && matches {
            return;
        }
        remove_delta_index(set, index);
    }
    if matches {
        if set.reverse.len() <= slot {
            set.reverse.resize(slot + 1, None);
        }
        let index = set.ids.len();
        set.ids.push(entity);
        set.reverse[slot] = Some(index);
    }
}

fn remove_delta_index(set: &mut DeltaSet, index: usize) {
    let removed = set.ids.swap_remove(index);
    set.reverse[removed.slot() as usize] = None;
    if let Some(&moved) = set.ids.get(index) {
        set.reverse[moved.slot() as usize] = Some(index);
    }
}

impl World {
    pub fn prepare_query1<T: 'static>(
        &mut self,
        spec: QuerySpec,
        policy: QueryPolicy,
    ) -> Result<PreparedQuery1<T>, QueryError> {
        let plan = self.resolve_query1_plan::<T>(&spec)?;
        validate_exact_ids(self, &plan)?;
        let materialization = Materialization::build(self, &plan, policy)?;
        Ok(PreparedQuery1 {
            owner: self.owner_token(),
            plan,
            materialization,
            mutation_scratch: Vec::new(),
            marker: PhantomData,
        })
    }

    pub fn prepare_query2<A: 'static, B: 'static>(
        &mut self,
        spec: QuerySpec,
        policy: QueryPolicy,
    ) -> Result<PreparedQuery2<A, B>, QueryError> {
        let (plan, second_index, second_is_table) = self.resolve_query2_plan::<A, B>(&spec)?;
        validate_exact_ids(self, &plan)?;
        let materialization = Materialization::build(self, &plan, policy)?;
        Ok(PreparedQuery2 {
            owner: self.owner_token(),
            plan,
            second_index,
            second_is_table,
            // Force the cached semantic plan's physical driver to be selected
            // from the current populations, even if the plan cache was filled
            // before a cardinality crossover.
            driver_revisions: (u64::MAX, u64::MAX),
            materialization,
            mutation_scratch: Vec::new(),
            marker: PhantomData,
        })
    }
}

impl<T: 'static> PreparedQuery1<T> {
    pub fn iter<'w, 'c>(
        &'w mut self,
        world: &'w mut World,
        window: QueryWindow<'c>,
    ) -> Result<Query1<'w, 'c, T>, QueryError> {
        self.validate_world(world)?;
        validate_exact_ids(world, &self.plan)?;
        let captured_now = world.change_tick();
        let (since, cursor) = window.into_parts(world, self.plan.fingerprint)?;
        self.materialization.refresh(world, &self.plan);
        let materialized = self.materialization.ids_and_temporal_filter(&self.plan);

        let table_component = match self.plan.traversal {
            TraversalSource::Table { component_index } => Some(component_index),
            _ => None,
        };
        if let Some(index) = table_component {
            world.ensure_table_archetypes(index);
        }
        let table_archetypes = table_component.map(|index| {
            world
                .table_archetypes(index)
                .expect("table archetypes prepared")
        });
        Query1::new_prepared(
            world,
            self.plan.clone(),
            since,
            captured_now,
            cursor,
            materialized,
            table_archetypes,
        )
    }

    pub fn for_each_mut(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut T) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.for_each_mut_inner(world, window, f)
    }

    pub fn for_each_mut_with_effects(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.for_each_mut_effects_inner(world, window, f)
    }

    fn for_each_mut_inner(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        mut f: impl FnMut(EntityId, &mut T) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.for_each_mut_effects_inner(world, window, |entity, value, _| f(entity, value))
    }

    fn for_each_mut_effects_inner(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut T, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.validate_world(world)?;
        validate_exact_ids(world, &self.plan)?;
        let captured_now = world.change_tick();
        let (since, mut cursor) = window.into_parts(world, self.plan.fingerprint)?;
        self.materialization.refresh(world, &self.plan);
        world.for_each_mut_resolved(
            &self.plan,
            self.materialization.ids_and_temporal_filter(&self.plan),
            &mut self.mutation_scratch,
            since,
            captured_now,
            f,
        )?;
        if let Some(cursor) = cursor.as_mut() {
            cursor.commit(captured_now);
        }
        Ok(())
    }

    fn validate_world(&self, world: &World) -> Result<(), QueryError> {
        if self.owner.same(&world.owner_token()) {
            Ok(())
        } else {
            Err(QueryError::WrongOwner)
        }
    }
}

impl<A: 'static, B: 'static> PreparedQuery2<A, B> {
    pub fn iter<'w, 'c>(
        &'w mut self,
        world: &'w mut World,
        window: QueryWindow<'c>,
    ) -> Result<Query2<'w, 'c, A, B>, QueryError> {
        self.validate_world(world)?;
        self.refresh_physical_plan(world);
        validate_exact_ids(world, &self.plan)?;
        let captured_now = world.change_tick();
        let (since, cursor) = window.into_parts(world, self.plan.fingerprint)?;
        self.materialization.refresh(world, &self.plan);
        let materialized = self.materialization.ids_and_temporal_filter(&self.plan);

        let table_component = match self.plan.traversal {
            TraversalSource::Table { component_index } => Some(component_index),
            _ => None,
        };
        if let Some(index) = table_component {
            world.ensure_table_archetypes(index);
        }
        let table_archetypes = table_component.map(|index| {
            world
                .table_archetypes(index)
                .expect("table archetypes prepared")
        });
        Query2::new_prepared(
            world,
            self.plan.clone(),
            since,
            captured_now,
            cursor,
            materialized,
            table_archetypes,
            self.second_index,
            self.second_is_table,
        )
    }

    pub fn for_each_mut_mut(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        mut f: impl FnMut(EntityId, &mut A, &mut B) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.for_each_mut_mut_with_effects(world, window, |entity, a, b, _| f(entity, a, b))
    }

    pub fn for_each_mut_mut_with_effects(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.execute_mut(world, window, f)
    }

    pub fn for_each_mut_read(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        mut f: impl FnMut(EntityId, &mut A, &B) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.for_each_mut_read_with_effects(world, window, |entity, a, b, _| f(entity, a, b))
    }

    pub fn for_each_mut_read_with_effects(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut A, &B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.execute_mut_read(world, window, f)
    }

    fn execute_mut(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut A, &mut B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.validate_world(world)?;
        self.refresh_physical_plan(world);
        validate_exact_ids(world, &self.plan)?;
        let captured_now = world.change_tick();
        let (since, mut cursor) = window.into_parts(world, self.plan.fingerprint)?;
        self.materialization.refresh(world, &self.plan);
        world.for_each2_mut_resolved(
            &self.plan,
            self.second_index,
            self.second_is_table,
            self.materialization.ids_and_temporal_filter(&self.plan),
            &mut self.mutation_scratch,
            since,
            captured_now,
            f,
        )?;
        if let Some(cursor) = cursor.as_mut() {
            cursor.commit(captured_now);
        }
        Ok(())
    }

    fn execute_mut_read(
        &mut self,
        world: &mut World,
        window: QueryWindow<'_>,
        f: impl FnMut(EntityId, &mut A, &B, &mut QueryEffects<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.validate_world(world)?;
        self.refresh_physical_plan(world);
        validate_exact_ids(world, &self.plan)?;
        let captured_now = world.change_tick();
        let (since, mut cursor) = window.into_parts(world, self.plan.fingerprint)?;
        self.materialization.refresh(world, &self.plan);
        world.for_each2_mut_read_resolved(
            &self.plan,
            self.second_index,
            self.second_is_table,
            self.materialization.ids_and_temporal_filter(&self.plan),
            &mut self.mutation_scratch,
            since,
            captured_now,
            f,
        )?;
        if let Some(cursor) = cursor.as_mut() {
            cursor.commit(captured_now);
        }
        Ok(())
    }

    fn validate_world(&self, world: &World) -> Result<(), QueryError> {
        if self.owner.same(&world.owner_token()) {
            Ok(())
        } else {
            Err(QueryError::WrongOwner)
        }
    }

    fn refresh_physical_plan(&mut self, world: &World) {
        if matches!(self.plan.traversal, TraversalSource::Exact { .. }) {
            return;
        }

        let revisions = (
            world.query_component_topology_revision(self.plan.primary_index),
            world.query_component_topology_revision(self.second_index),
        );
        if self.driver_revisions == revisions {
            return;
        }

        let primary_len =
            world.query_component_population(self.plan.primary_index, self.plan.primary_is_table);
        let second_len = world.query_component_population(self.second_index, self.second_is_table);
        let (component_index, is_table) = if second_len < primary_len {
            (self.second_index, self.second_is_table)
        } else {
            (self.plan.primary_index, self.plan.primary_is_table)
        };
        let traversal = if is_table {
            TraversalSource::Table { component_index }
        } else {
            TraversalSource::Sparse { component_index }
        };

        if !same_traversal(&self.plan.traversal, &traversal) {
            let mut plan = (*self.plan).clone();
            plan.traversal = traversal;
            self.plan = Rc::new(plan);
        }
        self.driver_revisions = revisions;
    }
}

fn same_traversal(left: &TraversalSource, right: &TraversalSource) -> bool {
    match (left, right) {
        (TraversalSource::All, TraversalSource::All) => true,
        (
            TraversalSource::Sparse {
                component_index: left,
            },
            TraversalSource::Sparse {
                component_index: right,
            },
        )
        | (
            TraversalSource::Table {
                component_index: left,
            },
            TraversalSource::Table {
                component_index: right,
            },
        ) => left == right,
        (TraversalSource::Exact { ids: left }, TraversalSource::Exact { ids: right }) => {
            left == right
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct A(i32);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct B(i32);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct C(i32);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct D(i32);

    fn world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<A>(ComponentOptions::sparse())
            .expect("A");
        builder
            .register_component::<B>(ComponentOptions::table())
            .expect("B");
        builder
            .register_component::<C>(ComponentOptions::sparse())
            .expect("C");
        builder
            .register_component::<D>(ComponentOptions::table())
            .expect("D");
        builder.build().expect("world")
    }

    #[test]
    fn reverse_indexed_delta_handles_add_remove_and_slot_reuse() {
        let mut world = world();
        let first = world.spawn().expect("first");
        world.insert(first, A(1)).expect("A");
        let plan = world
            .resolve_query1_plan::<A>(&QuerySpec::new())
            .expect("plan");
        let mut state =
            match Materialization::build(&mut world, &plan, QueryPolicy::DeltaMembership)
                .expect("delta")
            {
                Materialization::DeltaMembership(state) => state,
                _ => unreachable!(),
            };

        let second = world.spawn().expect("second");
        world.insert(second, A(2)).expect("A2");
        let third = world.spawn().expect("third");
        world.insert(third, A(3)).expect("A3");
        world.collect_query_delta_entities(
            &state.cursor,
            &plan,
            &mut state.changed,
            &mut state.changed_reverse,
        );
        let changed = state.changed.clone();
        assert_eq!(changed, alloc::vec![second, third]);
        for entity in changed {
            update_delta_entity(&mut state, &world, &plan, entity);
        }
        assert_eq!(state.ids, alloc::vec![first, second, third]);
        assert_eq!(state.reverse[second.slot() as usize], Some(1));

        world.remove::<A>(first).expect("remove");
        world.collect_query_delta_entities(
            &state.cursor,
            &plan,
            &mut state.changed,
            &mut state.changed_reverse,
        );
        let changed = state.changed.clone();
        assert_eq!(changed, alloc::vec![first]);
        for entity in changed {
            update_delta_entity(&mut state, &world, &plan, entity);
        }
        assert_eq!(state.ids.len(), 2);
        assert!(state.ids.contains(&second));
        assert!(state.ids.contains(&third));
        assert_eq!(state.reverse[first.slot() as usize], None);
        assert_eq!(
            state.reverse[second.slot() as usize],
            state.ids.iter().position(|&entity| entity == second)
        );

        world.despawn(second).expect("despawn");
        let replacement = world.spawn().expect("replacement");
        assert_eq!(replacement.slot(), second.slot());
        world.insert(replacement, A(4)).expect("replacement A");
        world.collect_query_delta_entities(
            &state.cursor,
            &plan,
            &mut state.changed,
            &mut state.changed_reverse,
        );
        let changed = state.changed.clone();
        // Changes are deduplicated by slot and retain the newest generation.
        assert_eq!(changed, alloc::vec![replacement]);
        for entity in changed {
            update_delta_entity(&mut state, &world, &plan, entity);
        }
        assert_eq!(state.ids.len(), 2);
        assert!(state.ids.contains(&third));
        assert!(state.ids.contains(&replacement));
        assert_eq!(
            state.reverse[third.slot() as usize],
            state.ids.iter().position(|&entity| entity == third)
        );
        assert_eq!(
            state.reverse[replacement.slot() as usize],
            state.ids.iter().position(|&entity| entity == replacement)
        );
    }

    #[test]
    fn materialized_policies_track_structural_changes() {
        for policy in [
            QueryPolicy::Membership,
            QueryPolicy::DeltaMembership,
            QueryPolicy::Result,
        ] {
            let mut world = world();
            let mut query = world
                .prepare_query1::<A>(QuerySpec::new(), policy)
                .expect("prepare");
            let entity = world.spawn().expect("spawn");
            world.insert(entity, A(3)).expect("insert");
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::All)
                    .expect("iter")
                    .count(),
                1
            );
            world.remove::<A>(entity).expect("remove");
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::All)
                    .expect("iter")
                    .count(),
                0
            );
        }
    }

    #[test]
    fn current_delta_query_skips_prefix_retained_for_lagging_query() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        let mut current = world
            .prepare_query1::<A>(QuerySpec::new(), QueryPolicy::DeltaMembership)
            .expect("current");
        let mut lagging = world
            .prepare_query1::<A>(QuerySpec::new(), QueryPolicy::DeltaMembership)
            .expect("lagging");

        for value in 0..32 {
            if value % 2 == 0 {
                world.insert(entity, A(value)).expect("insert");
            } else {
                world.remove::<A>(entity).expect("remove");
            }
            let expected = usize::from(value % 2 == 0);
            assert_eq!(
                current
                    .iter(&mut world, QueryWindow::All)
                    .expect("current refresh")
                    .count(),
                expected
            );
        }

        // The lagging cursor pins the full log, but both materializations still
        // converge on the same final membership when it eventually catches up.
        assert_eq!(world.query_delta_log_len_for_test(), 32);
        assert_eq!(
            lagging
                .iter(&mut world, QueryWindow::All)
                .expect("lagging refresh")
                .count(),
            0
        );
        assert_eq!(
            current
                .iter(&mut world, QueryWindow::All)
                .expect("current remains current")
                .count(),
            0
        );
    }

    #[test]
    fn delta_sequence_exhaustion_rebases_retained_log_and_cursors() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        let plan = world
            .resolve_query1_plan::<A>(&QuerySpec::new())
            .expect("plan");
        let cursor = world.register_query_delta_cursor();
        world.seed_query_delta_exhaustion_for_test(&cursor, entity, plan.primary_index);

        world.insert(entity, A(1)).expect("insert after rebase");
        assert_eq!(cursor.get(), 0);
        assert_eq!(world.query_delta_sequences_for_test(), alloc::vec![0, 1]);

        let mut changed = Vec::new();
        let mut reverse = Vec::new();
        world.collect_query_delta_entities(&cursor, &plan, &mut changed, &mut reverse);
        assert_eq!(changed, alloc::vec![entity]);
        assert_eq!(cursor.get(), 2);
    }

    #[test]
    fn delta_cursor_offsets_clamp_before_and_after_retained_range() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        let plan = world
            .resolve_query1_plan::<A>(&QuerySpec::new())
            .expect("plan");
        let cursor = world.register_query_delta_cursor();
        world.insert(entity, A(1)).expect("insert one");
        world.remove::<A>(entity).expect("remove one");
        world.insert(entity, A(2)).expect("insert two");

        cursor.set(2);
        world.remove::<A>(entity).expect("remove two");
        assert_eq!(world.query_delta_sequences_for_test(), alloc::vec![2, 3]);

        let mut changed = Vec::new();
        let mut reverse = Vec::new();
        cursor.set(0);
        world.collect_query_delta_entities(&cursor, &plan, &mut changed, &mut reverse);
        assert_eq!(changed, alloc::vec![entity]);
        assert_eq!(cursor.get(), 4);

        cursor.set(99);
        world.collect_query_delta_entities(&cursor, &plan, &mut changed, &mut reverse);
        assert!(changed.is_empty());
        assert_eq!(cursor.get(), 4);
    }

    #[test]
    fn mixed_mut_read_updates_only_a() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, A(2)).expect("A");
        world.insert(entity, B(5)).expect("B");
        let mut query = world
            .prepare_query2::<A, B>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("prepare");
        let before_b =
            world.component_changed_tick(entity, world.component_index::<B>().expect("B index"));
        query
            .for_each_mut_read(&mut world, QueryWindow::All, |_, a, b| {
                a.0 += b.0;
                Ok(())
            })
            .expect("execute");
        assert_eq!(world.get::<A>(entity).expect("get").expect("A"), &A(7));
        assert_eq!(world.get::<B>(entity).expect("get").expect("B"), &B(5));
        assert_eq!(
            world.component_changed_tick(entity, world.component_index::<B>().expect("B index"),),
            before_b
        );
    }

    #[test]
    fn query2_exact_ids_preserve_order_independent_of_driver() {
        let mut world = world();
        let first = world.spawn().expect("first");
        let second = world.spawn().expect("second");
        for (entity, value) in [(first, 1), (second, 2)] {
            world.insert(entity, A(value)).expect("A");
            world.insert(entity, B(value)).expect("B");
        }
        let spec = QuerySpec::new().exact_ids(
            alloc::vec![second, first],
            crate::query::ExactIdPolicy::SkipUnavailable,
        );
        let mut query = world
            .prepare_query2::<A, B>(spec, QueryPolicy::Prepared)
            .expect("prepare");
        let ids: Vec<_> = query
            .iter(&mut world, QueryWindow::All)
            .expect("iter")
            .map(|(entity, _, _)| entity)
            .collect();
        assert_eq!(ids, alloc::vec![second, first]);
    }

    #[test]
    fn query2_reselects_driver_after_cardinality_crossover_with_stable_cursors() {
        // sparse/sparse
        {
            let mut world = world();
            let first = world.spawn().expect("first");
            for entity in [
                first,
                world.spawn().expect("a2"),
                world.spawn().expect("a3"),
            ] {
                world.insert(entity, A(1)).expect("A");
            }
            world.insert(first, C(1)).expect("C");
            let spec = QuerySpec::new();
            let mut query = world
                .prepare_query2::<A, C>(spec.clone(), QueryPolicy::Prepared)
                .expect("prepare sparse/sparse");
            let c_index = world.component_index::<C>().expect("C index");
            assert!(matches!(
                query.plan.traversal,
                TraversalSource::Sparse { component_index } if component_index == c_index
            ));
            let mut before =
                QueryCursor::from_spec2_start::<A, C>(&mut world, &spec).expect("before cursor");
            for _ in 0..3 {
                let entity = world.spawn().expect("C-only");
                world.insert(entity, C(2)).expect("C-only insert");
            }
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::Cursor(&mut before))
                    .expect("sparse/sparse before cursor")
                    .count(),
                1
            );
            let a_index = world.component_index::<A>().expect("A index");
            assert!(matches!(
                query.plan.traversal,
                TraversalSource::Sparse { component_index } if component_index == a_index
            ));
            let mut after =
                QueryCursor::from_spec2_start::<A, C>(&mut world, &spec).expect("after cursor");
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::Cursor(&mut after))
                    .expect("sparse/sparse after cursor")
                    .count(),
                1
            );
        }

        // table/table
        {
            let mut world = world();
            let first = world.spawn().expect("first");
            for entity in [
                first,
                world.spawn().expect("b2"),
                world.spawn().expect("b3"),
            ] {
                world.insert(entity, B(1)).expect("B");
            }
            world.insert(first, D(1)).expect("D");
            let spec = QuerySpec::new();
            let mut query = world
                .prepare_query2::<B, D>(spec.clone(), QueryPolicy::Prepared)
                .expect("prepare table/table");
            let d_index = world.component_index::<D>().expect("D index");
            assert!(matches!(
                query.plan.traversal,
                TraversalSource::Table { component_index } if component_index == d_index
            ));
            let mut before =
                QueryCursor::from_spec2_start::<B, D>(&mut world, &spec).expect("before cursor");
            for _ in 0..3 {
                let entity = world.spawn().expect("D-only");
                world.insert(entity, D(2)).expect("D-only insert");
            }
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::Cursor(&mut before))
                    .expect("table/table before cursor")
                    .count(),
                1
            );
            let b_index = world.component_index::<B>().expect("B index");
            assert!(matches!(
                query.plan.traversal,
                TraversalSource::Table { component_index } if component_index == b_index
            ));
            let mut after =
                QueryCursor::from_spec2_start::<B, D>(&mut world, &spec).expect("after cursor");
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::Cursor(&mut after))
                    .expect("table/table after cursor")
                    .count(),
                1
            );
        }

        // sparse/table
        {
            let mut world = world();
            let first = world.spawn().expect("first");
            for entity in [
                first,
                world.spawn().expect("a2"),
                world.spawn().expect("a3"),
            ] {
                world.insert(entity, A(1)).expect("A");
            }
            world.insert(first, B(1)).expect("B");
            let spec = QuerySpec::new();
            let mut query = world
                .prepare_query2::<A, B>(spec.clone(), QueryPolicy::Prepared)
                .expect("prepare sparse/table");
            let b_index = world.component_index::<B>().expect("B index");
            assert!(matches!(
                query.plan.traversal,
                TraversalSource::Table { component_index } if component_index == b_index
            ));
            let mut before =
                QueryCursor::from_spec2_start::<A, B>(&mut world, &spec).expect("before cursor");
            for _ in 0..3 {
                let entity = world.spawn().expect("B-only");
                world.insert(entity, B(2)).expect("B-only insert");
            }
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::Cursor(&mut before))
                    .expect("sparse/table before cursor")
                    .count(),
                1
            );
            let a_index = world.component_index::<A>().expect("A index");
            assert!(matches!(
                query.plan.traversal,
                TraversalSource::Sparse { component_index } if component_index == a_index
            ));
            let mut after =
                QueryCursor::from_spec2_start::<A, B>(&mut world, &spec).expect("after cursor");
            assert_eq!(
                query
                    .iter(&mut world, QueryWindow::Cursor(&mut after))
                    .expect("sparse/table after cursor")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn mixed_mut_read_covers_all_storage_pairs() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, A(1)).expect("A");
        world.insert(entity, B(2)).expect("B");
        world.insert(entity, C(3)).expect("C");
        world.insert(entity, D(4)).expect("D");

        let mut sparse_sparse = world
            .prepare_query2::<A, C>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("sparse/sparse");
        sparse_sparse
            .for_each_mut_read(&mut world, QueryWindow::All, |_, a, c| {
                a.0 += c.0;
                Ok(())
            })
            .expect("sparse/sparse execute");

        let mut table_sparse = world
            .prepare_query2::<D, C>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("table/sparse");
        table_sparse
            .for_each_mut_read(&mut world, QueryWindow::All, |_, d, c| {
                d.0 += c.0;
                Ok(())
            })
            .expect("table/sparse execute");

        let mut table_table = world
            .prepare_query2::<D, B>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("table/table");
        table_table
            .for_each_mut_read(&mut world, QueryWindow::All, |_, d, b| {
                d.0 += b.0;
                Ok(())
            })
            .expect("table/table execute");

        assert_eq!(world.get::<A>(entity).expect("get").expect("A"), &A(4));
        assert_eq!(world.get::<B>(entity).expect("get").expect("B"), &B(2));
        assert_eq!(world.get::<C>(entity).expect("get").expect("C"), &C(3));
        assert_eq!(world.get::<D>(entity).expect("get").expect("D"), &D(9));
    }

    #[test]
    fn cursor_commits_only_after_full_iteration() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, A(1)).expect("A");
        let spec = QuerySpec::new().changed::<A>();
        let mut query = world
            .prepare_query1::<A>(spec.clone(), QueryPolicy::Prepared)
            .expect("prepare");
        let mut cursor = QueryCursor::from_spec_start::<A>(&mut world, &spec).expect("cursor");
        let before = cursor.since();
        {
            let mut iter = query
                .iter(&mut world, QueryWindow::Cursor(&mut cursor))
                .expect("iter");
            assert!(iter.next().is_some());
        }
        assert_eq!(cursor.since(), before);

        query
            .iter(&mut world, QueryWindow::Cursor(&mut cursor))
            .expect("iter")
            .for_each(drop);
        assert!(cursor.since() > before);
    }

    #[test]
    fn result_policy_rejects_moving_windows() {
        let mut world = world();
        assert!(matches!(
            world.prepare_query1::<A>(QuerySpec::new().changed::<A>(), QueryPolicy::Result,),
            Err(QueryError::MovingChangeWindow)
        ));
    }

    #[test]
    fn query2_cursor_matches_prepared_fingerprint_and_commits() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, A(1)).expect("A");
        world.insert(entity, B(2)).expect("B");
        let spec = QuerySpec::new().changed::<A>();
        let mut query = world
            .prepare_query2::<A, B>(spec.clone(), QueryPolicy::Prepared)
            .expect("prepare");
        let mut cursor = QueryCursor::from_spec2_start::<A, B>(&mut world, &spec).expect("cursor");
        let before = cursor.since();
        assert_eq!(
            query
                .iter(&mut world, QueryWindow::Cursor(&mut cursor))
                .expect("iter")
                .count(),
            1
        );
        assert!(cursor.since() > before);

        let mut wrong = QueryCursor::from_spec_start::<A>(&mut world, &spec).expect("Q1 cursor");
        assert!(matches!(
            query.iter(&mut world, QueryWindow::Cursor(&mut wrong)),
            Err(QueryError::WrongQuery { .. })
        ));
    }
}
