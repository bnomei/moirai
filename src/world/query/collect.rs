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
    match &plan.traversal {
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
            for archetype in world
                .archetypes
                .archetypes_with_component(*component_index as u32)
            {
                for &slot in world.archetype_entity_slots(archetype) {
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
    out
}

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
