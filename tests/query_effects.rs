//! Query-side effects and deferred commands during schedule execution.
use moirai::component::ComponentOptions;
use moirai::event::{EventOptions, EventReaderStart};
use moirai::query::{QueryParams, QuerySpec};
use moirai::schedule::{stage, System};
use moirai::world::WorldBuilder;
use moirai::{AppBuilder, QueryError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Velocity(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Damage(u32);

struct Spawned(moirai::EntityId);

#[test]
fn query_effects_spawn_during_update_commits_on_flush() {
    let mut app_builder = AppBuilder::new();
    app_builder
        .world_builder()
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    app_builder.world_builder().register_resource::<Spawned>();
    app_builder
        .add_system(System::new("seed", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(entity, Position(1))
                .expect("insert");
        }))
        .expect("seed");
    app_builder
        .add_system(System::new("mutate", stage::UPDATE, |world, _dt| {
            let mut spawned = None;
            world
                .for_each_mut_with_effects::<Position>(
                    &QuerySpec::new(),
                    QueryParams::new(),
                    |_, pos, effects| {
                        pos.0 += 1;
                        spawned = Some(
                            effects
                                .commands()
                                .expect("commands")
                                .spawn()
                                .expect("spawn"),
                        );
                        Ok(())
                    },
                )
                .expect("mutate");
            let entity = spawned.expect("spawned");
            world.insert_resource(Spawned(entity)).expect("track");
        }))
        .expect("mutate");
    let mut app = app_builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    let spawned = app
        .world()
        .resource::<Spawned>()
        .expect("resource")
        .expect("tracked")
        .0;
    assert!(app.world().is_alive(spawned));
}

#[test]
fn query_effects_send_during_update() {
    let mut app_builder = AppBuilder::new();
    app_builder
        .world_builder()
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    app_builder
        .world_builder()
        .add_event::<Damage>(EventOptions::manual())
        .expect("event");
    app_builder
        .add_system(System::new("seed", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(entity, Position(1))
                .expect("insert");
        }))
        .expect("seed");
    app_builder
        .add_system(System::new("emit", stage::UPDATE, |world, _dt| {
            world
                .for_each_mut_with_effects::<Position>(
                    &QuerySpec::new(),
                    QueryParams::new(),
                    |_, _, effects| {
                        effects.send(Damage(7)).expect("send");
                        Ok(())
                    },
                )
                .expect("mutate");
        }))
        .expect("emit");
    let mut app = app_builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    let mut reader = app
        .world_mut()
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert_eq!(
        app.world_mut()
            .read_event(&mut reader)
            .expect("read")
            .map(|d| d.0),
        Some(7)
    );
}

#[test]
fn query_effects_despawn_during_update() {
    struct Victim(moirai::EntityId);

    let mut app_builder = AppBuilder::new();
    app_builder
        .world_builder()
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    app_builder.world_builder().register_resource::<Victim>();
    app_builder
        .add_system(System::new("seed", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(entity, Position(1))
                .expect("insert");
            world.insert_resource(Victim(entity)).expect("track");
        }))
        .expect("seed");
    app_builder
        .add_system(System::new("cull", stage::UPDATE, |world, _dt| {
            let victim = world
                .resource::<Victim>()
                .expect("resource")
                .expect("victim")
                .0;
            world
                .for_each_mut_with_effects::<Position>(
                    &QuerySpec::new(),
                    QueryParams::new(),
                    |_, _, effects| {
                        effects
                            .commands()
                            .expect("commands")
                            .despawn(victim)
                            .expect("despawn");
                        Ok(())
                    },
                )
                .expect("mutate");
        }))
        .expect("cull");
    let mut app = app_builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    let victim = app
        .world()
        .resource::<Victim>()
        .expect("resource")
        .expect("victim")
        .0;
    assert!(!app.world().is_alive(victim));
}

#[test]
fn query_effects_send_unregistered_event_errors() {
    let mut world = WorldBuilder::new();
    world
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    let mut world = world.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let err = world
        .for_each_mut_with_effects::<Position>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, _, effects| {
                effects.send(Damage(1))?;
                Ok(())
            },
        )
        .expect_err("unregistered");

    assert!(matches!(err, QueryError::WrongQuery { .. }));
}

#[test]
fn query2_effects_spawn_during_update_commits_on_flush() {
    let mut app_builder = AppBuilder::new();
    app_builder
        .world_builder()
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    app_builder
        .world_builder()
        .register_component::<Velocity>(ComponentOptions::sparse())
        .expect("register");
    app_builder.world_builder().register_resource::<Spawned>();
    app_builder
        .add_system(System::new("seed", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(entity, Position(1))
                .expect("insert");
            world
                .commands()
                .expect("commands")
                .insert(entity, Velocity(1))
                .expect("vel");
        }))
        .expect("seed");
    app_builder
        .add_system(System::new("mutate", stage::UPDATE, |world, _dt| {
            let mut spawned = None;
            world
                .for_each2_mut_with_effects::<Position, Velocity>(
                    &QuerySpec::new(),
                    QueryParams::new(),
                    |_, _, _, effects| {
                        spawned = Some(
                            effects
                                .commands()
                                .expect("commands")
                                .spawn()
                                .expect("spawn"),
                        );
                        Ok(())
                    },
                )
                .expect("mutate");
            let entity = spawned.expect("spawned");
            world.insert_resource(Spawned(entity)).expect("track");
        }))
        .expect("mutate");
    let mut app = app_builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    let spawned = app
        .world()
        .resource::<Spawned>()
        .expect("resource")
        .expect("tracked")
        .0;
    assert!(app.world().is_alive(spawned));
}

#[test]
fn query_effects_rejects_commands_during_render() {
    let mut app_builder = AppBuilder::new();
    app_builder
        .world_builder()
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    app_builder
        .add_system(System::new("seed", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(entity, Position(1))
                .expect("insert");
        }))
        .expect("seed");
    app_builder
        .add_system(System::new("draw", stage::RENDER, |world, _dt| {
            let result = world.for_each_mut_with_effects::<Position>(
                &QuerySpec::new(),
                QueryParams::new(),
                |_, _, effects| {
                    let _ = effects.commands()?;
                    Ok(())
                },
            );
            assert!(matches!(result, Err(QueryError::BorrowConflict { .. })));
        }))
        .expect("draw");
    let mut app = app_builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    app.render(1.0 / 60.0).expect("render");
}
