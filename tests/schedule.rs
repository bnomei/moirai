use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;

use moirai::event::{EventOptions, EventReaderStart};
#[cfg(feature = "testkit")]
use moirai::schedule::FlushMode;
use moirai::schedule::{stage, Condition, ScheduleBuilder, System, SystemSet};
use moirai::state::{apply, State};
use moirai::world::WorldBuilder;
use moirai::FixedConfig;
use moirai::StageOperation;
use moirai::{AppBuilder, AppError, BuildError};

static UPDATE_COUNT: AtomicU32 = AtomicU32::new(0);

fn build_app(system: System) -> moirai::App {
    let mut builder = AppBuilder::new();
    builder.add_system(system).expect("add");
    builder.build().expect("build")
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
fn unknown_system_set_is_rejected_at_build() {
    let world = WorldBuilder::new().build().expect("world");
    let set = SystemSet::new("missing");
    let result = ScheduleBuilder::standard()
        .add_system(System::new("a", stage::UPDATE, |_world, _dt| {}).in_set(&set));
    assert!(matches!(result, Err(BuildError::UnknownSystemSet { .. })));
    let _ = world;
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
    builder.world_builder().register_state::<u8>();
    builder
        .add_system(
            System::new("gate", stage::UPDATE, |_world, _dt| {
                RAN.fetch_add(1, Ordering::SeqCst);
            })
            .run_if(Condition::in_state(2u8)),
        )
        .expect("add");
    let mut app = builder.build().expect("build");
    app.world_mut()
        .insert_resource(State::new(1u8))
        .expect("state");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(RAN.load(Ordering::SeqCst), 0);
}

#[test]
fn state_changed_runs_after_explicit_apply() {
    static RAN: AtomicU32 = AtomicU32::new(0);

    let mut builder = AppBuilder::new();
    builder.world_builder().register_state::<u8>();
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
        .insert_resource(State::new(1u8))
        .expect("state");
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
        .add_system(System::new("emit", stage::UPDATE, |world, _dt| {
            world.send(UpdateFrameEvent(1)).expect("send");
        }))
        .expect("emit");
    builder
        .add_system(System::new("draw", stage::RENDER, |world, _dt| {
            world.send(RenderFrameEvent(2)).expect("send");
        }))
        .expect("draw");
    let mut app = builder.build().expect("build");

    app.world_mut().send(UpdateFrameEvent(9)).expect("prequeue");
    app.update(1.0 / 60.0).expect("update");

    let mut update_reader = app
        .world_mut()
        .event_reader::<UpdateFrameEvent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world()
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
        .world()
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
    let world = WorldBuilder::new().build().expect("world");
    let result = ScheduleBuilder::standard().add_system(System::new(
        "fixed",
        stage::FIXED_UPDATE,
        |_world, _dt| {},
    ));
    assert!(matches!(result, Err(BuildError::FixedUpdateWithoutConfig)));
    let _ = world;
}

#[test]
#[cfg(feature = "testkit")]
fn standard_builder_defaults_stage_flush_mode() {
    let mut world = WorldBuilder::new().build().expect("world");
    let schedule = ScheduleBuilder::standard()
        .build(&mut world)
        .expect("build");
    assert_eq!(
        schedule.stage_flush_mode_for_test(stage::UPDATE),
        Some(FlushMode::Stage)
    );
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
        .add_system(System::new("emit", stage::UPDATE, |world, _dt| {
            world.send(Persistent(1)).expect("send");
        }))
        .expect("emit");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");

    let mut reader = app
        .world_mut()
        .event_reader::<Persistent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert_eq!(
        app.world()
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
#[cfg(feature = "std")]
fn panic_clears_running_and_faults_app() {
    let mut app = build_app(System::new("panic", stage::UPDATE, |_world, _dt| {
        panic!("expected test panic");
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.update(1.0 / 60.0);
    }));
    assert!(result.is_err());
    assert!(app.is_faulted());
    assert!(app.world().run_guard_is_idle());
    assert!(matches!(
        app.update(1.0 / 60.0),
        Err(AppError::TerminalFault)
    ));
}
