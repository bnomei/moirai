use moirai::component::ComponentOptions;
use moirai::world::{Bundle, BundleWriter, DynamicBundle, WorldBuilder, WorldError};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Health(i32);

struct OwnedHealth {
    value: i32,
    drops: Rc<Cell<usize>>,
}

impl Drop for OwnedHealth {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

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

#[test]
fn deferred_insert_moves_a_non_clone_value_and_drops_it_once() {
    let drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<OwnedHealth>(ComponentOptions::table())
        .expect("register");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    world
        .commands()
        .expect("commands")
        .insert(
            entity,
            OwnedHealth {
                value: 7,
                drops: Rc::clone(&drops),
            },
        )
        .expect("queue");
    assert_eq!(drops.get(), 0);
    world.flush().expect("flush");
    assert_eq!(
        world
            .get::<OwnedHealth>(entity)
            .expect("get")
            .expect("present")
            .value,
        7
    );
    assert_eq!(drops.get(), 0);

    world
        .commands()
        .expect("commands")
        .remove::<OwnedHealth>(entity)
        .expect("queue remove");
    world.flush().expect("flush remove");
    assert_eq!(drops.get(), 1);
}

#[test]
fn deferred_non_clone_values_drop_once_on_discard_and_failed_preflight() {
    let drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<OwnedHealth>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let live = world.spawn().expect("live");

    world
        .commands()
        .expect("commands")
        .insert(
            live,
            OwnedHealth {
                value: 1,
                drops: Rc::clone(&drops),
            },
        )
        .expect("queue");
    world.discard_commands().expect("discard");
    assert_eq!(drops.get(), 1);

    world
        .commands()
        .expect("commands")
        .insert(
            live,
            OwnedHealth {
                value: 2,
                drops: Rc::clone(&drops),
            },
        )
        .expect("queue");
    world
        .commands()
        .expect("commands")
        .despawn(live)
        .expect("first despawn");
    world
        .commands()
        .expect("commands")
        .despawn(live)
        .expect("duplicate despawn");
    assert!(matches!(world.flush(), Err(WorldError::Flush(_))));
    assert_eq!(drops.get(), 2);
    assert!(!world.has_pending_commands());
}

#[test]
fn deferred_bundle_inserts_into_an_existing_entity() {
    let drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("health");
    builder
        .register_component::<OwnedHealth>(ComponentOptions::table())
        .expect("owned");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    world
        .commands()
        .expect("commands")
        .insert_bundle(
            entity,
            (
                Health(9),
                OwnedHealth {
                    value: 11,
                    drops: Rc::clone(&drops),
                },
            ),
        )
        .expect("bundle");
    assert!(world.get::<Health>(entity).expect("health").is_none());

    let report = world.flush().expect("flush");
    assert_eq!(report.commands_applied, 2);
    assert_eq!(
        world.get::<Health>(entity).expect("health"),
        Some(&Health(9))
    );
    assert_eq!(
        world
            .get::<OwnedHealth>(entity)
            .expect("owned")
            .expect("present")
            .value,
        11
    );
    assert_eq!(drops.get(), 0);
}

#[test]
fn deferred_dynamic_bundle_inserts_into_an_existing_entity() {
    let mut builder = WorldBuilder::new();
    let tag = builder.register_tag("selected").expect("tag");
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("health");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    let mut bundle = DynamicBundle::new();
    bundle.push(&world, Health(4)).expect("health");
    bundle.push_tag(&tag).expect("tag");

    world
        .commands()
        .expect("commands")
        .insert_bundle(entity, bundle)
        .expect("bundle");
    world.flush().expect("flush");

    assert_eq!(
        world.get::<Health>(entity).expect("health"),
        Some(&Health(4))
    );
    assert!(world.has_tag(entity, &tag).expect("tag"));
}

struct RejectedExistingBundle {
    drops: Rc<Cell<usize>>,
}

impl Bundle for RejectedExistingBundle {
    fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError> {
        writer.insert(DropTrackedForBundle { drops: self.drops })?;
        Err(WorldError::WrongStorageKind {
            name: String::from("reject bundle"),
        })
    }
}

struct DropTrackedForBundle {
    drops: Rc<Cell<usize>>,
}

impl Drop for DropTrackedForBundle {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn rejected_existing_entity_bundle_leaves_the_batch_unchanged() {
    let drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<DropTrackedForBundle>(ComponentOptions::sparse())
        .expect("tracked");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    let error = world
        .commands()
        .expect("commands")
        .insert_bundle(
            entity,
            RejectedExistingBundle {
                drops: Rc::clone(&drops),
            },
        )
        .expect_err("reject");

    assert!(matches!(error, WorldError::WrongStorageKind { .. }));
    assert_eq!(drops.get(), 1);
    assert!(!world.has_pending_commands());
}
