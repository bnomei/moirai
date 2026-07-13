use moirai::component::ComponentOptions;
use moirai::world::{DynamicBundle, WorldBuilder, WorldError};
use moirai::{EntityScratch, EntityScratchError, State, StateError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct TableA(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct TableB(i32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Menu,
    Playing,
    Paused,
}

#[test]
fn entity_scratch_round_trips_live_values_and_reports_absence() {
    let mut world = WorldBuilder::new().build().expect("build");
    let entity = world.spawn().expect("spawn");
    let missing = world.spawn().expect("spawn missing");
    let mut scratch = EntityScratch::new(&world);

    assert!(scratch.is_empty());
    assert_eq!(scratch.get(&world, entity).expect("live missing"), None);
    assert_eq!(scratch.insert(&world, entity, 10).expect("insert"), None);
    assert_eq!(scratch.len(), 1);
    assert_eq!(scratch.get(&world, entity).expect("get"), Some(&10));
    *scratch
        .get_mut(&world, entity)
        .expect("get mut")
        .expect("present") = 12;
    assert_eq!(
        scratch.insert(&world, entity, 20).expect("replace"),
        Some(12)
    );
    assert_eq!(scratch.get(&world, missing).expect("other live"), None);
    assert_eq!(scratch.remove(&world, entity).expect("remove"), Some(20));
    assert!(scratch.is_empty());
}

#[test]
fn entity_scratch_rejects_wrong_world_before_entity_validation() {
    let mut world_a = WorldBuilder::new().build().expect("world a");
    let world_b = WorldBuilder::new().build().expect("world b");
    let entity = world_a.spawn().expect("spawn");
    let mut scratch = EntityScratch::new(&world_a);
    scratch.insert(&world_a, entity, 1).expect("insert");

    assert_eq!(
        scratch.get(&world_b, entity),
        Err(EntityScratchError::WrongWorld)
    );
    assert_eq!(
        scratch.retain_live(&world_b),
        Err(EntityScratchError::WrongWorld)
    );
    assert_eq!(
        scratch.get(&world_a, entity).expect("still present"),
        Some(&1)
    );
}

#[test]
fn entity_scratch_rejects_reserved_stale_and_reused_generations() {
    let mut world = WorldBuilder::new().build().expect("build");
    let reserved = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    let mut scratch = EntityScratch::new(&world);
    assert_eq!(
        scratch.insert(&world, reserved, 1),
        Err(EntityScratchError::EntityNotLive { entity: reserved })
    );
    world.flush().expect("make reservation live");
    scratch.insert(&world, reserved, 2).expect("insert live");
    world.despawn(reserved).expect("despawn");

    assert_eq!(
        scratch.get(&world, reserved),
        Err(EntityScratchError::StaleEntity { entity: reserved })
    );
    assert_eq!(
        scratch.get_mut(&world, reserved),
        Err(EntityScratchError::StaleEntity { entity: reserved })
    );
    assert_eq!(
        scratch.remove(&world, reserved),
        Err(EntityScratchError::StaleEntity { entity: reserved })
    );

    let replacement = world.spawn().expect("reuse slot");
    assert_ne!(reserved, replacement);
    assert_eq!(
        scratch.get(&world, replacement).expect("new generation"),
        None
    );
    assert_eq!(
        scratch.len(),
        1,
        "stale value remains until explicit cleanup"
    );
}

#[test]
fn entity_scratch_retain_live_removes_stale_and_preserves_live_values() {
    let mut world = WorldBuilder::new().build().expect("build");
    let stale = world.spawn().expect("stale");
    let live = world.spawn().expect("live");
    let mut scratch = EntityScratch::new(&world);
    scratch.insert(&world, stale, 1).expect("insert stale");
    scratch.insert(&world, live, 2).expect("insert live");
    world.despawn(stale).expect("despawn");

    assert_eq!(scratch.retain_live(&world).expect("retain"), 1);
    assert_eq!(scratch.len(), 1);
    assert_eq!(scratch.get(&world, live).expect("live remains"), Some(&2));
}

#[test]
fn entity_scratch_values_drop_exactly_once_across_all_ownership_paths() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct DropValue(Rc<Cell<usize>>);

    impl Drop for DropValue {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let mut world = WorldBuilder::new().build().expect("build");
    let entity = world.spawn().expect("spawn");
    let drops = Rc::new(Cell::new(0));
    let mut scratch = EntityScratch::new(&world);

    scratch
        .insert(&world, entity, DropValue(drops.clone()))
        .expect("insert");
    let replaced = scratch
        .insert(&world, entity, DropValue(drops.clone()))
        .expect("replace")
        .expect("old value");
    assert_eq!(drops.get(), 0);
    drop(replaced);
    assert_eq!(drops.get(), 1);

    let removed = scratch
        .remove(&world, entity)
        .expect("remove")
        .expect("stored value");
    assert_eq!(drops.get(), 1);
    drop(removed);
    assert_eq!(drops.get(), 2);

    scratch
        .insert(&world, entity, DropValue(drops.clone()))
        .expect("insert for clear");
    scratch.clear();
    assert_eq!(drops.get(), 3);

    scratch
        .insert(&world, entity, DropValue(drops.clone()))
        .expect("insert for scratch drop");
    drop(scratch);
    assert_eq!(drops.get(), 4);
}

#[test]
fn state_builder_seed_is_last_call_wins() {
    let mut builder = WorldBuilder::new();
    builder.insert_state(Phase::Menu);
    builder.insert_state(Phase::Playing);
    let world = builder.build().expect("build");
    let state = world
        .resource::<State<Phase>>()
        .expect("state resource")
        .expect("seeded state");

    assert_eq!(state.current(), &Phase::Playing);
    assert_eq!(
        world
            .resource_added_tick::<State<Phase>>()
            .expect("added tick"),
        Some(moirai::ChangeTick::from_raw(1))
    );
}

#[test]
fn state_transition_request_truth_table_is_idempotent_and_conflict_specific() {
    let mut state = State::new(Phase::Menu);

    state.request(Phase::Menu).expect("current is a no-op");
    assert_eq!(state.pending(), None);

    state.request(Phase::Playing).expect("queue transition");
    state
        .request(Phase::Playing)
        .expect("duplicate pending is a no-op");
    assert_eq!(state.pending(), Some(&Phase::Playing));

    assert_eq!(
        state.request(Phase::Paused),
        Err(StateError::ConflictingTransition)
    );
    assert_eq!(
        state.request(Phase::Menu),
        Err(StateError::ConflictingTransition)
    );
    assert_eq!(state.pending(), Some(&Phase::Playing));
}

#[test]
fn archetype_move_preserves_retained_component() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    builder
        .register_component::<TableB>(ComponentOptions::table())
        .expect("b");
    let mut world = builder.build().expect("build");

    let entity = world.spawn().expect("spawn");
    world.insert(entity, TableA(1)).expect("insert a");
    world.insert(entity, TableB(2)).expect("insert b");
    assert_eq!(
        world.get::<TableA>(entity).expect("get a").map(|v| v.0),
        Some(1)
    );
    assert_eq!(
        world.get::<TableB>(entity).expect("get b").map(|v| v.0),
        Some(2)
    );

    let removed = world.remove::<TableA>(entity).expect("remove a");
    assert_eq!(removed.map(|v| v.0), Some(1));
    assert_eq!(
        world.get::<TableB>(entity).expect("get b").map(|v| v.0),
        Some(2)
    );
}

#[derive(Clone, Copy)]
struct Player;

#[test]
fn tag_add_remove_and_has_round_trip() {
    let mut builder = WorldBuilder::new();
    let tag = builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("tag");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    assert!(world.add_tag(entity, &tag).expect("add"));
    assert!(world.has_tag(entity, &tag).expect("has"));
    assert!(world.remove_tag(entity, &tag).expect("remove"));
    assert!(!world.has_tag(entity, &tag).expect("has again"));
}

#[test]
fn tag_get_mut_is_rejected() {
    let mut builder = WorldBuilder::new();
    let tag = builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("tag");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.add_tag(entity, &tag).expect("add");
    assert!(matches!(
        world.get_mut::<Player>(entity),
        Err(WorldError::WrongStorageKind { .. })
    ));
}

#[test]
fn table_get_mut_updates_value() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TableA(4)).expect("insert");
    world
        .get_mut::<TableA>(entity)
        .expect("mut")
        .expect("present")
        .0 = 9;
    assert_eq!(
        world.get::<TableA>(entity).expect("get").map(|v| v.0),
        Some(9)
    );
}

