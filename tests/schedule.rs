use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use moirai::component::ComponentOptions;
use moirai::event::{EventOptions, EventReaderStart};
use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
use moirai::schedule::FlushMode;
use moirai::schedule::{stage, Condition, ScheduleBuilder, System, SystemSet};
use moirai::state::{apply, on_exit, State};
use moirai::world::WorldBuilder;
use moirai::FixedConfig;
use moirai::StageOperation;
use moirai::{AppBuilder, AppError, BuildError};

static UPDATE_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Debug, PartialEq)]
struct RoleEvent(u8);

fn build_app(system: System) -> moirai::App {
    let mut builder = AppBuilder::new();
    builder.add_system(system).expect("add");
    builder.build().expect("build")
}

#[test]
fn runtime_rejects_undeclared_system_send_without_mutating_channel() {
    let rejected = Rc::new(Cell::new(false));
    let saw_rejection = Rc::clone(&rejected);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<RoleEvent>(EventOptions::manual())
        .expect("event");
    builder
        .add_system(System::new(
            "undeclared",
            stage::UPDATE,
            move |world, _dt| {
                saw_rejection.set(world.send(RoleEvent(1)).is_err());
            },
        ))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(0.0).expect("update");
    assert!(rejected.get());
    let mut reader = app
        .world_mut()
        .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut reader)
        .expect("read")
        .is_none());
}

#[test]
fn runtime_rejects_undeclared_reader_creation() {
    let rejected = Rc::new(Cell::new(false));
    let saw_rejection = Rc::clone(&rejected);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<RoleEvent>(EventOptions::manual().external_source())
        .expect("event");
    builder
        .add_system(System::new(
            "undeclared",
            stage::UPDATE,
            move |world, _dt| {
                saw_rejection.set(
                    world
                        .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
                        .is_err(),
                );
            },
        ))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(0.0).expect("update");
    assert!(rejected.get());
    assert!(app
        .world_mut()
        .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
        .is_ok());
}

#[test]
fn runtime_rejects_undeclared_read_without_advancing_reader() {
    let reader = Rc::new(RefCell::new(None));
    let system_reader = Rc::clone(&reader);
    let rejected = Rc::new(Cell::new(false));
    let saw_rejection = Rc::clone(&rejected);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<RoleEvent>(EventOptions::manual().external_source())
        .expect("event");
    builder
        .add_system(System::new(
            "undeclared",
            stage::UPDATE,
            move |world, _dt| {
                let mut slot = system_reader.borrow_mut();
                let event_reader = slot.as_mut().expect("reader installed");
                saw_rejection.set(world.read_event(event_reader).is_err());
            },
        ))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.world_mut().send(RoleEvent(7)).expect("host send");
    *reader.borrow_mut() = Some(
        app.world_mut()
            .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
            .expect("reader"),
    );
    app.update(0.0).expect("update");
    assert!(rejected.get());
    let mut slot = reader.borrow_mut();
    assert_eq!(
        app.world_mut()
            .read_event(slot.as_mut().expect("reader"))
            .expect("host read")
            .map(|event| event.0),
        Some(7)
    );
}

#[test]
fn declared_consumer_preserves_foreign_reader_owner_error_and_cursor() {
    let reader: Rc<RefCell<Option<moirai::event::EventReader<RoleEvent>>>> =
        Rc::new(RefCell::new(None));
    let system_reader = Rc::clone(&reader);
    let saw_owner_mismatch = Rc::new(Cell::new(false));
    let system_saw_owner_mismatch = Rc::clone(&saw_owner_mismatch);

    let mut app_builder = AppBuilder::new();
    app_builder
        .world_builder()
        .add_event::<RoleEvent>(EventOptions::manual().external_source())
        .expect("app event");
    app_builder
        .add_system(
            System::new("consumer", stage::UPDATE, move |world, _dt| {
                let mut slot = system_reader.borrow_mut();
                let error = world
                    .read_event(slot.as_mut().expect("foreign reader"))
                    .expect_err("owner mismatch");
                system_saw_owner_mismatch.set(matches!(
                    error,
                    moirai::world::EventReadError::OwnerMismatch { .. }
                ));
            })
            .consumes::<RoleEvent>(),
        )
        .expect("consumer");
    let mut app = app_builder.build().expect("app");

    let mut foreign_builder = WorldBuilder::new();
    foreign_builder
        .add_event::<RoleEvent>(EventOptions::manual())
        .expect("foreign event");
    let mut foreign = foreign_builder.build().expect("foreign world");
    foreign.send(RoleEvent(13)).expect("foreign send");
    *reader.borrow_mut() = Some(
        foreign
            .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
            .expect("foreign reader"),
    );

    app.update(0.0).expect("update");
    assert!(saw_owner_mismatch.get());
    assert_eq!(
        foreign
            .read_event(reader.borrow_mut().as_mut().expect("reader"))
            .expect("foreign read")
            .map(|event| event.0),
        Some(13)
    );
}

#[test]
fn event_role_guard_restores_idle_host_access_after_system_error() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<RoleEvent>(EventOptions::manual())
        .expect("event");
    builder
        .add_system(
            System::try_new("fail", stage::UPDATE, |_world, _dt| {
                Err(String::from("stop"))
            })
            .emits::<RoleEvent>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    assert!(app.update(0.0).is_err());
    assert!(app.world().run_guard_is_idle());
    app.world_mut().send(RoleEvent(9)).expect("idle host send");
    let mut reader = app
        .world_mut()
        .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
        .expect("idle host reader");
    assert_eq!(
        app.world_mut()
            .read_event(&mut reader)
            .expect("idle host read")
            .map(|event| event.0),
        Some(9)
    );
}

#[test]
fn lifecycle_consumer_observes_events_after_structural_flush() {
    #[derive(Clone)]
    struct Health;

    let observed = Rc::new(Cell::new(false));
    let saw_event = Rc::clone(&observed);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("component");
    builder
        .add_system(System::new("seed", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(entity, Health)
                .expect("insert");
        }))
        .expect("seed");
    builder
        .add_system(
            System::new("consume", stage::UPDATE, move |world, _dt| {
                let mut reader = world
                    .on_add_reader::<Health>(EventReaderStart::OldestRetained)
                    .expect("reader");
                saw_event.set(world.read_event(&mut reader).expect("read").is_some());
            })
            .consumes_on_add::<Health>(),
        )
        .expect("consumer");
    let mut app = builder.build().expect("app");
    app.update(0.0).expect("update");
    assert!(observed.get());
}

