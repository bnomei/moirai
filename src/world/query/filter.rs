use crate::entity::EntityId;
use crate::query::{ExactIdPolicy, QueryError};
use crate::time::ChangeTick;
use crate::world::World;

use super::plan::{ResolvedPlan, TraversalSource};

pub(crate) fn validate_exact_id_duplicates(
    world: &World,
    ids: &[EntityId],
) -> Result<(), QueryError> {
    for (index, &entity) in ids.iter().enumerate() {
        if !world.owns_entity(entity)
            || !world.is_alive(entity)
            || world.allocator_is_reserved(entity)
        {
            continue;
        }
        if ids[..index].contains(&entity) {
            return Err(QueryError::DuplicateExactId { entity });
        }
    }
    Ok(())
}

pub(crate) fn entity_matches_structural(
    world: &World,
    entity: EntityId,
    plan: &ResolvedPlan,
) -> bool {
    if world.allocator_is_reserved(entity) {
        return false;
    }
    if !world.is_alive(entity) {
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

pub(crate) fn entity_matches_structural_with_covered(
    world: &World,
    entity: EntityId,
    plan: &ResolvedPlan,
    covered_required: &[usize],
) -> bool {
    if world.allocator_is_reserved(entity) {
        return false;
    }
    if !world.is_alive(entity) {
        return false;
    }
    for &index in &plan.required_indices {
        if !covered_required.contains(&index) && !world.entity_has_component(entity, index) {
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
    if !plan.added_indices.is_empty()
        && !plan.added_indices.iter().any(|&index| {
            tick_in_window(
                world.component_added_tick(entity, index),
                since,
                captured_now,
            )
        })
    {
        return false;
    }
    if !plan.changed_indices.is_empty()
        && !plan.changed_indices.iter().any(|&index| {
            tick_in_window(
                world.component_changed_tick(entity, index),
                since,
                captured_now,
            )
        })
    {
        return false;
    }
    true
}

pub(crate) fn entity_matches_with_covered(
    world: &World,
    entity: EntityId,
    plan: &ResolvedPlan,
    since: ChangeTick,
    captured_now: ChangeTick,
    covered_required: &[usize],
) -> bool {
    if !entity_matches_structural_with_covered(world, entity, plan, covered_required) {
        return false;
    }
    if !plan.added_indices.is_empty()
        && !plan.added_indices.iter().any(|&index| {
            tick_in_window(
                world.component_added_tick(entity, index),
                since,
                captured_now,
            )
        })
    {
        return false;
    }
    if !plan.changed_indices.is_empty()
        && !plan.changed_indices.iter().any(|&index| {
            tick_in_window(
                world.component_changed_tick(entity, index),
                since,
                captured_now,
            )
        })
    {
        return false;
    }
    true
}

pub(crate) fn validate_exact_ids(world: &World, plan: &ResolvedPlan) -> Result<(), QueryError> {
    let TraversalSource::Exact { ids } = &plan.traversal else {
        return Ok(());
    };
    if ids.iter().any(|&entity| !world.owns_entity(entity)) {
        return Err(QueryError::WrongOwner);
    }
    if plan.exact_id_policy == Some(ExactIdPolicy::ErrorOnUnavailable) {
        for &entity in ids {
            if !world.is_alive(entity)
                || world.allocator_is_reserved(entity)
                || !entity_matches_structural(world, entity, plan)
            {
                return Err(QueryError::MissingExactId { entity });
            }
        }
    }
    validate_exact_id_duplicates(world, ids)
}

fn tick_in_window(tick: Option<ChangeTick>, since: ChangeTick, captured_now: ChangeTick) -> bool {
    let Some(tick) = tick else {
        return false;
    };
    tick > since && tick <= captured_now
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::ExactIdPolicy;
    use crate::time::ChangeTick;
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Marker(u8);

    fn sparse_plan(primary: usize) -> ResolvedPlan {
        ResolvedPlan {
            fingerprint: 1,
            primary_index: primary,
            primary_is_table: false,
            traversal: TraversalSource::Sparse {
                component_index: primary,
            },
            required_indices: alloc::vec![primary],
            without_indices: alloc::vec![],
            with_tag_indices: alloc::vec![],
            without_tag_indices: alloc::vec![],
            added_indices: alloc::vec![],
            changed_indices: alloc::vec![],
            exact_id_policy: None,
        }
    }

    #[test]
    fn entity_matches_structural_rejects_reserved_entities() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("marker");
        let mut world = builder.build().expect("build");
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        let plan = sparse_plan(0);
        assert!(!entity_matches_structural(&world, reserved, &plan));
    }

    #[test]
    fn entity_matches_structural_rejects_stale_generations() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("marker");
        let mut world = builder.build().expect("build");
        let live = world.spawn().expect("live");
        let stale = live.with_generation(live.generation().wrapping_add(1));
        let plan = sparse_plan(0);
        assert!(!world.is_alive(stale));
        assert!(!entity_matches_structural(&world, stale, &plan));
    }

    #[test]
    fn entity_matches_structural_rejects_dead_and_reserved_entities() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("marker");
        let mut world = builder.build().expect("build");
        let live = world.spawn().expect("live");
        world.insert(live, Marker(1)).expect("insert");
        let plan = sparse_plan(0);

        world.despawn(live).expect("despawn");
        assert!(!entity_matches_structural(&world, live, &plan));

        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        assert!(!entity_matches_structural(&world, reserved, &plan));
    }

    #[test]
    fn entity_matches_rejects_changed_outside_window_and_missing_ticks() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("marker");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Marker(1)).expect("insert");
        world
            .get_mut::<Marker>(entity)
            .expect("mut")
            .expect("present")
            .0 = 2;

        let mut plan = sparse_plan(0);
        plan.changed_indices = alloc::vec![0];
        let since = ChangeTick::from_raw(10);
        let captured_now = ChangeTick::from_raw(20);
        assert!(!entity_matches(&world, entity, &plan, since, captured_now));

        plan.added_indices = alloc::vec![0];
        plan.changed_indices.clear();
        assert!(!entity_matches(
            &world,
            entity,
            &plan,
            ChangeTick::from_raw(100),
            ChangeTick::from_raw(200),
        ));
    }

    #[test]
    fn validate_exact_ids_ignores_non_exact_traversal_with_error_policy() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Marker>(ComponentOptions::sparse())
            .expect("marker");
        let world = builder.build().expect("build");
        let mut plan = sparse_plan(0);
        plan.exact_id_policy = Some(ExactIdPolicy::ErrorOnUnavailable);
        assert!(validate_exact_ids(&world, &plan).is_ok());
    }

    #[test]
    fn tick_in_window_rejects_missing_ticks() {
        assert!(!tick_in_window(
            None,
            ChangeTick::ZERO,
            ChangeTick::from_raw(1)
        ));
    }
}
