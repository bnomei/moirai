use moirai::component::ComponentOptions;
use moirai::event::EventOptions;
#[cfg(feature = "testkit")]
use moirai::testkit::WorldTestExt;
use moirai::world::{Bundle, BundleWriter, WorldBuilder, WorldError};
#[cfg(feature = "testkit")]
use moirai::ChangeTick;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Health(i32);

struct FailingBundle;

impl Bundle for FailingBundle {
    fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError> {
        writer.insert(Health(1))?;
        Err(WorldError::WrongStorageKind {
            name: String::from("bundle failed"),
        })
    }
}

struct DropTracked {
    drops: Rc<Cell<usize>>,
}

impl Drop for DropTracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

struct FailingOwnedBundle {
    drops: Rc<Cell<usize>>,
}

impl Bundle for FailingOwnedBundle {
    fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError> {
        writer.insert(DropTracked { drops: self.drops })?;
        Err(WorldError::WrongStorageKind {
            name: String::from("owned bundle failed"),
        })
    }
}

#[derive(Debug, PartialEq)]
struct Score(i32);

#[test]
fn deferred_spawn_bundle_rolls_back_on_bundle_error() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");

    assert!(matches!(
        world
            .commands()
            .expect("commands")
            .spawn_bundle(FailingBundle),
        Err(WorldError::WrongStorageKind { .. })
    ));
    assert!(!world.has_pending_commands());
}

#[test]
fn immediate_spawn_bundle_rolls_back_on_bundle_error() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");

    assert!(matches!(
        world.spawn_bundle(FailingBundle),
        Err(WorldError::WrongStorageKind { .. })
    ));
    assert!(!world.has_pending_commands());
}

#[test]
fn failing_bundle_rollbacks_drop_non_clone_table_values_once() {
    let immediate_drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<DropTracked>(ComponentOptions::table())
        .expect("register");
    let mut world = builder.build().expect("build");

    assert!(matches!(
        world.spawn_bundle(FailingOwnedBundle {
            drops: Rc::clone(&immediate_drops),
        }),
        Err(WorldError::WrongStorageKind { .. })
    ));
    assert_eq!(immediate_drops.get(), 1);

    let deferred_drops = Rc::new(Cell::new(0));
    assert!(matches!(
        world
            .commands()
            .expect("commands")
            .spawn_bundle(FailingOwnedBundle {
                drops: Rc::clone(&deferred_drops),
            }),
        Err(WorldError::WrongStorageKind { .. })
    ));
    assert_eq!(deferred_drops.get(), 1);
    assert!(!world.has_pending_commands());
}

#[test]
#[cfg(feature = "testkit")]
fn poisoned_world_rejects_new_commands_and_can_discard_existing_ones() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");

    let entity = world.spawn().expect("spawn");
    world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
    world.insert(entity, Health(1)).expect("consume last tick");
    assert!(matches!(
        world.insert(entity, Health(2)),
        Err(WorldError::ChangeTickExhausted)
    ));

    assert!(matches!(
        world.commands().expect("commands").despawn(entity),
        Err(WorldError::ChangeTickExhausted)
    ));
    assert!(!world.has_pending_commands());
}

#[test]
fn unregistered_resource_insert_does_not_advance_change_tick() {
    let mut world = WorldBuilder::new().build().expect("build");
    assert!(matches!(
        world.insert_resource(Score(1)),
        Err(WorldError::UnregisteredResource { .. })
    ));
}

#[test]
fn absent_resource_mut_does_not_advance_change_tick() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");

    assert!(world.resource_mut::<Score>().expect("missing").is_none());
    assert_eq!(world.resource_changed_tick::<Score>().expect("tick"), None);
}

#[test]
fn post_build_event_registration_is_only_on_builder() {
    assert!(WorldBuilder::new()
        .add_event::<Health>(EventOptions::manual())
        .is_ok());
}