#[test]
fn declared_consumer_creates_reader_and_reads_ordered_event() {
    let observed = Rc::new(Cell::new(None));
    let saw_event = Rc::clone(&observed);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<RoleEvent>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    builder
        .add_system(
            System::new("producer", stage::UPDATE, |world, _dt| {
                world.send(RoleEvent(11)).expect("send");
            })
            .emits::<RoleEvent>(),
        )
        .expect("producer");
    builder
        .add_system(
            System::new("consumer", stage::UPDATE, move |world, _dt| {
                let mut reader = world
                    .event_reader::<RoleEvent>(EventReaderStart::OldestRetained)
                    .expect("reader");
                saw_event.set(
                    world
                        .read_event(&mut reader)
                        .expect("read")
                        .map(|event| event.0),
                );
            })
            .consumes::<RoleEvent>()
            .after("producer"),
        )
        .expect("consumer");
    let mut app = builder.build().expect("app");
    app.update(0.0).expect("update");
    assert_eq!(observed.get(), Some(11));
}

#[test]
fn remove_lifecycle_consumer_observes_event_after_system_flush() {
    #[derive(Clone)]
    struct Health;

    let entity = Rc::new(Cell::new(None));
    let seeded_entity = Rc::clone(&entity);
    let removed_entity = Rc::clone(&entity);
    let observed = Rc::new(Cell::new(false));
    let saw_event = Rc::clone(&observed);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("component");
    builder
        .add_system(System::new("seed", stage::STARTUP, move |world, _dt| {
            let id = world.commands().expect("commands").spawn().expect("spawn");
            world
                .commands()
                .expect("commands")
                .insert(id, Health)
                .expect("insert");
            seeded_entity.set(Some(id));
        }))
        .expect("seed");
    builder
        .add_system(
            System::new("remove", stage::UPDATE, move |world, _dt| {
                world
                    .commands()
                    .expect("commands")
                    .remove::<Health>(removed_entity.get().expect("seeded"))
                    .expect("remove");
            })
            .flush_after(),
        )
        .expect("remove");
    builder
        .add_system(
            System::new("consume", stage::UPDATE, move |world, _dt| {
                let mut reader = world
                    .on_remove_reader::<Health>(EventReaderStart::OldestRetained)
                    .expect("reader");
                saw_event.set(world.read_event(&mut reader).expect("read").is_some());
            })
            .consumes_on_remove::<Health>()
            .after("remove"),
        )
        .expect("consumer");
    let mut app = builder.build().expect("app");
    app.update(0.0).expect("update");
    assert!(observed.get());
}

#[test]
fn app_runs_update_system_in_order() {
    UPDATE_COUNT.store(0, Ordering::SeqCst);
    let mut app = build_app(System::new("count", stage::UPDATE, |_world, _dt| {
        UPDATE_COUNT.fetch_add(1, Ordering::SeqCst);
    }));

    app.update(1.0 / 60.0).expect("update");
    assert_eq!(UPDATE_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(app.world().world_tick().raw(), 1);
}

#[test]
fn startup_runs_once_across_updates() {
    static STARTUP_COUNT: AtomicU32 = AtomicU32::new(0);
    STARTUP_COUNT.store(0, Ordering::SeqCst);

    let mut app = build_app(System::new("startup", stage::STARTUP, |_world, _dt| {
        STARTUP_COUNT.fetch_add(1, Ordering::SeqCst);
    }));

    app.update(1.0 / 60.0).expect("first");
    app.update(1.0 / 60.0).expect("second");
    assert_eq!(STARTUP_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn registration_order_tie_breaks_system_order() {
    static ORDER: AtomicU32 = AtomicU32::new(0);

    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("b", stage::UPDATE, |_world, _dt| {
            assert_eq!(ORDER.fetch_add(1, Ordering::SeqCst), 0);
        }))
        .expect("b");
    builder
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {
            assert_eq!(ORDER.fetch_add(1, Ordering::SeqCst), 1);
        }))
        .expect("a");
    let mut app = builder.build().expect("build");
    ORDER.store(0, Ordering::SeqCst);
    app.update(1.0 / 60.0).expect("update");
}

#[test]
fn world_predicate_gates_systems_and_sets_from_read_only_resources() {
    static RAN: AtomicU32 = AtomicU32::new(0);

    #[derive(Clone)]
    struct Gate {
        open: bool,
    }

    let enabled = Condition::from_world(|world| {
        world
            .resource::<Gate>()
            .expect("gate resource")
            .is_some_and(|gate| gate.open)
    });
    let set = SystemSet::new("gated");
    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Gate>();
    builder.register_set(set.clone()).expect("set");
    builder
        .set_run_if(&set, enabled.clone())
        .expect("set condition");
    builder
        .add_system(
            System::new("run", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .in_set(&set)
            .run_if(enabled),
        )
        .expect("system");
    let mut app = builder.build().expect("app");

    RAN.store(0, Ordering::SeqCst);
    app.world_mut()
        .insert_resource(Gate { open: false })
        .expect("closed gate");
    app.update(0.0).expect("closed update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);

    app.world_mut()
        .insert_resource(Gate { open: true })
        .expect("open gate");
    app.update(0.0).expect("open update");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn cycle_is_rejected_at_build() {
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
fn cross_stage_system_edge_is_rejected_at_build() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::STARTUP, |_world, _dt| {}).before("b"))
        .expect("a");
    builder
        .add_system(System::new("b", stage::UPDATE, |_world, _dt| {}))
        .expect("b");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::CrossStageSystemEdge { .. })
    ));
}

#[test]
fn system_to_set_and_set_to_set_edges_expand_deterministically() {
    static ORDER: AtomicU32 = AtomicU32::new(0);

    fn record(digit: u32) {
        ORDER
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                Some(value * 10 + digit)
            })
            .expect("record order");
    }

    let early = SystemSet::new("early");
    let late = SystemSet::new("late");
    let mut builder = AppBuilder::new();
    builder.register_set(early.clone()).expect("early set");
    builder.register_set(late.clone()).expect("late set");
    builder
        .order_set_before(&early, &late)
        .expect("set ordering");
    builder
        .add_system(System::new("tail", stage::UPDATE, |_world, _dt| record(4)).in_set(&late))
        .expect("tail");
    builder
        .add_system(
            System::new("middle", stage::UPDATE, |_world, _dt| record(3))
                .after_set(&early)
                .before_set(&late),
        )
        .expect("middle");
    builder
        .add_system(System::new("early-b", stage::UPDATE, |_world, _dt| record(2)).in_set(&early))
        .expect("early-b");
    builder
        .add_system(System::new("early-a", stage::UPDATE, |_world, _dt| record(1)).in_set(&early))
        .expect("early-a");
    let mut app = builder.build().expect("app");

    ORDER.store(0, Ordering::SeqCst);
    app.update(0.0).expect("update");
    assert_eq!(ORDER.load(Ordering::SeqCst), 2134);
}

