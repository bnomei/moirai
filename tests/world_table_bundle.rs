use moirai::component::ComponentOptions;
use moirai::world::WorldBuilder;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Velocity(i32);

#[test]
fn spawn_bundle_with_table_components() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    let mut world = builder.build().expect("build");

    let entity = world
        .spawn_bundle((Position(1), Velocity(2)))
        .expect("spawn bundle");
    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get position")
            .map(|p| p.0),
        Some(1)
    );
    assert_eq!(
        world
            .get::<Velocity>(entity)
            .expect("get velocity")
            .map(|v| v.0),
        Some(2)
    );
}
