//! Collect query entity ids from sparse, table, archetype, and exact-id sources.

use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::time::ChangeTick;
use crate::world::World;

use super::filter::{entity_matches, entity_matches_structural};
use super::plan::{ResolvedPlan, TraversalSource};

pub(crate) fn collect_query1_structural_members(
    world: &World,
    plan: &ResolvedPlan,
) -> Vec<EntityId> {
    let mut out = Vec::new();
    match &plan.traversal {
        TraversalSource::All => {
            world.collect_live_entities(&mut out);
            out.retain(|&entity| entity_matches_structural(world, entity, plan));
        }
        TraversalSource::Sparse { component_index } => {
            if let Some(slots) = world.sparse_dense_slots(*component_index) {
                for &slot in slots {
                    let entity = world.entity_from_slot(slot);
                    if entity_matches_structural(world, entity, plan) {
                        out.push(entity);
                    }
                }
            }
        }
        TraversalSource::Table { component_index } => {
            for archetype in world
                .archetypes
                .archetypes_with_component(*component_index as u32)
            {
                for &slot in world.archetype_entity_slots(archetype) {
                    let entity = world.entity_from_slot(slot);
                    if entity_matches_structural(world, entity, plan) {
                        out.push(entity);
                    }
                }
            }
        }
        TraversalSource::Exact { ids } => {
            for &entity in ids {
                if entity_matches_structural(world, entity, plan) {
                    out.push(entity);
                }
            }
        }
    }
    out
}

pub(crate) fn collect_query1_entities(
    world: &World,
    plan: &ResolvedPlan,
    since: ChangeTick,
    captured_now: ChangeTick,
) -> Vec<EntityId> {
    let mut out = Vec::new();
    collect_query1_entities_into(world, plan, since, captured_now, &mut out);
    out
}