#[test]
fn expanded_set_edges_participate_in_cycles_and_reject_cross_stage_ordering() {
    let first = SystemSet::new("first");
    let second = SystemSet::new("second");

    let mut world = WorldBuilder::new().build().expect("world");
    let mut cycle = ScheduleBuilder::standard();
    cycle.register_set(first.clone()).expect("first");
    cycle.register_set(second.clone()).expect("second");
    cycle
        .order_set_before(&first, &second)
        .expect("first before second");
    cycle
        .order_set_after(&first, &second)
        .expect("first after second");
    cycle
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).in_set(&first))
        .expect("a");
    cycle
        .add_system(System::new("b", stage::UPDATE, |_world, _dt| {}).in_set(&second))
        .expect("b");
    assert!(matches!(
        cycle.build(&mut world),
        Err(BuildError::Cycle { .. })
    ));

    let mut world = WorldBuilder::new().build().expect("world");
    let mut cross_stage = ScheduleBuilder::standard();
    cross_stage.register_set(first.clone()).expect("first");
    cross_stage.register_set(second.clone()).expect("second");
    cross_stage
        .order_set_before(&first, &second)
        .expect("set ordering");
    cross_stage
        .add_system(System::new("startup", stage::STARTUP, |_world, _dt| {}).in_set(&first))
        .expect("startup");
    cross_stage
        .add_system(System::new("update", stage::UPDATE, |_world, _dt| {}).in_set(&second))
        .expect("update");
    assert!(matches!(
        cross_stage.build(&mut world),
        Err(BuildError::CrossStageSystemEdge { from, to })
            if from == "startup" && to == "update"
    ));
}

#[test]
fn empty_registered_sets_keep_normal_ordering_semantics() {
    let empty = SystemSet::new("empty");
    let occupied = SystemSet::new("occupied");
    let missing = SystemSet::new("missing");
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder.register_set(empty.clone()).expect("empty");
    builder.register_set(occupied.clone()).expect("occupied");
    builder
        .order_set_before(&empty, &occupied)
        .expect("empty ordering");
    builder
        .add_system(
            System::new("work", stage::UPDATE, |_world, _dt| {})
                .in_set(&occupied)
                .after_set(&empty),
        )
        .expect("work");
    assert!(matches!(
        builder.order_set_before(&empty, &missing),
        Err(BuildError::UnknownSystemSet { label }) if label == "missing"
    ));
    builder
        .build(&mut world)
        .expect("empty relations are inert");
}

#[test]
fn unknown_system_set_is_rejected_at_build() {
    let world = WorldBuilder::new().build().expect("world");
    let set = SystemSet::new("missing");
    let result = ScheduleBuilder::standard()
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).in_set(&set));
    assert!(matches!(result, Err(BuildError::UnknownSystemSet { .. })));
    let _ = world;
}

#[test]
fn stage_operation_mismatch_is_rejected() {
    let mut builder = ScheduleBuilder::new();
    builder
        .add_stage(stage::UPDATE, StageOperation::Update)
        .expect("update");
    assert!(matches!(
        builder.add_stage(stage::UPDATE, StageOperation::Render),
        Err(BuildError::StageOperationMismatch { .. })
    ));
}

#[test]
fn set_run_if_unknown_set_is_rejected() {
    let set = SystemSet::new("missing");
    let mut builder = ScheduleBuilder::standard();
    assert!(matches!(
        builder.set_run_if(&set, Condition::always()),
        Err(BuildError::UnknownSystemSet { .. })
    ));
}

#[test]
fn add_system_unknown_stage_is_rejected() {
    let mut builder = ScheduleBuilder::standard();
    assert!(matches!(
        builder.add_system(System::new("orphan", "missing", |_world, _dt| {})),
        Err(BuildError::UnknownStage { .. })
    ));
}

#[test]
fn build_rejects_unknown_system_edge_and_self_edge() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).before("missing"))
        .expect("a");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::UnknownSystem { .. })
    ));

    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("self_edge", stage::UPDATE, |_world, _dt| {}).before("self_edge"))
        .expect("self");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::SelfEdge { .. })
    ));
}

#[test]
fn build_rejects_live_lease_already_attached() {
    let mut world = WorldBuilder::new().build().expect("world");
    let _schedule = ScheduleBuilder::standard()
        .build(&mut world)
        .expect("schedule");
    assert!(matches!(
        ScheduleBuilder::standard().build(&mut world),
        Err(BuildError::LiveLeaseAlreadyAttached)
    ));
}

#[test]
fn fixed_config_without_fixed_update_stage_is_rejected() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::new();
    builder
        .add_stage(stage::UPDATE, StageOperation::Update)
        .expect("update");
    builder.fixed(
        FixedConfig::new(Duration::from_secs_f32(1.0 / 60.0))
            .expect("fixed")
            .with_max_substeps(4)
            .expect("substeps"),
    );
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::FixedConfigWithoutFixedUpdate)
    ));
}

#[test]
fn schedule_builder_default_constructs() {
    let _builder = ScheduleBuilder::default();
}

#[test]
fn duplicate_system_labels_are_rejected_at_build() {
    let world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("dup", stage::UPDATE, |_world, _dt| {}))
        .expect("first");
    assert!(matches!(
        builder.add_system(System::new("dup", stage::UPDATE, |_world, _dt| {})),
        Err(BuildError::DuplicateSystemLabel { .. })
    ));
    let _ = world;
}

#[test]
fn pending_idle_commands_reject_update() {
    let mut app = build_app(System::new("noop", stage::UPDATE, |_world, _dt| {}));

    let entity = app.world_mut().spawn().expect("spawn");
    let _ = app
        .world_mut()
        .commands()
        .expect("commands")
        .despawn(entity);
    assert!(matches!(
        app.update(1.0 / 60.0),
        Err(AppError::PendingIdleCommands)
    ));
}

#[test]
fn fallible_system_faults_app_and_leaves_world_idle() {
    let mut app = build_app(System::try_new("fail", stage::UPDATE, |_world, _dt| {
        Err("expected".to_string())
    }));

    assert!(matches!(app.update(1.0 / 60.0), Err(AppError::Fault(_))));
    assert!(app.is_faulted());
    assert!(app.world().run_guard_is_idle());
    assert!(matches!(
        app.update(1.0 / 60.0),
        Err(AppError::TerminalFault)
    ));
    assert_eq!(
        app.fault().and_then(|fault| fault.detail.as_deref()),
        Some("expected")
    );
}

