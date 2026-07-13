use moirai::component::ComponentOptions;
use moirai::world::{WorldBuilder, WorldError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Health(i32);

#[test]
fn deferred_spawn_is_not_alive_until_flush() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");

    let entity = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    assert!(!world.is_alive(entity));
    assert!(matches!(
        world.get::<Health>(entity),
        Err(WorldError::EntityNotLive { .. })
    ));

    world
        .commands()
        .expect("commands")
        .insert(entity, Health(3))
        .expect("queue");
    let report = world.flush().expect("flush");
    assert_eq!(report.commands_applied, 2);
    assert!(world.is_alive(entity));
    assert_eq!(
        world.get::<Health>(entity).expect("get").map(|h| h.0),
        Some(3)
    );
}

#[test]
fn failed_flush_releases_reservations() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let live = world.spawn().expect("live");

    let entity = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    world
        .commands()
        .expect("commands")
        .despawn(live)
        .expect("queue first despawn");
    world
        .commands()
        .expect("commands")
        .despawn(live)
        .expect("queue duplicate despawn");
    assert!(matches!(world.flush(), Err(WorldError::Flush(_))));
    assert!(!world.is_alive(entity));
    assert!(!world.has_pending_commands());
}

#[test]
fn discard_releases_reserved_entities() {
    let mut world = WorldBuilder::new().build().expect("build");
    let entity = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    world.discard_commands().expect("discard");
    assert!(!world.is_alive(entity));
    assert!(!world.has_pending_commands());
}

#[derive(Clone, Copy)]
struct Marker;

#[test]
fn deferred_tag_insert_via_commands() {
    use moirai::world::DynamicBundle;

    let mut builder = WorldBuilder::new();
    let tag = builder
        .register_component::<Marker>(ComponentOptions::tag())
        .expect("tag");
    let mut world = builder.build().expect("build");
    let mut bundle = DynamicBundle::new();
    bundle.push_tag(&tag).expect("push");
    let entity = world
        .commands()
        .expect("commands")
        .spawn_bundle(bundle)
        .expect("spawn");
    world.flush().expect("flush");
    assert!(world.has_tag(entity, &tag).expect("has"));
}

#[test]
fn deferred_remove_component_via_commands() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Health(1)).expect("insert");

    world
        .commands()
        .expect("commands")
        .remove::<Health>(entity)
        .expect("queue");
    world.flush().expect("flush");
    assert!(world.get::<Health>(entity).expect("get").is_none());
}
