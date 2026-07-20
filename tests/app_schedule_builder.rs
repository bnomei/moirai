//! Caller-authored schedule templates via [`AppBuilder::with_schedule_builder`].

use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;
use std::cell::RefCell;
use std::rc::Rc;

use moirai::diagnostics::{DiagnosticEvent, Observer};
use moirai::event::{EventOptions, EventReaderStart};
use moirai::schedule::{stage, FlushMode, ScheduleBuilder, System};
use moirai::world::WorldBuilder;
use moirai::{AppBuilder, AppError, BuildError, FixedConfig, FixedStep, StageOperation};

fn authored_playable_schedule() -> ScheduleBuilder {
    let mut schedule = ScheduleBuilder::new();
    for (label, operation) in [
        (stage::STARTUP, StageOperation::Update),
        ("Input", StageOperation::Update),
        ("Sim", StageOperation::Update),
        ("Collision", StageOperation::Update),
        ("Damage", StageOperation::Update),
        (stage::FIXED_UPDATE, StageOperation::Update),
        ("RenderPrep", StageOperation::Update),
        (stage::RENDER, StageOperation::Render),
    ] {
        schedule.add_stage(label, operation).expect("stage");
    }
    for label in [
        stage::STARTUP,
        "Input",
        "Sim",
        "Collision",
        "Damage",
        stage::FIXED_UPDATE,
        "RenderPrep",
    ] {
        schedule
            .set_stage_flush_mode(label, FlushMode::Stage)
            .expect("stage flush");
    }
    schedule
        .set_stage_flush_mode(stage::RENDER, FlushMode::Final)
        .expect("render flush");
    schedule
}

fn push_trace(trace: &Rc<RefCell<Vec<&'static str>>>, label: &'static str) -> System {
    let trace = Rc::clone(trace);
    System::new(label, label, move |_world, _dt| {
        trace.borrow_mut().push(label);
    })
}

fn push_trace_on(
    trace: &Rc<RefCell<Vec<&'static str>>>,
    name: &'static str,
    stage: &'static str,
) -> System {
    let trace = Rc::clone(trace);
    System::new(name, stage, move |_world, _dt| {
        trace.borrow_mut().push(name);
    })
}

#[test]
fn standard_entry_points_keep_backward_compatible_template() {
    for mut app in [
        AppBuilder::new().build().expect("new"),
        AppBuilder::default().build().expect("default"),
        moirai::App::builder().build().expect("app::builder"),
    ] {
        let order = [
            stage::STARTUP,
            stage::FIXED_UPDATE,
            stage::UPDATE,
            stage::RENDER,
        ];
        for label in order {
            assert!(
                app.schedule().stage_id(label).is_some(),
                "missing standard stage {label}"
            );
        }
        assert!(app.schedule().stage_id("Input").is_none());
        app.update(0.0).expect("update");
        assert_eq!(app.world().world_tick().raw(), 1);
    }
}

#[test]
fn caller_authored_execution_order_startup_once_and_render_separate() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut schedule = authored_playable_schedule();
    schedule
        .add_system(push_trace_on(&trace, "startup", stage::STARTUP))
        .expect("startup");
    for label in ["Input", "Sim", "Collision", "Damage", "RenderPrep"] {
        schedule
            .add_system(push_trace(&trace, label))
            .expect("stage system");
    }
    schedule
        .add_system(push_trace_on(&trace, "fixed", stage::FIXED_UPDATE))
        .expect("fixed");
    schedule
        .add_system(push_trace_on(&trace, "draw", stage::RENDER))
        .expect("render");
    schedule.fixed(FixedConfig::new(Duration::from_millis(16)).expect("fixed"));

    let mut builder = AppBuilder::with_schedule_builder(schedule);
    // Retain full AppBuilder surface after with_schedule_builder.
    let _ = builder.schedule_builder();
    let _ = builder.world_builder();
    let mut app = builder.build().expect("app");

    app.update(0.016).expect("first update");
    assert_eq!(
        trace.borrow().as_slice(),
        [
            "startup",
            "Input",
            "Sim",
            "Collision",
            "Damage",
            "fixed",
            "RenderPrep"
        ]
    );
    assert_eq!(app.world().world_tick().raw(), 1);

    trace.borrow_mut().clear();
    app.render(0.016).expect("render");
    assert_eq!(trace.borrow().as_slice(), ["draw"]);
    assert_eq!(app.world().world_tick().raw(), 1);

    trace.borrow_mut().clear();
    app.update(0.016).expect("second update");
    assert_eq!(
        trace.borrow().as_slice(),
        ["Input", "Sim", "Collision", "Damage", "fixed", "RenderPrep"]
    );
    assert_eq!(app.world().world_tick().raw(), 2);
}