#[test]
fn run_if_skips_system_body() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    let mut app = build_app(
        System::new("gated", stage::UPDATE, |_world, _dt| {
            RAN.fetch_add(1, Ordering::SeqCst);
        })
        .run_if(Condition::never()),
    );
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
}

#[test]
fn set_run_if_gates_all_members_once_per_stage() {
    static A: AtomicU32 = AtomicU32::new(0);
    static B: AtomicU32 = AtomicU32::new(0);
    let set = SystemSet::new("sim");

    let mut builder = AppBuilder::new();
    builder.register_set(set.clone()).expect("set");
    builder.set_run_if(&set, Condition::never()).expect("cond");
    builder
        .add_system(
            System::new("a", stage::UPDATE, |_world, _dt| {
                A.fetch_add(1, Ordering::SeqCst);
            })
            .in_set(&set),
        )
        .expect("a");
    builder
        .add_system(
            System::new("b", stage::UPDATE, |_world, _dt| {
                B.fetch_add(1, Ordering::SeqCst);
            })
            .in_set(&set),
        )
        .expect("b");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(A.load(Ordering::SeqCst), 0);
    assert_eq!(B.load(Ordering::SeqCst), 0);
}

#[test]
fn in_state_gates_execution() {
    static RAN: AtomicU32 = AtomicU32::new(0);

    let mut builder = AppBuilder::new();
    builder.insert_state(1u8);
    builder
        .add_system(
            System::new("gate", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::in_state(2u8)),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
}

#[test]
fn state_changed_runs_after_explicit_apply() {
    static RAN: AtomicU32 = AtomicU32::new(0);

    let mut builder = AppBuilder::new();
    builder.insert_state(1u8);
    builder
        .add_system(apply::<u8>("apply", stage::UPDATE))
        .expect("apply");
    builder
        .add_system(
            System::new("watch", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .after("apply")
            .run_if(Condition::state_changed::<u8>()),
        )
        .expect("watch");
    let mut app = builder.build().expect("build");
    app.world_mut()
        .resource_mut::<State<u8>>()
        .expect("state")
        .expect("present")
        .request(2)
        .expect("request");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn on_exit_runs_once_for_a_pending_request_across_fixed_substeps() {
    static EXITS: AtomicU32 = AtomicU32::new(0);
    EXITS.store(0, Ordering::SeqCst);

    let fixed = FixedConfig::new(Duration::from_millis(1))
        .expect("fixed")
        .with_max_substeps(4)
        .expect("substeps");
    let mut builder = AppBuilder::new();
    builder.insert_state(1u8).fixed(fixed);
    builder
        .add_system(on_exit::<u8>("exit", stage::FIXED_UPDATE, |_world, _dt| {
            EXITS.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("exit");
    builder
        .add_system(apply::<u8>("apply", stage::UPDATE))
        .expect("apply");
    let mut app = builder.build().expect("build");

    app.world_mut()
        .resource_mut::<State<u8>>()
        .expect("state")
        .expect("present")
        .request(2)
        .expect("request");
    app.update(0.003).expect("first update");
    assert_eq!(EXITS.load(Ordering::SeqCst), 1);

    app.world_mut()
        .resource_mut::<State<u8>>()
        .expect("state")
        .expect("present")
        .request(1)
        .expect("second request");
    app.update(0.003).expect("second update");
    assert_eq!(EXITS.load(Ordering::SeqCst), 2);
}

#[test]
fn flush_mode_stage_makes_commands_visible_between_stages() {
    static SAW: AtomicU32 = AtomicU32::new(0);

    struct Spawned(moirai::EntityId);

    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Spawned>();
    builder
        .world_builder()
        .register_component::<u32>(moirai::component::ComponentOptions::sparse())
        .expect("register");
    builder
        .add_system(System::new("spawn", stage::STARTUP, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            let _ = world.commands().expect("commands").insert(entity, 1u32);
            world.insert_resource(Spawned(entity)).expect("track");
        }))
        .expect("spawn");
    builder
        .add_system(System::new("check", stage::UPDATE, |world, _dt| {
            if let Some(spawned) = world.resource::<Spawned>().expect("resource") {
                if world.is_alive(spawned.0) {
                    SAW.fetch_add(1, Ordering::SeqCst);
                }
            }
        }))
        .expect("check");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(SAW.load(Ordering::SeqCst), 1);
}

#[test]
fn explicit_stage_flush_makes_commands_visible_to_the_next_stage() {
    static SAW: AtomicU32 = AtomicU32::new(0);

    let spawned = Rc::new(Cell::new(None));
    let capture = Rc::clone(&spawned);
    let observe = Rc::clone(&spawned);
    let mut schedule_builder = ScheduleBuilder::new();
    schedule_builder
        .add_stage("First", StageOperation::Update)
        .expect("first stage");
    schedule_builder
        .add_stage("Second", StageOperation::Update)
        .expect("second stage");
    schedule_builder
        .set_stage_flush_mode("First", FlushMode::Stage)
        .expect("stage flush");
    schedule_builder
        .add_system(System::new("spawn", "First", move |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            capture.set(Some(entity));
        }))
        .expect("spawn system");
    schedule_builder
        .add_system(System::new("observe", "Second", move |world, _dt| {
            if observe.get().is_some_and(|entity| world.is_alive(entity)) {
                SAW.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .expect("observe system");
    let mut world = WorldBuilder::new().build().expect("world");
    let schedule = schedule_builder.build(&mut world).expect("schedule");
    let mut app = moirai::App::from_parts(world, schedule).expect("app");

    SAW.store(0, Ordering::SeqCst);
    app.update(0.0).expect("update");
    assert_eq!(SAW.load(Ordering::SeqCst), 1);
}

#[test]
fn flush_mode_final_defers_commands_until_update_end() {
    static MID: AtomicU32 = AtomicU32::new(0);

    let mut builder = ScheduleBuilder::new();
    builder
        .add_stage(stage::UPDATE, StageOperation::Update)
        .expect("stage");
    let mut world = WorldBuilder::new().build().expect("world");
    let mut schedule_builder = builder;
    schedule_builder
        .add_system(System::new("defer", stage::UPDATE, |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            let _ = world.commands().expect("commands").insert(entity, 1u32);
            if !world.is_alive(entity) {
                MID.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .expect("defer");
    let schedule = schedule_builder.build(&mut world).expect("schedule");
    let mut app = moirai::App::from_parts(world, schedule).expect("app");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(MID.load(Ordering::SeqCst), 1);
}

#[test]
fn fixed_update_respects_accumulator_and_cap() {
    static STEPS: AtomicU32 = AtomicU32::new(0);
    let fixed = FixedConfig::new(Duration::from_millis(16)).expect("fixed");

    let mut builder = AppBuilder::new();
    builder.fixed(fixed);
    builder
        .add_system(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {
            STEPS.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("fixed");
    let mut app = builder.build().expect("build");

    app.update(1.0).expect("update");
    assert_eq!(STEPS.load(Ordering::SeqCst), 8);
    assert_eq!(app.world().world_tick().raw(), 1);
}

#[test]
fn render_does_not_advance_world_tick() {
    let mut app = build_app(System::new("draw", stage::RENDER, |_world, _dt| {}));
    app.update(1.0 / 60.0).expect("update");
    let tick = app.world().world_tick().raw();
    app.render(1.0 / 60.0).expect("render");
    assert_eq!(app.world().world_tick().raw(), tick);
}

#[test]
fn render_rejects_structural_commands_in_system() {
    static REJECTED: AtomicU32 = AtomicU32::new(0);
    let mut app = build_app(System::new("draw", stage::RENDER, |world, _dt| {
        if world.commands().is_err() {
            REJECTED.fetch_add(1, Ordering::SeqCst);
        }
    }));
    app.render(1.0 / 60.0).expect("render");
    assert_eq!(REJECTED.load(Ordering::SeqCst), 1);
}

#[test]
fn frame_events_clear_per_operation_boundary() {
    #[derive(Clone, Debug, PartialEq)]
    struct UpdateFrameEvent(u8);
    #[derive(Clone, Debug, PartialEq)]
    struct RenderFrameEvent(u8);

    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<UpdateFrameEvent>(EventOptions::frame(StageOperation::Update))
        .expect("update event");
    builder
        .world_builder()
        .add_event::<RenderFrameEvent>(EventOptions::frame(StageOperation::Render))
        .expect("render event");
    builder
        .add_system(
            System::new("emit", stage::UPDATE, |world, _dt| {
                world.send(UpdateFrameEvent(1)).expect("send");
            })
            .emits::<UpdateFrameEvent>(),
        )
        .expect("emit");
    builder
        .add_system(
            System::new("draw", stage::RENDER, |world, _dt| {
                world.send(RenderFrameEvent(2)).expect("send");
            })
            .emits::<RenderFrameEvent>(),
        )
        .expect("draw");
    let mut app = builder.build().expect("build");

    app.world_mut().send(UpdateFrameEvent(9)).expect("prequeue");
    app.update(1.0 / 60.0).expect("update");

    let mut update_reader = app
        .world_mut()
        .event_reader::<UpdateFrameEvent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut update_reader)
        .expect("read")
        .is_none());

    app.world_mut()
        .send(RenderFrameEvent(8))
        .expect("prequeue render");
    app.render(1.0 / 60.0).expect("render");

    let mut render_reader = app
        .world_mut()
        .event_reader::<RenderFrameEvent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut render_reader)
        .expect("read")
        .is_none());
}

#[test]
fn update_with_observes_stable_world_after_flush() {
    static SAW_TICK: AtomicU32 = AtomicU32::new(0);

    let mut app = build_app(System::new("mark", stage::UPDATE, |_world, _dt| {
        SAW_TICK.store(1, Ordering::SeqCst);
    }));

    let tick = app
        .update_with(1.0 / 60.0, |world| world.world_tick())
        .expect("observe");
    assert_eq!(tick.raw(), 1);
    assert_eq!(SAW_TICK.load(Ordering::SeqCst), 1);
}

#[test]
fn missing_required_resource_is_rejected_at_build() {
    #[derive(Clone)]
    struct Needed(#[allow(dead_code)] u8);

    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    let result = builder.add_system(
        System::new("needs", stage::UPDATE, |_world, _dt| {}).requires_resource::<Needed>(),
    );
    assert!(matches!(result, Ok(())));
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::MissingRequiredResource { .. })
    ));
}

#[test]
fn fixed_update_without_config_is_rejected_at_build() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {}))
        .expect("authoring is call-order independent");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::FixedUpdateWithoutConfig)
    ));
}

#[test]
fn fixed_configuration_can_follow_fixed_system_authoring() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {}))
        .expect("fixed system");
    builder.fixed(FixedConfig::new(Duration::from_millis(16)).expect("config"));
    builder.build(&mut world).expect("schedule");
}

