use moirai::component::ComponentOptions;
use moirai::world::{DynamicBundle, WorldBuilder};

#[derive(Clone, Copy)]
struct TableA(i32);

#[derive(Clone, Copy)]
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

