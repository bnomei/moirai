use moirai::world::WorldBuilder;
use moirai::ChangeTick;

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