#[test]
fn explicit_flush_modes_are_effective_and_invalid_placements_are_rejected() {
    let mut builder = ScheduleBuilder::standard();
    builder
        .set_stage_flush_mode(stage::UPDATE, FlushMode::Final)
        .expect("final update flush");
    builder
        .set_stage_flush_mode(stage::UPDATE, FlushMode::Stage)
        .expect("stage update flush");
    assert!(matches!(
        builder.set_stage_flush_mode(stage::UPDATE, FlushMode::AfterSystem),
        Err(BuildError::InvalidStageFlushMode { .. })
    ));
    assert!(matches!(
        builder.set_stage_flush_mode(stage::RENDER, FlushMode::Stage),
        Err(BuildError::InvalidStageFlushMode { .. })
    ));
    assert!(matches!(
        builder.add_system(
            System::new("bad-stage", stage::UPDATE, |_world, _dt| {}).flush_mode(FlushMode::Stage)
        ),
        Err(BuildError::InvalidSystemFlushMode { .. })
    ));
    assert!(matches!(
        builder
            .add_system(System::new("bad-render", stage::RENDER, |_world, _dt| {}).flush_after()),
        Err(BuildError::InvalidSystemFlushMode { .. })
    ));
}

#[test]
fn duplicate_set_labels_are_rejected_at_build() {
    let set = SystemSet::new("sim");
    let mut builder = ScheduleBuilder::standard();
    builder.register_set(set.clone()).expect("first");
    assert!(matches!(
        builder.register_set(set),
        Err(BuildError::DuplicateSystemSet { .. })
    ));
}

