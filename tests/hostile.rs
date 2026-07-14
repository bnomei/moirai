use moirai::component::{ComponentOptions, RegistrationError};
use moirai::event::{EventOptions, EventReaderStart};
use moirai::query::{QueryError, QueryPolicy, QuerySpec, QueryWindow};
use moirai::schedule::{stage, BuildError, ScheduleBuilder, System};
#[cfg(feature = "testkit")]
use moirai::testkit::WorldTestExt;
use moirai::world::WorldBuilder;
use moirai::world::WorldError;
use moirai::{AppBuilder, AppError, DenseEntityScratch, EntityScratchError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Damage(u32);

#[derive(Clone, Copy)]
struct Position(#[allow(dead_code)] i32);

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Velocity(#[allow(dead_code)] i32);

fn sparse_world() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder.build().expect("build")
}

#[allow(dead_code)]
fn sparse_pair_world() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register position");
    builder
        .register_component::<Velocity>(ComponentOptions::sparse())
        .expect("register velocity");
    builder.build().expect("build")
}

#[test]
fn entity_scratch_never_falls_through_to_a_foreign_entity_handle() {
    let mut world_a = sparse_world();
    let mut world_b = sparse_world();
    let entity_a = world_a.spawn().expect("spawn a");
    let entity_b = world_b.spawn().expect("spawn b");
    let mut scratch = DenseEntityScratch::new(&world_a);
    scratch.insert(&world_a, entity_a, 7).expect("insert");

    assert_eq!(
        scratch.get(&world_a, entity_b),
        Err(EntityScratchError::StaleEntity { entity: entity_b })
    );
    assert_eq!(
        scratch.get(&world_b, entity_a),
        Err(EntityScratchError::WrongWorld)
    );
    assert_eq!(scratch.get(&world_a, entity_a).expect("origin"), Some(&7));
}

#[test]
fn freed_slot_is_not_alive() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    world.despawn(entity).expect("despawn");
    assert!(!world.is_alive(entity));
}

#[test]
fn conflicting_registration_leaves_registry_unchanged() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("first");
    let err = builder
        .register_component::<Position>(ComponentOptions::table())
        .expect_err("conflict");
    assert!(matches!(err, RegistrationError::TypeConflict { .. }));
    let mut world = builder.build().expect("build");
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Prepared)
        .expect("prepare");
    let count = query
        .iter(&mut world, QueryWindow::All)
        .expect("query")
        .count();
    assert_eq!(count, 0);
}

#[test]
fn immediate_structural_mutation_during_run_is_rejected() {
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("try_spawn", stage::UPDATE, |world, _dt| {
            assert!(matches!(
                world.spawn(),
                Err(WorldError::StructuralMutationDuringRun)
            ));
        }))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
}

#[test]
fn duplicate_mutable_query2_is_rejected_before_borrow() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let mut query = world
        .prepare_query2::<Position, Position>(QuerySpec::new(), QueryPolicy::Prepared)
        .expect("prepare immutable pair");
    assert!(matches!(
        query.for_each_mut_mut(&mut world, QueryWindow::All, |_, _, _| Ok(())),
        Err(QueryError::DuplicateMutableComponent { .. })
    ));
}

#[test]
fn prepared_query_from_another_world_is_rejected() {
    let mut world_a = sparse_world();
    let mut world_b = sparse_world();

    let entity = world_a.spawn().expect("spawn");
    world_a.insert(entity, Position(1)).expect("insert");
    let mut query = world_a
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Membership)
        .expect("prepare");

    assert!(matches!(
        query.iter(&mut world_b, QueryWindow::All),
        Err(QueryError::WrongOwner)
    ));
}

#[test]
fn schedule_cycle_is_build_error_not_runtime_panic() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).before("b"))
        .expect("a");
    builder
        .add_system(System::new("b", stage::UPDATE, |_world, _dt| {}).before("a"))
        .expect("b");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::Cycle { .. })
    ));
}

#[test]
fn stale_entity_id_is_not_alive() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    world.despawn(entity).expect("despawn");
    assert!(!world.is_alive(entity));
    assert!(matches!(
        world.get::<Position>(entity),
        Err(WorldError::StaleEntity { .. })
    ));
}

#[test]
fn resource_scope_ref_cannot_reborrow_same_type() {
    #[derive(Debug, PartialEq)]
    struct Score(i32);

    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(1)).expect("insert");

    let result = world
        .resource_scope_ref::<Score, _>(|_, inner| inner.resource_changed_tick::<Score>())
        .expect("scope");
    assert!(matches!(result, Err(WorldError::ResourceScoped { .. })));
}

#[test]
fn failed_command_batch_applies_no_structural_operation() {
    let mut world = sparse_world();
    let live = world.spawn().expect("live");
    world.insert(live, Position(1)).expect("insert");

    let reserved = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    world
        .commands()
        .expect("commands")
        .despawn(live)
        .expect("queue first");
    world
        .commands()
        .expect("commands")
        .despawn(live)
        .expect("queue duplicate");

    assert!(matches!(world.flush(), Err(WorldError::Flush(_))));
    assert!(!world.is_alive(reserved));
    assert!(world.is_alive(live));
    assert!(!world.has_pending_commands());
}

