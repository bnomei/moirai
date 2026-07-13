use moirai::component::ComponentOptions;
use moirai::world::{DynamicBundle, WorldBuilder, WorldError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct TableA(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct TableB(i32);

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