#[test]
fn set_and_system_conditions_compose_with_and_semantics() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    let set = SystemSet::new("sim");

    let mut builder = AppBuilder::new();
    builder.register_set(set.clone()).expect("set");
    builder.set_run_if(&set, Condition::always()).expect("set");
    builder
        .add_system(
            System::new("gated", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .in_set(&set)
            .run_if(Condition::never()),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
}

#[test]
fn apply_state_system_errors_when_resource_missing() {
    #[derive(Clone, Eq, PartialEq)]
    struct Menu;

    let mut builder = AppBuilder::new();
    builder
        .add_system(apply::<Menu>("apply", stage::UPDATE))
        .expect("apply");
    assert!(matches!(
        builder.build(),
        Err(BuildError::MissingRequiredResource { .. })
    ));
}

#[test]
fn app_builder_seeds_state_before_schedule_validation_regardless_of_call_order() {
    #[derive(Clone, Eq, PartialEq)]
    struct Menu;

    let mut builder = AppBuilder::new();
    builder
        .add_system(apply::<Menu>("apply", stage::UPDATE))
        .expect("apply");
    builder.insert_state(Menu);
    let app = builder.build().expect("seed satisfies requirement");
    assert!(app.world().contains_resource::<State<Menu>>());
}

#[test]
fn app_builder_resource_seed_satisfies_requirements_and_has_one_initial_change() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    RAN.store(0, Ordering::SeqCst);
    #[derive(Debug, PartialEq)]
    struct SeededScore(u32);

    let mut builder = AppBuilder::new();
    builder
        .add_system(
            System::new("seeded", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .requires_resource::<SeededScore>()
            .run_if(Condition::resource_changed::<SeededScore>()),
        )
        .expect("system before seed");
    builder.insert_resource(SeededScore(1));
    builder.insert_resource(SeededScore(2));

    let mut app = builder.build().expect("seed satisfies requirement");
    assert_eq!(
        app.world().resource::<SeededScore>().expect("score"),
        Some(&SeededScore(2))
    );
    app.update(1.0 / 60.0).expect("first update");
    app.update(1.0 / 60.0).expect("second update");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn condition_debug_formats_as_opaque_label() {
    assert_eq!(format!("{:?}", Condition::always()), "Condition");
}

#[test]
fn set_system_enabled_skips_disabled_system() {
    static RAN: AtomicU32 = AtomicU32::new(0);

    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("work", stage::UPDATE, |_world, _dt| {
            RAN.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("add");
    let mut app = builder.build().expect("build");
    let id = app.schedule().system_id("work").expect("id");
    app.set_system_enabled(&id, false).expect("disable");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
}

#[test]
fn flush_after_system_makes_commands_visible_before_next_system() {
    static SAW: AtomicU32 = AtomicU32::new(0);

    struct Spawned(moirai::EntityId);

    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Spawned>();
    builder
        .world_builder()
        .register_component::<u32>(moirai::component::ComponentOptions::sparse())
        .expect("register");
    builder
        .add_system(
            System::new("spawn", stage::UPDATE, |world, _dt| {
                let entity = world.commands().expect("commands").spawn().expect("spawn");
                let _ = world.commands().expect("commands").insert(entity, 1u32);
                world.insert_resource(Spawned(entity)).expect("track");
            })
            .flush_after(),
        )
        .expect("spawn");
    builder
        .add_system(
            System::new("check", stage::UPDATE, |world, _dt| {
                if let Some(spawned) = world.resource::<Spawned>().expect("resource") {
                    if world.is_alive(spawned.0) {
                        SAW.fetch_add(1, Ordering::SeqCst);
                    }
                }
            })
            .after("spawn"),
        )
        .expect("check");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(SAW.load(Ordering::SeqCst), 1);
}

#[test]
fn flush_failure_faults_app_and_discards_batch() {
    struct Live(moirai::EntityId);

    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Live>();
    builder
        .add_system(
            System::new("bad", stage::UPDATE, |world, _dt| {
                let live = world.resource::<Live>().expect("resource").expect("live").0;
                world
                    .commands()
                    .expect("commands")
                    .despawn(live)
                    .expect("first");
                world
                    .commands()
                    .expect("commands")
                    .despawn(live)
                    .expect("duplicate");
            })
            .flush_after(),
        )
        .expect("bad");
    let mut app = builder.build().expect("build");
    let live = app.world_mut().spawn().expect("live");
    app.world_mut().insert_resource(Live(live)).expect("track");
    assert!(matches!(app.update(1.0 / 60.0), Err(AppError::Fault(_))));
    assert!(app.is_faulted());
    assert!(app.world().run_guard_is_idle());
}

#[test]
fn fixed_accumulator_carries_remainder_across_updates() {
    static STEPS: AtomicU32 = AtomicU32::new(0);
    let fixed = FixedConfig::new(Duration::from_millis(16)).expect("fixed");

    let mut builder = AppBuilder::new();
    builder.fixed(fixed);
    builder
        .add_system(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {
            STEPS.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("fixed");
    let mut app = builder.build().expect("build");

    STEPS.store(0, Ordering::SeqCst);
    app.update(0.05).expect("first");
    let first = STEPS.load(Ordering::SeqCst);
    assert!(first > 0 && first <= 8);

    STEPS.store(0, Ordering::SeqCst);
    app.update(0.05).expect("second");
    assert!(STEPS.load(Ordering::SeqCst) > 0);
}

#[test]
fn persistent_events_survive_update_until_read() {
    #[derive(Clone, Debug, PartialEq)]
    struct Persistent(u8);

    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<Persistent>(moirai::event::EventOptions::manual())
        .expect("event");
    builder
        .add_system(
            System::new("emit", stage::UPDATE, |world, _dt| {
                world.send(Persistent(1)).expect("send");
            })
            .emits::<Persistent>(),
        )
        .expect("emit");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");

    let mut reader = app
        .world_mut()
        .event_reader::<Persistent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert_eq!(
        app.world_mut()
            .read_event(&mut reader)
            .expect("read")
            .map(|event| event.0),
        Some(1)
    );
}

#[test]
fn custom_update_stage_runs_from_compiled_order() {
    static CUSTOM: AtomicU32 = AtomicU32::new(0);
    CUSTOM.store(0, Ordering::SeqCst);

    let mut builder = AppBuilder::new();
    builder
        .schedule_builder()
        .add_stage("PostStartup", StageOperation::Update)
        .expect("stage");
    builder
        .add_system(System::new("custom", "PostStartup", |_world, _dt| {
            CUSTOM.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(CUSTOM.load(Ordering::SeqCst), 1);
}

#[test]
fn resource_changed_condition_consumes_observation_across_updates() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    RAN.store(0, Ordering::SeqCst);

    #[derive(Clone)]
    struct Score(#[allow(dead_code)] u32);

    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Score>();
    builder
        .add_system(
            System::new("watch", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::resource_changed::<Score>()),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.world_mut().insert_resource(Score(1)).expect("insert");
    app.update(1.0 / 60.0).expect("first");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
    app.update(1.0 / 60.0).expect("second");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_build_does_not_lock_required_resources() {
    #[derive(Clone)]
    struct Needed(#[allow(dead_code)] u8);

    let mut world = WorldBuilder::new();
    world.register_resource::<Needed>();
    let mut world = world.build().expect("world");
    world.insert_resource(Needed(1)).expect("seed");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).before("b"))
        .expect("a");
    builder
        .add_system(System::new("b", stage::UPDATE, |_world, _dt| {}).before("a"))
        .expect("b");
    builder
        .add_system(
            System::new("needs", stage::UPDATE, |_world, _dt| {}).requires_resource::<Needed>(),
        )
        .expect("needs");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::Cycle { .. })
    ));
    assert!(world.remove_resource::<Needed>().expect("remove").is_some());
}

#[test]
fn dropped_schedule_releases_resource_locks() {
    #[derive(Clone)]
    struct Needed(#[allow(dead_code)] u8);

    let mut world = WorldBuilder::new();
    world.register_resource::<Needed>();
    let mut world = world.build().expect("world");
    world.insert_resource(Needed(1)).expect("seed");
    {
        let mut schedule_builder = ScheduleBuilder::standard();
        schedule_builder
            .add_system(
                System::new("needs", stage::UPDATE, |_world, _dt| {}).requires_resource::<Needed>(),
            )
            .expect("add");
        let schedule = schedule_builder.build(&mut world).expect("build");
        assert!(world.remove_resource::<Needed>().is_err());
        drop(schedule);
    }
    ScheduleBuilder::standard()
        .build(&mut world)
        .expect("rebuild prunes locks");
    assert!(world.remove_resource::<Needed>().expect("remove").is_some());
}

#[test]
fn panic_clears_running_and_faults_app() {
    let mut app = build_app(System::new("panic", stage::UPDATE, |world, _dt| {
        world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve entity");
        panic!("expected test panic");
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.update(1.0 / 60.0);
    }));
    assert!(result.is_err());
    assert!(app.is_faulted());
    assert!(app.world().run_guard_is_idle());
    assert!(!app.world().has_pending_commands());
    assert!(app.world().fixed_step().is_none());
    assert!(matches!(
        app.update(1.0 / 60.0),
        Err(AppError::TerminalFault)
    ));
    assert_eq!(
        app.fault().and_then(|fault| fault.detail.as_deref()),
        Some("panic during execution")
    );
}

#[test]
fn resource_exists_condition_gates_until_resource_present() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    RAN.store(0, Ordering::SeqCst);

    #[derive(Clone)]
    struct Flag;

    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Flag>();
    builder
        .add_system(
            System::new("needs_flag", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::resource_exists::<Flag>()),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("without resource");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
    app.world_mut().insert_resource(Flag).expect("insert");
    app.update(1.0 / 60.0).expect("with resource");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn condition_and_requires_both_true() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    RAN.store(0, Ordering::SeqCst);

    let mut builder = AppBuilder::new();
    builder
        .add_system(
            System::new("and_gate", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::always().and(Condition::never())),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
}

#[test]
fn resource_added_runs_once_after_insert() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    RAN.store(0, Ordering::SeqCst);

    #[derive(Clone)]
    struct Score(#[allow(dead_code)] u32);

    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Score>();
    builder
        .add_system(
            System::new("watch", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::resource_added::<Score>()),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("without resource");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
    app.world_mut().insert_resource(Score(1)).expect("insert");
    app.update(1.0 / 60.0).expect("after insert");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
    app.update(1.0 / 60.0).expect("stale");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn set_resource_added_gates_members() {
    static A: AtomicU32 = AtomicU32::new(0);
    static B: AtomicU32 = AtomicU32::new(0);
    A.store(0, Ordering::SeqCst);
    B.store(0, Ordering::SeqCst);

    #[derive(Clone)]
    struct Flag;

    let set = SystemSet::new("flagged");
    let mut builder = AppBuilder::new();
    builder.world_builder().register_resource::<Flag>();
    builder.register_set(set.clone()).expect("set");
    builder
        .set_run_if(&set, Condition::resource_added::<Flag>())
        .expect("cond");
    builder
        .add_system(
            System::new("a", stage::UPDATE, |_world, _dt| {
                A.fetch_add(1, Ordering::SeqCst);
            })
            .in_set(&set),
        )
        .expect("a");
    builder
        .add_system(
            System::new("b", stage::UPDATE, |_world, _dt| {
                B.fetch_add(1, Ordering::SeqCst);
            })
            .in_set(&set),
        )
        .expect("b");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("without resource");
    assert_eq!(A.load(Ordering::SeqCst), 0);
    assert_eq!(B.load(Ordering::SeqCst), 0);
    app.world_mut().insert_resource(Flag).expect("insert");
    app.update(1.0 / 60.0).expect("after insert");
    assert_eq!(A.load(Ordering::SeqCst), 1);
    assert_eq!(B.load(Ordering::SeqCst), 1);
}

#[test]
fn update_rejects_invalid_delta() {
    let mut app = build_app(System::new("noop", stage::UPDATE, |_world, _dt| {}));
    assert!(matches!(app.update(f32::NAN), Err(AppError::InvalidDelta)));
    assert!(matches!(app.update(-1.0), Err(AppError::InvalidDelta)));
    assert_eq!(app.world().world_tick().raw(), 0);
}

#[test]
fn from_parts_rejects_lease_mismatch() {
    let mut world_a = WorldBuilder::new().build().expect("a");
    let world_b = WorldBuilder::new().build().expect("b");
    let schedule = ScheduleBuilder::standard()
        .build(&mut world_a)
        .expect("schedule");
    assert!(matches!(
        moirai::App::from_parts(world_b, schedule),
        Err(BuildError::LeaseMismatch)
    ));
}

#[test]
fn app_builder_and_app_builder_entry_construct() {
    let _ = AppBuilder::default();
    let _ = moirai::App::builder();
}

#[test]
fn from_parts_rejects_pending_commands() {
    let mut world = WorldBuilder::new().build().expect("world");
    let schedule = ScheduleBuilder::standard()
        .build(&mut world)
        .expect("schedule");
    let _ = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    assert!(matches!(
        moirai::App::from_parts(world, schedule),
        Err(BuildError::PendingCommands)
    ));
}

#[test]
fn add_stage_same_label_and_operation_is_idempotent() {
    let mut builder = ScheduleBuilder::new();
    builder
        .add_stage(stage::UPDATE, StageOperation::Update)
        .expect("first");
    builder
        .add_stage(stage::UPDATE, StageOperation::Update)
        .expect("repeat");
}

#[test]
fn schedule_build_rejects_pending_commands_running_and_poisoned_world() {
    let mut world = WorldBuilder::new().build().expect("world");
    let _ = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    assert!(matches!(
        ScheduleBuilder::standard().build(&mut world),
        Err(BuildError::PendingCommands)
    ));
}

#[test]
fn build_rejects_unknown_after_edge_and_cross_stage_after() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).after("missing"))
        .expect("a");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::UnknownSystem { .. })
    ));

    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).after("startup"))
        .expect("a");
    builder
        .add_system(System::new("startup", stage::STARTUP, |_world, _dt| {}))
        .expect("startup");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::CrossStageSystemEdge { .. })
    ));

    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("leaf", stage::UPDATE, |_world, _dt| {}))
        .expect("leaf");
    builder
        .add_system(System::new("before", stage::UPDATE, |_world, _dt| {}).before("missing"))
        .expect("before");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::UnknownSystem { .. })
    ));
}

