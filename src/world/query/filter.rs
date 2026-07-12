use crate::entity::EntityId;
use crate::query::{ExactIdPolicy, QueryError};
use crate::time::ChangeTick;
use crate::world::World;

use super::plan::{ResolvedPlan, TraversalSource};

pub(crate) fn entity_matches_structural(
    world: &World,
    entity: EntityId,
    plan: &ResolvedPlan,
) -> bool {
    if !world.is_alive(entity) {
        return false;
    }
    if world.allocator_is_reserved(entity) {
        return false;
    }
    for &index in &plan.required_indices {
        if !world.entity_has_component(entity, index) {
            return false;
        }
    }
    for &index in &plan.without_indices {
        if world.entity_has_component(entity, index) {
            return false;
        }
    }
    for &index in &plan.with_tag_indices {
        if !world.entity_has_tag(entity, index) {
            return false;
        }
    }
    for &index in &plan.without_tag_indices {
        if world.entity_has_tag(entity, index) {
            return false;
        }
    }
    true
}

pub(crate) fn entity_matches(
    world: &World,
    entity: EntityId,
    plan: &ResolvedPlan,
    since: ChangeTick,
    captured_now: ChangeTick,
) -> bool {
    if !entity_matches_structural(world, entity, plan) {
        return false;
    }
    if let Some(index) = plan.added_index {
        if !tick_in_window(
            world.component_added_tick(entity, index),
            since,
            captured_now,
        ) {
            return false;
        }
    }
    if let Some(index) = plan.changed_index {
        if !tick_in_window(
            world.component_changed_tick(entity, index),
            since,
            captured_now,
        ) {
            return false;
        }
    }
    true
}

pub(crate) fn validate_exact_ids(world: &World, plan: &ResolvedPlan) -> Result<(), QueryError> {
    if plan.exact_id_policy != Some(ExactIdPolicy::ErrorOnUnavailable) {
        return Ok(());
    }
    let TraversalSource::Exact { ids } = &plan.traversal else {
        return Ok(());
    };
    for &entity in ids {
        if !world.is_alive(entity)
            || world.allocator_is_reserved(entity)
            || !entity_matches_structural(world, entity, plan)
        {
            return Err(QueryError::MissingExactId { entity });
        }
    }
    Ok(())
}

fn tick_in_window(tick: Option<ChangeTick>, since: ChangeTick, captured_now: ChangeTick) -> bool {
    let Some(tick) = tick else {
        return false;
    };
    tick > since && tick <= captured_now
}