pub(crate) fn collect_query1_entities_into(
    world: &World,
    plan: &ResolvedPlan,
    since: ChangeTick,
    captured_now: ChangeTick,
    out: &mut Vec<EntityId>,
) {
    out.clear();
    match &plan.traversal {
        TraversalSource::All => {
            world.collect_live_entities(out);
            out.retain(|&entity| entity_matches(world, entity, plan, since, captured_now));
        }
        TraversalSource::Sparse { component_index } => {
            if let Some(slots) = world.sparse_dense_slots(*component_index) {
                for &slot in slots {
                    let entity = world.entity_from_slot(slot);
                    if entity_matches(world, entity, plan, since, captured_now) {
                        out.push(entity);
                    }
                }
            }
        }
        TraversalSource::Table { component_index } => {
            for slots in world
                .archetypes
                .entity_slot_slices_with_component(*component_index as u32)
            {
                for &slot in slots {
                    let entity = world.entity_from_slot(slot);
                    if entity_matches(world, entity, plan, since, captured_now) {
                        out.push(entity);
                    }
                }
            }
        }
        TraversalSource::Exact { ids } => {
            for &entity in ids {
                if entity_matches(world, entity, plan, since, captured_now) {
                    out.push(entity);
                }
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn collect_query2_entities(
    world: &World,
    plan: &ResolvedPlan,
    since: ChangeTick,
    captured_now: ChangeTick,
    second_index: usize,
    second_is_table: bool,
) -> Vec<EntityId> {
    collect_query1_entities(world, plan, since, captured_now)
        .into_iter()
        .filter(|&entity| world.entity_has_query2_second(entity, second_index, second_is_table))
        .collect()
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{ExactIdPolicy, QuerySpec};
    use crate::world::query::plan::{ResolvedPlan, TraversalSource};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Sparse(#[allow(dead_code)] i32);

    #[derive(Clone, Copy)]
    struct Table(#[allow(dead_code)] i32);

    #[derive(Clone, Copy)]
    struct Tag;

    #[test]
    fn structural_sparse_collects_matching_entities() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Sparse(1)).expect("insert");
        let plan = world
            .resolve_query1_plan::<Sparse>(&QuerySpec::new())
            .expect("plan");
        let members = collect_query1_structural_members(&world, &plan);
        assert_eq!(members, vec![entity]);
    }

    #[test]
    fn structural_sparse_with_no_store_returns_empty() {
        let world = WorldBuilder::new().build().expect("build");
        let plan = ResolvedPlan {
            fingerprint: 0,
            primary_index: 99,
            primary_is_table: false,
            traversal: TraversalSource::Sparse {
                component_index: 99,
            },
            required_indices: Vec::new(),
            without_indices: Vec::new(),
            with_tag_indices: Vec::new(),
            without_tag_indices: Vec::new(),
            added_indices: Vec::new(),
            changed_indices: Vec::new(),
            exact_id_policy: None,
        };
        assert!(collect_query1_structural_members(&world, &plan).is_empty());
    }

    #[test]
    fn structural_table_collects_archetype_rows() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Table>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Table(2)).expect("insert");
        let plan = world
            .resolve_query1_plan::<Table>(&QuerySpec::new())
            .expect("plan");
        let members = collect_query1_structural_members(&world, &plan);
        assert_eq!(members, vec![entity]);
    }

    #[test]
    fn structural_exact_ids_preserves_requested_order() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        let mut world = builder.build().expect("build");
        let first = world.spawn().expect("first");
        let second = world.spawn().expect("second");
        world.insert(first, Sparse(1)).expect("first");
        world.insert(second, Sparse(2)).expect("second");
        let spec = QuerySpec::new().exact_ids(vec![second, first], ExactIdPolicy::SkipUnavailable);
        let plan = world.resolve_query1_plan::<Sparse>(&spec).expect("plan");
        let members = collect_query1_structural_members(&world, &plan);
        assert_eq!(members, vec![second, first]);
    }

    #[test]
    fn tick_filtered_table_and_exact_collect_matching_entities() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Table>(ComponentOptions::table())
            .expect("table");
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        let mut world = builder.build().expect("build");
        let table_entity = world.spawn().expect("table");
        let sparse_entity = world.spawn().expect("sparse");
        world.insert(table_entity, Table(3)).expect("table");
        world.insert(sparse_entity, Sparse(4)).expect("sparse");
        let since = world.change_tick();
        let now = world.change_tick();

        let table_plan = world
            .resolve_query1_plan::<Table>(&QuerySpec::new())
            .expect("table plan");
        assert_eq!(
            collect_query1_entities(&world, &table_plan, since, now),
            vec![table_entity]
        );

        let exact_spec =
            QuerySpec::new().exact_ids(vec![sparse_entity], ExactIdPolicy::SkipUnavailable);
        let exact_plan = world
            .resolve_query1_plan::<Sparse>(&exact_spec)
            .expect("exact plan");
        assert_eq!(
            collect_query1_entities(&world, &exact_plan, since, now),
            vec![sparse_entity]
        );
    }

    #[test]
    fn tick_filtered_sparse_with_missing_store_returns_empty() {
        let world = WorldBuilder::new().build().expect("build");
        let plan = ResolvedPlan {
            fingerprint: 1,
            primary_index: 0,
            primary_is_table: false,
            traversal: TraversalSource::Sparse { component_index: 0 },
            required_indices: Vec::new(),
            without_indices: Vec::new(),
            with_tag_indices: Vec::new(),
            without_tag_indices: Vec::new(),
            added_indices: Vec::new(),
            changed_indices: Vec::new(),
            exact_id_policy: None,
        };
        assert!(
            collect_query1_entities(&world, &plan, ChangeTick::ZERO, ChangeTick::ZERO,).is_empty()
        );
    }

    #[test]
    fn tick_filtered_sparse_with_empty_slots_returns_empty() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        let mut world = builder.build().expect("build");
        let plan = world
            .resolve_query1_plan::<Sparse>(&QuerySpec::new())
            .expect("plan");
        let since = world.change_tick();
        let now = since;
        assert!(collect_query1_entities(&world, &plan, since, now).is_empty());
    }

    #[test]
    fn entity_all_traversal_collects_only_structural_matches() {
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Tag>(ComponentOptions::tag())
            .expect("tag");
        let mut world = builder.build().expect("build");
        let untagged = world.spawn().expect("untagged");
        let tagged = world.spawn().expect("tagged");
        world.add_tag(tagged, &tag).expect("tag");
        let plan = world
            .resolve_entity_plan(&QuerySpec::new().with_tag::<Tag>())
            .expect("plan");

        assert_eq!(
            collect_query1_structural_members(&world, &plan),
            vec![tagged]
        );
        assert_eq!(
            collect_query1_entities(&world, &plan, ChangeTick::ZERO, world.change_tick(),),
            vec![tagged]
        );
        assert_ne!(untagged, tagged);
    }

    #[test]
    fn query2_collection_filters_entities_missing_second_component() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        builder
            .register_component::<Table>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("build");
        let both = world.spawn().expect("both");
        let primary_only = world.spawn().expect("primary");
        world.insert(both, Sparse(1)).expect("sparse");
        world.insert(both, Table(2)).expect("table");
        world.insert(primary_only, Sparse(3)).expect("primary");
        let (plan, second_index, second_is_table) = world
            .resolve_query2_plan::<Sparse, Table>(&QuerySpec::new())
            .expect("plan");

        assert_eq!(
            collect_query2_entities(
                &world,
                &plan,
                ChangeTick::ZERO,
                world.change_tick(),
                second_index,
                second_is_table,
            ),
            vec![both]
        );
    }

    #[test]
    fn every_collection_source_skips_nonmatching_candidates() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Sparse>(ComponentOptions::sparse())
            .expect("sparse");
        builder
            .register_component::<Table>(ComponentOptions::table())
            .expect("table");
        let mut world = builder.build().expect("build");
        let both = world.spawn().expect("both");
        let sparse_only = world.spawn().expect("sparse only");
        let table_only = world.spawn().expect("table only");
        world.insert(both, Sparse(1)).expect("both sparse");
        world.insert(both, Table(2)).expect("both table");
        world
            .insert(sparse_only, Sparse(3))
            .expect("sparse only component");
        world
            .insert(table_only, Table(4))
            .expect("table only component");

        let all = world
            .resolve_entity_plan(&QuerySpec::new())
            .expect("all plan");
        assert_eq!(
            collect_query1_structural_members(&world, &all),
            alloc::vec![both, sparse_only, table_only]
        );

        let table_index = world.component_index::<Table>().expect("table index");
        let mut table_plan = (*world
            .resolve_query1_plan::<Table>(&QuerySpec::new().with::<Sparse>())
            .expect("table plan"))
        .clone();
        table_plan.traversal = TraversalSource::Table {
            component_index: table_index,
        };
        assert_eq!(
            collect_query1_structural_members(&world, &table_plan),
            alloc::vec![both]
        );

        let exact_spec = QuerySpec::new().exact_ids(
            alloc::vec![sparse_only, both],
            ExactIdPolicy::SkipUnavailable,
        );
        let exact_plan = world
            .resolve_query1_plan::<Table>(&exact_spec)
            .expect("exact plan");
        assert_eq!(
            collect_query1_structural_members(&world, &exact_plan),
            alloc::vec![both]
        );

        let sparse_index = world.component_index::<Sparse>().expect("sparse index");
        let mut sparse_plan = (*world
            .resolve_query1_plan::<Sparse>(&QuerySpec::new().with::<Table>())
            .expect("sparse plan"))
        .clone();
        sparse_plan.traversal = TraversalSource::Sparse {
            component_index: sparse_index,
        };
        assert_eq!(
            collect_query1_entities(&world, &sparse_plan, ChangeTick::ZERO, world.change_tick(),),
            alloc::vec![both]
        );
        assert_eq!(
            collect_query1_entities(&world, &table_plan, ChangeTick::ZERO, world.change_tick(),),
            alloc::vec![both]
        );
        assert_eq!(
            collect_query1_entities(&world, &exact_plan, ChangeTick::ZERO, world.change_tick(),),
            alloc::vec![both]
        );
    }
}