#[test]
fn dynamic_bundle_push_tag_and_reject_value_on_tag() {
    let mut builder = WorldBuilder::new();
    let tag = builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("tag");
    let mut world = builder.build().expect("build");
    let mut bundle = DynamicBundle::new();
    bundle.push_tag(&tag).expect("push tag");
    assert!(matches!(
        bundle.push(&world, Player),
        Err(WorldError::WrongStorageKind { .. })
    ));
    let entity = world.spawn_bundle(bundle).expect("spawn");
    assert!(world.has_tag(entity, &tag).expect("has"));
}

#[test]
fn dynamic_bundle_rejects_duplicate_components() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    let world = builder.build().expect("build");

    let mut bundle = DynamicBundle::new();
    bundle.push(&world, TableA(1)).expect("first");
    assert!(bundle.push(&world, TableA(2)).is_err());
}

#[test]
fn table_insert_replaces_existing_returns_prior_value() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    assert_eq!(world.insert(entity, TableA(10)).expect("first"), None);
    assert_eq!(
        world
            .insert(entity, TableA(99))
            .expect("replace")
            .map(|v| v.0),
        Some(10)
    );
    assert_eq!(
        world.get::<TableA>(entity).expect("get").map(|v| v.0),
        Some(99)
    );
}

#[test]
fn table_add_second_component_migrates_archetype_preserving_first() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    builder
        .register_component::<TableB>(ComponentOptions::table())
        .expect("b");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    world.insert(entity, TableA(7)).expect("add a");
    world.insert(entity, TableB(3)).expect("add b");

    assert_eq!(
        world.get::<TableA>(entity).expect("get a").map(|v| v.0),
        Some(7)
    );
    assert_eq!(
        world.get::<TableB>(entity).expect("get b").map(|v| v.0),
        Some(3)
    );
}