#[test]
fn frame_events_are_cleared_after_update() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<Damage>(EventOptions::frame(moirai::StageOperation::Update))
        .expect("register");
    builder
        .add_system(
            System::new("send", stage::UPDATE, |world, _dt| {
                world.send(Damage(1)).expect("send");
            })
            .emits::<Damage>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    let mut reader = app
        .world_mut()
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut reader)
        .expect("after")
        .is_none());
}

#[test]
fn stale_handle_stays_dead_after_slot_reuse() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    world.despawn(entity).expect("despawn");
    let replacement = world.spawn().expect("reuse");
    assert!(!world.is_alive(entity));
    assert_ne!(entity, replacement);
    assert!(world.is_alive(replacement));
}

#[test]
fn pending_idle_commands_reject_app_update() {
    let mut app = AppBuilder::new().build().expect("app");
    let _ = app
        .world_mut()
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    assert!(matches!(
        app.update(1.0 / 60.0),
        Err(AppError::PendingIdleCommands)
    ));
}

#[test]
#[cfg(feature = "testkit")]
fn for_each_mut_rejects_poisoned_world() {
    use moirai::ChangeTick;

    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("seed");
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Prepared)
        .expect("prepare");

    world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
    world
        .insert(entity, Position(2))
        .expect("consume last tick");
    assert!(matches!(
        world.insert(entity, Position(3)),
        Err(WorldError::ChangeTickExhausted)
    ));
    assert!(world.is_mutation_poisoned());

    let err = query
        .for_each_mut(&mut world, QueryWindow::All, |_, _| Ok(()))
        .expect_err("poisoned");

    assert!(matches!(
        err,
        QueryError::BorrowConflict { detail }
            if detail.contains("world mutation is poisoned")
    ));
}

#[test]
#[cfg(feature = "testkit")]
fn poisoned_world_rejects_app_update() {
    use moirai::component::ComponentOptions;
    use moirai::ChangeTick;

    #[derive(Clone, Copy)]
    struct Health(#[allow(dead_code)] i32);

    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    builder
        .add_system(System::new("noop", stage::UPDATE, |_world, _dt| {}))
        .expect("add");
    let mut app = builder.build().expect("app");
    let entity = app.world_mut().spawn().expect("spawn");
    app.world_mut().insert(entity, Health(0)).expect("seed");
    app.world_mut()
        .set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
    app.world_mut().insert(entity, Health(1)).expect("consume");
    assert!(matches!(
        app.world_mut().insert(entity, Health(2)),
        Err(WorldError::ChangeTickExhausted)
    ));

    assert!(matches!(
        app.update(1.0 / 60.0),
        Err(AppError::WorldMutationPoisoned)
    ));
}

#[test]
#[cfg(feature = "testkit")]
fn caught_tick_exhaustion_faults_before_the_next_system() {
    use core::sync::atomic::{AtomicU32, Ordering};
    use moirai::ChangeTick;

    static LATER_RUNS: AtomicU32 = AtomicU32::new(0);
    LATER_RUNS.store(0, Ordering::SeqCst);

    #[derive(Clone, Copy)]
    struct Counter;

    let mut builder = AppBuilder::new();
    builder.insert_resource(Counter);
    builder
        .add_system(System::new("poison", stage::UPDATE, |world, _dt| {
            let _ = world.resource_mut::<Counter>();
        }))
        .expect("poison system");
    builder
        .add_system(System::new("later", stage::UPDATE, |_world, _dt| {
            LATER_RUNS.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("later system");
    let mut app = builder.build().expect("app");
    app.world_mut()
        .set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));

    assert!(matches!(app.update(0.0), Err(AppError::Fault(_))));
    assert_eq!(LATER_RUNS.load(Ordering::SeqCst), 0);
    assert_eq!(
        app.fault().and_then(|fault| fault.system.as_deref()),
        Some("poison")
    );
}

#[test]
#[cfg(feature = "testkit")]
fn for_each2_mut_preflight_rejects_insufficient_change_ticks() {
    use moirai::ChangeTick;

    let mut world = sparse_pair_world();
    let a = world.spawn().expect("a");
    let b = world.spawn().expect("b");
    world.insert(a, Position(1)).expect("a");
    world.insert(a, Velocity(1)).expect("a vel");
    world.insert(b, Position(2)).expect("b");
    world.insert(b, Velocity(2)).expect("b vel");
    let mut query = world
        .prepare_query2::<Position, Velocity>(QuerySpec::new(), QueryPolicy::Prepared)
        .expect("prepare");

    world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));

    let err = query
        .for_each_mut_mut(&mut world, QueryWindow::All, |_, _, _| Ok(()))
        .expect_err("preflight");

    assert!(matches!(
        err,
        QueryError::BorrowConflict { detail }
            if detail.contains("insufficient change ticks for query mutation")
    ));
    assert_eq!(
        world.get::<Position>(a).expect("get").expect("present").0,
        1
    );
}
