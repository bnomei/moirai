use moirai::component::ComponentOptions;
use moirai::query::{QueryParams, QuerySpec};
use moirai::world::WorldBuilder;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Velocity(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mass(i32);

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

#[test]
fn table_query2_mutates_both_components() {
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
        .expect("spawn");

    world
        .for_each2_mut::<Position, Velocity>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, pos, vel| {
                pos.0 += vel.0;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world.get::<Position>(entity).expect("get").map(|p| p.0),
        Some(3)
    );
}

#[test]
fn table_for_each2_mut_with_reversed_component_indices() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Mass>(ComponentOptions::table())
        .expect("mass");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    let mut world = builder.build().expect("build");

    let entity = world.spawn().expect("spawn");
    world.insert(entity, Mass(5)).expect("mass");
    world.insert(entity, Velocity(10)).expect("vel");

    world
        .for_each2_mut::<Velocity, Mass>(&QuerySpec::new(), QueryParams::new(), |_, vel, mass| {
            vel.0 += mass.0;
            mass.0 *= 2;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world.get::<Velocity>(entity).expect("vel").map(|v| v.0),
        Some(15)
    );
    assert_eq!(
        world.get::<Mass>(entity).expect("mass").map(|v| v.0),
        Some(10)
    );
}

#[test]
fn table_insert_replace_preserves_added_tick() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    world.insert(entity, Position(1)).expect("add");
    let since_after_add = world.change_tick();
    world.insert(entity, Position(2)).expect("replace");

    let added: Vec<_> = world
        .query::<Position>(
            &QuerySpec::new().added::<Position>(),
            QueryParams::new().since(since_after_add),
        )
        .expect("added")
        .map(|(_, p)| p.0)
        .collect();
    assert!(added.is_empty());

    let changed: Vec<_> = world
        .query::<Position>(
            &QuerySpec::new().changed::<Position>(),
            QueryParams::new().since(since_after_add),
        )
        .expect("changed")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(changed, vec![2]);
}