#[test]
fn add_system_rejects_duplicate_label() {
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("dup", stage::UPDATE, |_world, _dt| {}))
        .expect("first");
    assert!(matches!(
        builder.add_system(System::new("dup", stage::UPDATE, |_world, _dt| {})),
        Err(BuildError::DuplicateSystemLabel { .. })
    ));
}

#[test]
fn condition_or_runs_when_either_true() {
    static RAN: AtomicU32 = AtomicU32::new(0);
    RAN.store(0, Ordering::SeqCst);

    let mut builder = AppBuilder::new();
    builder
        .add_system(
            System::new("or_gate", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::never().or(Condition::always())),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 1);
}

#[test]
fn system_local_initializes_once_and_persists_across_runs() {
    let initializations = Rc::new(Cell::new(0_u32));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let init_counter = Rc::clone(&initializations);
    let observed_runs = Rc::clone(&observed);

    let mut builder = AppBuilder::new();
    builder
        .add_system(System::with_local(
            "local",
            stage::UPDATE,
            move |_context| {
                init_counter.set(init_counter.get() + 1);
                Ok(0_u32)
            },
            move |_world, _dt, local| {
                *local += 1;
                observed_runs.borrow_mut().push(*local);
                Ok(())
            },
        ))
        .expect("system");

    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("first");
    app.update(1.0 / 60.0).expect("second");

    assert_eq!(initializations.get(), 1);
    assert_eq!(&*observed.borrow(), &[1, 2]);
}

