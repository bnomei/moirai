use moirai::world::WorldBuilder;
use moirai::ChangeTick;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug, PartialEq)]
struct Score(i32);

#[test]
fn resource_insert_get_remove_round_trip() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");

    assert!(!world.contains_resource::<Score>());
    assert!(world.insert_resource(Score(10)).expect("insert").is_none());
    assert_eq!(world.resource::<Score>().expect("get"), Some(&Score(10)));

    let replaced = world.insert_resource(Score(20)).expect("replace");
    assert_eq!(replaced, Some(Score(10)));
    assert_eq!(
        world.resource_changed_tick::<Score>().expect("tick"),
        Some(ChangeTick::from_raw(2))
    );

    assert_eq!(
        world.remove_resource::<Score>().expect("remove"),
        Some(Score(20))
    );
    assert!(!world.contains_resource::<Score>());
}

#[test]
fn builder_seed_registers_resource_at_the_initial_tick() {
    let mut builder = WorldBuilder::new();
    builder.insert_resource(Score(10));
    let world = builder.build().expect("build");

    assert_eq!(world.resource::<Score>().expect("get"), Some(&Score(10)));
    assert_eq!(
        world.resource_added_tick::<Score>().expect("added tick"),
        Some(ChangeTick::from_raw(1))
    );
    assert_eq!(
        world
            .resource_changed_tick::<Score>()
            .expect("changed tick"),
        Some(ChangeTick::from_raw(1))
    );
}

#[test]
fn duplicate_builder_seed_is_last_call_wins_with_one_initial_tick() {
    let mut builder = WorldBuilder::new();
    builder.insert_resource(Score(10));
    builder.insert_resource(Score(20));
    let world = builder.build().expect("build");

    assert_eq!(world.resource::<Score>().expect("get"), Some(&Score(20)));
    assert_eq!(
        world.resource_added_tick::<Score>().expect("added tick"),
        Some(ChangeTick::from_raw(1))
    );
    assert_eq!(
        world
            .resource_changed_tick::<Score>()
            .expect("changed tick"),
        Some(ChangeTick::from_raw(1))
    );
}

#[test]
fn resource_scope_reports_missing_without_mutation() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");

    let seen = world
        .resource_scope::<Score, _>(|value, _| value.is_none())
        .expect("scope");
    assert!(seen);
    assert!(!world.contains_resource::<Score>());
}

#[test]
fn resource_scope_updates_value() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(1)).expect("seed");

    world
        .resource_scope::<Score, _>(|value, _| {
            if let Some(score) = value {
                score.0 = 5;
            }
        })
        .expect("scope");

    assert_eq!(world.resource::<Score>().expect("get"), Some(&Score(5)));
}

#[test]
fn resource_added_and_changed_ticks_absent_when_missing() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let world = builder.build().expect("build");
    assert_eq!(world.resource_changed_tick::<Score>().expect("tick"), None);
    assert_eq!(world.resource::<Score>().expect("get"), None);
}

#[test]
fn resource_scope_rejects_revision_reads_of_the_scoped_resource() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(1)).expect("seed");

    let result = world
        .resource_scope::<Score, _>(|_, world| world.resource_changed_tick::<Score>())
        .expect("scope result");

    assert!(matches!(
        result,
        Err(moirai::world::WorldError::ResourceScoped { .. })
    ));
}

#[test]
fn resource_scope_restores_present_resource_after_unwind() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(7)).expect("seed");

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = world.resource_scope::<Score, _>(|value, _| {
            value.expect("present").0 = 9;
            panic!("scope panic");
        });
    }));
    assert!(panic.is_err());
    assert_eq!(
        world.resource::<Score>().expect("restored"),
        Some(&Score(9))
    );

    let seen = world
        .resource_scope::<Score, _>(|value, _| value.map(|score| score.0))
        .expect("sentinel cleared");
    assert_eq!(seen, Some(9));
}

#[test]
fn resource_scope_clears_missing_resource_sentinel_after_unwind() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = world.resource_scope::<Score, _>(|value, _| {
            assert!(value.is_none());
            panic!("scope panic");
        });
    }));
    assert!(panic.is_err());

    let missing = world
        .resource_scope::<Score, _>(|value, _| value.is_none())
        .expect("sentinel cleared");
    assert!(missing);
}

struct DropTracked(Rc<Cell<usize>>);

impl Drop for DropTracked {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn resource_scope_drops_values_exactly_once_across_exit_paths() {
    let normal_drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder.register_resource::<DropTracked>();
    let mut world = builder.build().expect("build");
    world
        .insert_resource(DropTracked(Rc::clone(&normal_drops)))
        .expect("seed");
    world
        .resource_scope::<DropTracked, _>(|value, _| assert!(value.is_some()))
        .expect("normal scope");
    assert_eq!(normal_drops.get(), 0);
    drop(world.remove_resource::<DropTracked>().expect("remove"));
    assert_eq!(normal_drops.get(), 1);

    let replacement_drops = Rc::new(Cell::new(0));
    world
        .insert_resource(DropTracked(Rc::clone(&replacement_drops)))
        .expect("seed replacement case");
    drop(
        world
            .insert_resource(DropTracked(Rc::clone(&replacement_drops)))
            .expect("replace"),
    );
    assert_eq!(replacement_drops.get(), 1);
    drop(
        world
            .remove_resource::<DropTracked>()
            .expect("remove replacement"),
    );
    assert_eq!(replacement_drops.get(), 2);

    let unwind_drops = Rc::new(Cell::new(0));
    world
        .insert_resource(DropTracked(Rc::clone(&unwind_drops)))
        .expect("seed unwind case");
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = world.resource_scope::<DropTracked, _>(|value, _| {
            assert!(value.is_some());
            panic!("scope panic");
        });
    }));
    assert!(panic.is_err());
    assert_eq!(unwind_drops.get(), 0);
    drop(
        world
            .remove_resource::<DropTracked>()
            .expect("remove unwind"),
    );
    assert_eq!(unwind_drops.get(), 1);
}