#[test]
fn caller_authored_system_order_inside_stage_is_compiled_order() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage("Sim", StageOperation::Update)
        .expect("sim");
    schedule
        .set_stage_flush_mode("Sim", FlushMode::Stage)
        .expect("flush");
    schedule
        .add_system(push_trace_on(&trace, "a", "Sim"))
        .expect("a");
    schedule
        .add_system(push_trace_on(&trace, "b", "Sim"))
        .expect("b");
    schedule
        .add_system(push_trace_on(&trace, "c", "Sim"))
        .expect("c");

    let mut app = AppBuilder::with_schedule_builder(schedule)
        .build()
        .expect("app");
    app.update(0.0).expect("update");
    assert_eq!(trace.borrow().as_slice(), ["a", "b", "c"]);
}

#[test]
fn caller_authored_fixed_timestep_accumulator_and_cap() {
    static STEPS: AtomicU32 = AtomicU32::new(0);
    let observed = Rc::new(RefCell::new(Vec::<FixedStep>::new()));
    let capture = Rc::clone(&observed);

    let fixed = FixedConfig::new(Duration::from_millis(16))
        .expect("fixed")
        .with_max_substeps(2)
        .expect("cap");

    let mut schedule = authored_playable_schedule();
    schedule.fixed(fixed);
    schedule
        .add_system(System::new(
            "fixed",
            stage::FIXED_UPDATE,
            move |world, _dt| {
                STEPS.fetch_add(1, Ordering::SeqCst);
                capture
                    .borrow_mut()
                    .push(world.fixed_step().expect("fixed identity"));
            },
        ))
        .expect("fixed system");

    let mut app = AppBuilder::with_schedule_builder(schedule)
        .build()
        .expect("app");

    STEPS.store(0, Ordering::SeqCst);
    app.update(0.008).expect("sub-step delta");
    assert_eq!(STEPS.load(Ordering::SeqCst), 0);
    assert!(observed.borrow().is_empty());
    assert_eq!(app.world().world_tick().raw(), 1);

    STEPS.store(0, Ordering::SeqCst);
    app.update(0.008).expect("second half-step");
    assert_eq!(STEPS.load(Ordering::SeqCst), 1);
    assert_eq!(observed.borrow().len(), 1);
    assert_eq!(observed.borrow()[0].index, 0);
    assert_eq!(observed.borrow()[0].steps, 1);
    assert_eq!(app.world().world_tick().raw(), 2);

    STEPS.store(0, Ordering::SeqCst);
    observed.borrow_mut().clear();
    app.update(0.032).expect("two-step delta");
    assert_eq!(STEPS.load(Ordering::SeqCst), 2);
    assert_eq!(observed.borrow().len(), 2);
    assert_eq!(observed.borrow()[0].index, 1);
    assert_eq!(observed.borrow()[1].index, 2);

    STEPS.store(0, Ordering::SeqCst);
    // 5 whole intervals with max_substeps=2 → exactly two fixed runs.
    app.update(0.080).expect("over-cap delta");
    assert_eq!(STEPS.load(Ordering::SeqCst), 2);
    assert!(app.world().fixed_step().is_none());
}