#[test]
fn table_remove_absent_and_remove_last_clears_component() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    assert_eq!(world.remove::<TableA>(entity).expect("absent"), None);
    world.insert(entity, TableA(4)).expect("insert");
    assert_eq!(
        world.remove::<TableA>(entity).expect("remove").map(|v| v.0),
        Some(4)
    );
    assert!(world.get::<TableA>(entity).expect("gone").is_none());
    assert!(world.is_alive(entity));
}

#[test]
fn tag_api_rejects_non_tag_component() {
    let mut builder = WorldBuilder::new();
    let table = builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("table");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    assert!(matches!(
        world.add_tag(entity, &table),
        Err(WorldError::WrongStorageKind { .. })
    ));
    assert!(matches!(
        world.has_tag(entity, &table),
        Err(WorldError::WrongStorageKind { .. })
    ));
    assert!(matches!(
        world.remove_tag(entity, &table),
        Err(WorldError::WrongStorageKind { .. })
    ));
}

#[test]
fn get_on_tag_returns_none_not_error() {
    let mut builder = WorldBuilder::new();
    let tag = builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("tag");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.add_tag(entity, &tag).expect("add");
    assert!(world.get::<Player>(entity).expect("get").is_none());
    assert!(world.has_tag(entity, &tag).expect("has"));
}

#[test]
fn table_remove_one_component_repairs_sibling_entity_row() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TableA>(ComponentOptions::table())
        .expect("a");
    builder
        .register_component::<TableB>(ComponentOptions::table())
        .expect("b");
    let mut world = builder.build().expect("build");

    let e1 = world.spawn().expect("e1");
    let e2 = world.spawn().expect("e2");
    world.insert(e1, TableA(1)).expect("e1 a");
    world.insert(e1, TableB(10)).expect("e1 b");
    world.insert(e2, TableA(2)).expect("e2 a");
    world.insert(e2, TableB(20)).expect("e2 b");

    world.remove::<TableB>(e1).expect("remove e1 b");

    assert!(world.get::<TableB>(e1).expect("e1 b gone").is_none());
    assert_eq!(world.get::<TableA>(e1).expect("e1 a").map(|v| v.0), Some(1));
    assert_eq!(world.get::<TableA>(e2).expect("e2 a").map(|v| v.0), Some(2));
    assert_eq!(
        world.get::<TableB>(e2).expect("e2 b").map(|v| v.0),
        Some(20)
    );
}