#[derive(Clone, Copy)]
struct LocalQueryPosition(i32);

#[test]
fn system_local_prepared_query_tracks_entities_across_runs() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_counts = Rc::clone(&observed);
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .register_component::<LocalQueryPosition>(ComponentOptions::sparse())
        .expect("component");
    builder
        .add_system(System::with_local(
            "local-query",
            stage::UPDATE,
            |context| {
                context
                    .prepare_query1::<LocalQueryPosition>(
                        QuerySpec::new(),
                        QueryPolicy::DeltaMembership,
                    )
                    .map_err(|error| format!("{error:?}"))
            },
            move |world, _dt, query| {
                let count = query
                    .iter(world, QueryWindow::All)
                    .map_err(|error| format!("{error:?}"))?
                    .map(|(_, position)| position.0)
                    .sum::<i32>();
                observed_counts.borrow_mut().push(count);
                if count == 0 {
                    let entity = world
                        .commands()
                        .map_err(|error| format!("{error:?}"))?
                        .spawn()
                        .map_err(|error| format!("{error:?}"))?;
                    world
                        .commands()
                        .map_err(|error| format!("{error:?}"))?
                        .insert(entity, LocalQueryPosition(1))
                        .map_err(|error| format!("{error:?}"))?;
                }
                Ok(())
            },
        ))
        .expect("system");

    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("first");
    app.update(1.0 / 60.0).expect("second");
    assert_eq!(&*observed.borrow(), &[0, 1]);
}

#[test]
fn failed_system_local_initialization_attaches_no_schedule_lease() {
    let mut world = WorldBuilder::new().build().expect("world");
    let mut failing = ScheduleBuilder::standard();
    failing
        .add_system(System::with_local(
            "bad-local",
            stage::UPDATE,
            |_context| Err::<(), _>(String::from("nope")),
            |_world, _dt, _local| Ok(()),
        ))
        .expect("add");

    assert!(matches!(
        failing.build(&mut world),
        Err(BuildError::SystemInitialization { system, detail })
            if system == "bad-local" && detail == "nope"
    ));

    let mut replacement = ScheduleBuilder::standard();
    replacement
        .add_system(System::new("replacement", stage::UPDATE, |_world, _dt| {}))
        .expect("replacement");
    replacement
        .build(&mut world)
        .expect("failed initialization must not leave a live lease");
}

#[test]
fn failed_system_local_initialization_drops_prior_locals_atomically() {
    struct LocalDrop(Rc<Cell<u32>>);

    impl Drop for LocalDrop {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let drops = Rc::new(Cell::new(0));
    let local_drops = Rc::clone(&drops);
    let mut world = WorldBuilder::new().build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::with_local(
            "initialized-first",
            stage::UPDATE,
            move |_| Ok(LocalDrop(local_drops)),
            |_world, _dt, _local| Ok(()),
        ))
        .expect("first");
    builder
        .add_system(System::with_local(
            "fails-second",
            stage::UPDATE,
            |_| Err::<(), _>(String::from("stop")),
            |_world, _dt, _local| Ok(()),
        ))
        .expect("second");

    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::SystemInitialization { system, detail })
            if system == "fails-second" && detail == "stop"
    ));
    assert_eq!(drops.get(), 1);
}

#[test]
fn fixed_step_mod_observes_zero_based_binary_phases() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_steps = Rc::clone(&observed);
    let mut builder = AppBuilder::new();
    builder
        .schedule_builder()
        .fixed(FixedConfig::new(Duration::from_millis(1)).expect("fixed"));
    builder
        .add_system(
            System::new("cadenced", stage::FIXED_UPDATE, move |world, _dt| {
                observed_steps
                    .borrow_mut()
                    .push(world.fixed_step().expect("fixed step").index);
            })
            .run_if(Condition::fixed_step_mod(4, 0).expect("cadence")),
        )
        .expect("system");

    let mut app = builder.build().expect("app");
    app.update(0.004).expect("first four steps");
    app.update(0.004).expect("second four steps");
    assert_eq!(&*observed.borrow(), &[0, 4]);
}