#[test]
fn caller_authored_stage_flush_and_final_flush_are_distinct() {
    static SAW_ALIVE: AtomicU32 = AtomicU32::new(0);
    static SAW_DEAD_MID_SYSTEM: AtomicU32 = AtomicU32::new(0);

    let spawned = Rc::new(RefCell::new(None));
    let capture = Rc::clone(&spawned);
    let observe = Rc::clone(&spawned);

    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage("First", StageOperation::Update)
        .expect("first");
    schedule
        .add_stage("Second", StageOperation::Update)
        .expect("second");
    schedule
        .set_stage_flush_mode("First", FlushMode::Stage)
        .expect("stage flush");
    // Second keeps default Final from ScheduleBuilder::new.

    schedule
        .add_system(System::new("spawn", "First", move |world, _dt| {
            let entity = world.commands().expect("commands").spawn().expect("spawn");
            *capture.borrow_mut() = Some(entity);
            // Entity is not alive until the stage flush after First.
            if !world.is_alive(entity) {
                SAW_DEAD_MID_SYSTEM.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .expect("spawn");
    schedule
        .add_system(System::new("observe", "Second", move |world, _dt| {
            if observe
                .borrow()
                .is_some_and(|entity| world.is_alive(entity))
            {
                SAW_ALIVE.fetch_add(1, Ordering::SeqCst);
            }
        }))
        .expect("observe");

    let mut app = AppBuilder::with_schedule_builder(schedule)
        .build()
        .expect("app");
    SAW_ALIVE.store(0, Ordering::SeqCst);
    SAW_DEAD_MID_SYSTEM.store(0, Ordering::SeqCst);
    app.update(0.0).expect("update");
    assert_eq!(SAW_DEAD_MID_SYSTEM.load(Ordering::SeqCst), 1);
    assert_eq!(SAW_ALIVE.load(Ordering::SeqCst), 1);
    assert_eq!(app.world().world_tick().raw(), 1);
    assert!(!app.world().has_pending_commands());
}

#[test]
fn caller_authored_update_frame_events_cross_stages_and_clear_once() {
    #[derive(Clone, Debug, PartialEq)]
    struct FrameEvt(u8);

    let saw = Rc::new(RefCell::new(Vec::<u8>::new()));
    let capture = Rc::clone(&saw);

    let mut schedule = authored_playable_schedule();
    schedule
        .add_system(
            System::new("emit", "Input", |world, _dt| {
                world.send(FrameEvt(7)).expect("send");
            })
            .emits::<FrameEvt>(),
        )
        .expect("emit");
    schedule
        .add_system(
            System::new("consume", "Damage", move |world, _dt| {
                let mut reader = world
                    .event_reader::<FrameEvt>(EventReaderStart::OldestRetained)
                    .expect("reader");
                while let Some(event) = world.read_event(&mut reader).expect("read") {
                    capture.borrow_mut().push(event.0);
                }
            })
            .consumes::<FrameEvt>(),
        )
        .expect("consume");

    let mut builder = AppBuilder::with_schedule_builder(schedule);
    builder
        .world_builder()
        .add_event::<FrameEvt>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut app = builder.build().expect("app");

    app.update(0.0).expect("update");
    assert_eq!(saw.borrow().as_slice(), &[7]);
    assert_eq!(app.world().world_tick().raw(), 1);

    // Frame events are cleaned exactly once after the complete Update operation.
    let mut reader = app
        .world_mut()
        .event_reader::<FrameEvt>(EventReaderStart::OldestRetained)
        .expect("post reader");
    assert!(app
        .world_mut()
        .read_event(&mut reader)
        .expect("read")
        .is_none());
}

#[test]
fn caller_authored_observer_receives_authored_stage_identities() {
    let stages = Rc::new(RefCell::new(Vec::<String>::new()));
    let systems = Rc::new(RefCell::new(Vec::<String>::new()));
    let flushes = Rc::new(RefCell::new(0u32));

    struct TraceObserver {
        stages: Rc<RefCell<Vec<String>>>,
        systems: Rc<RefCell<Vec<String>>>,
        flushes: Rc<RefCell<u32>>,
    }

    impl Observer for TraceObserver {
        fn observe(&mut self, event: DiagnosticEvent<'_>) {
            match event {
                DiagnosticEvent::StageStart { name } => {
                    self.stages.borrow_mut().push(name.to_string());
                }
                DiagnosticEvent::SystemStart { name } => {
                    self.systems.borrow_mut().push(name.to_string());
                }
                DiagnosticEvent::FlushComplete => {
                    *self.flushes.borrow_mut() += 1;
                }
                _ => {}
            }
        }
    }

    let mut schedule = authored_playable_schedule();
    schedule
        .add_system(System::new("input_sys", "Input", |_world, _dt| {}))
        .expect("input");
    schedule
        .add_system(System::new("sim_sys", "Sim", |_world, _dt| {}))
        .expect("sim");
    schedule
        .add_system(System::new("draw_sys", stage::RENDER, |_world, _dt| {}))
        .expect("draw");
    schedule.fixed(FixedConfig::new(Duration::from_millis(16)).expect("fixed"));
    schedule
        .add_system(System::new(
            "fixed_sys",
            stage::FIXED_UPDATE,
            |_world, _dt| {},
        ))
        .expect("fixed");

    let mut builder = AppBuilder::with_schedule_builder(schedule);
    builder.observer(TraceObserver {
        stages: Rc::clone(&stages),
        systems: Rc::clone(&systems),
        flushes: Rc::clone(&flushes),
    });
    let mut app = builder.build().expect("app");

    app.update(0.016).expect("update");
    assert_eq!(
        stages.borrow().as_slice(),
        [
            stage::STARTUP,
            "Input",
            "Sim",
            "Collision",
            "Damage",
            stage::FIXED_UPDATE,
            "RenderPrep",
        ]
    );
    assert_eq!(
        systems.borrow().as_slice(),
        ["input_sys", "sim_sys", "fixed_sys"]
    );
    // Stage flushes for each Update stage with FlushMode::Stage plus the
    // operation-final flush — distinct boundaries, not "one total flush".
    assert!(*flushes.borrow() >= 2);

    stages.borrow_mut().clear();
    systems.borrow_mut().clear();
    app.render(0.016).expect("render");
    assert_eq!(stages.borrow().as_slice(), [stage::RENDER]);
    assert_eq!(systems.borrow().as_slice(), ["draw_sys"]);
}

#[test]
fn caller_authored_empty_schedule_builds_and_updates_without_standard_repair() {
    let mut app = AppBuilder::with_schedule_builder(ScheduleBuilder::new())
        .build()
        .expect("empty");
    assert!(app.schedule().stage_id(stage::STARTUP).is_none());
    assert!(app.schedule().stage_id(stage::UPDATE).is_none());
    app.update(0.0).expect("empty update");
    assert_eq!(app.world().world_tick().raw(), 1);
}

#[test]
fn caller_authored_invalid_stage_flush_is_rejected() {
    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage(stage::RENDER, StageOperation::Render)
        .expect("render");
    assert!(matches!(
        schedule.set_stage_flush_mode(stage::RENDER, FlushMode::Stage),
        Err(BuildError::InvalidStageFlushMode { .. })
    ));
}

#[test]
fn caller_authored_fixed_without_fixed_update_is_rejected() {
    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage("Sim", StageOperation::Update)
        .expect("sim");
    schedule.fixed(FixedConfig::new(Duration::from_millis(16)).expect("fixed"));
    assert!(matches!(
        AppBuilder::with_schedule_builder(schedule).build(),
        Err(BuildError::FixedConfigWithoutFixedUpdate)
    ));
}

#[test]
fn caller_authored_lease_ownership_validation_remains_intact() {
    let mut world_a = WorldBuilder::new().build().expect("a");
    let world_b = WorldBuilder::new().build().expect("b");
    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage(stage::UPDATE, StageOperation::Update)
        .expect("update");
    let compiled = schedule.build(&mut world_a).expect("schedule");
    assert!(matches!(
        moirai::App::from_parts(world_b, compiled),
        Err(BuildError::LeaseMismatch)
    ));
}

#[test]
fn caller_authored_terminal_fault_remains_fail_closed() {
    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage("Sim", StageOperation::Update)
        .expect("sim");
    let mut builder = AppBuilder::with_schedule_builder(schedule);
    builder
        .add_system(System::try_new("fail", "Sim", |_world, _dt| {
            Err(String::from("boom"))
        }))
        .expect("system");
    let mut app = builder.build().expect("app");
    assert!(matches!(app.update(0.0), Err(AppError::Fault(_))));
    assert!(app.is_faulted());
    assert!(matches!(app.update(0.0), Err(AppError::TerminalFault)));
    assert!(matches!(app.render(0.0), Err(AppError::TerminalFault)));
}

#[test]
fn with_schedule_builder_public_path_compiles_through_app_builder_surface() {
    let mut schedule = ScheduleBuilder::new();
    schedule
        .add_stage("Input", StageOperation::Update)
        .expect("input");
    schedule
        .add_stage(stage::FIXED_UPDATE, StageOperation::Update)
        .expect("fixed");
    schedule
        .set_stage_flush_mode("Input", FlushMode::Stage)
        .expect("input flush");
    schedule
        .set_stage_flush_mode(stage::FIXED_UPDATE, FlushMode::Stage)
        .expect("fixed flush");
    schedule.fixed(FixedConfig::new(Duration::from_millis(16)).expect("fixed"));

    let mut builder = AppBuilder::with_schedule_builder(schedule);
    builder.insert_resource(1u8);
    builder
        .add_system(System::new("poll", "Input", |_world, _dt| {}))
        .expect("system");
    builder
        .set_stage_flush_mode("Input", FlushMode::Stage)
        .expect("still configurable");
    let mut app = builder.build().expect("app");
    app.update(0.016).expect("update");
    assert_eq!(app.world().world_tick().raw(), 1);
}
