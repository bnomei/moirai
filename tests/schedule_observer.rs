use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;
#[cfg(feature = "std")]
use std::cell::Cell;
#[cfg(feature = "std")]
use std::rc::Rc;

use moirai::diagnostics::{DiagnosticEvent, Observer};
use moirai::event::{EventOptions, EventReaderStart};
use moirai::schedule::{stage, System};
use moirai::FixedConfig;
use moirai::{AppBuilder, AppError, StageOperation};

struct CountingObserver {
    events: AtomicU32,
}

impl Observer for CountingObserver {
    fn observe(&mut self, event: DiagnosticEvent<'_>) {
        match event {
            DiagnosticEvent::UpdateStart { .. }
            | DiagnosticEvent::UpdateFinish
            | DiagnosticEvent::RenderStart { .. }
            | DiagnosticEvent::RenderFinish
            | DiagnosticEvent::StageStart { .. }
            | DiagnosticEvent::StageFinish { .. }
            | DiagnosticEvent::SystemStart { .. }
            | DiagnosticEvent::SystemFinish { .. }
            | DiagnosticEvent::FlushComplete => {
                self.events.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
    }
}

#[test]
fn observer_receives_stage_and_system_events() {
    let mut builder = AppBuilder::new();
    builder.observer(CountingObserver {
        events: AtomicU32::new(0),
    });
    builder
        .add_system(System::new("work", stage::UPDATE, |_world, _dt| {}))
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
}

#[test]
fn absent_observer_does_not_require_host_allocation() {
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("work", stage::UPDATE, |_world, _dt| {}))
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
}

static DROPPED_STEPS: AtomicU32 = AtomicU32::new(0);

struct DebtObserver;

impl Observer for DebtObserver {
    fn observe(&mut self, event: DiagnosticEvent<'_>) {
        if let DiagnosticEvent::FixedDebtDropped { steps } = event {
            DROPPED_STEPS.store(
                steps.try_into().expect("test debt fits u32"),
                Ordering::SeqCst,
            );
        }
    }
}

#[test]
fn observer_receives_render_events() {
    let observer = CountingObserver {
        events: AtomicU32::new(0),
    };
    let mut builder = AppBuilder::new();
    builder.observer(observer);
    builder
        .add_system(System::new(
            "draw",
            moirai::schedule::stage::RENDER,
            |_world, _dt| {},
        ))
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    let before = app.world().world_tick().raw();
    app.render(1.0 / 60.0).expect("render");
    assert_eq!(before, app.world().world_tick().raw());

    let observed = app
        .render_with(1.0 / 60.0, |world| world.world_tick().raw())
        .expect("render_with");
    assert_eq!(observed, before);
}

#[test]
fn observer_reports_fixed_debt_dropped() {
    DROPPED_STEPS.store(0, Ordering::SeqCst);
    let fixed = FixedConfig::new(Duration::from_millis(16)).expect("fixed");
    let mut builder = AppBuilder::new();
    builder.fixed(fixed);
    builder.observer(DebtObserver);
    builder
        .add_system(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {}))
        .expect("add");
    let mut app = builder.build().expect("build");
    app.update(1.0).expect("update");
    assert!(DROPPED_STEPS.load(Ordering::SeqCst) > 0);
}

#[derive(Clone, Debug, PartialEq)]
struct CallbackEvent(u8);

#[test]
fn update_callback_panic_faults_and_clears_update_frame_events() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<CallbackEvent>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    builder
        .add_system(
            System::new("emit", stage::UPDATE, |world, _dt| {
                world.send(CallbackEvent(1)).expect("send");
            })
            .emits::<CallbackEvent>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.update_with(1.0 / 60.0, |_world| {
            panic!("update callback panic");
        });
    }));

    assert!(result.is_err());
    assert!(app.is_faulted());
    assert!(app.world().run_guard_is_idle());
    assert!(!app.world().has_pending_commands());
    assert_eq!(
        app.fault().and_then(|fault| fault.detail.as_deref()),
        Some("panic during execution")
    );
    assert!(matches!(app.update(0.0), Err(AppError::TerminalFault)));
    let mut reader = app
        .world_mut()
        .event_reader::<CallbackEvent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut reader)
        .expect("read")
        .is_none());
}

#[test]
fn render_callback_panic_preserves_tick_and_clears_render_frame_events() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<CallbackEvent>(EventOptions::frame(StageOperation::Render))
        .expect("event");
    builder
        .add_system(
            System::new("draw", stage::RENDER, |world, _dt| {
                world.send(CallbackEvent(2)).expect("send");
            })
            .emits::<CallbackEvent>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    let tick = app.world().world_tick();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.render_with(1.0 / 60.0, |_world| {
            panic!("render callback panic");
        });
    }));

    assert!(result.is_err());
    assert!(app.is_faulted());
    assert_eq!(app.world().world_tick(), tick);
    assert!(app.world().run_guard_is_idle());
    let mut reader = app
        .world_mut()
        .event_reader::<CallbackEvent>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut reader)
        .expect("read")
        .is_none());
}

#[cfg(feature = "std")]
struct TerminalObserver {
    faults: Rc<Cell<u32>>,
    finishes: Rc<Cell<u32>>,
}

#[cfg(feature = "std")]
impl Observer for TerminalObserver {
    fn observe(&mut self, event: DiagnosticEvent<'_>) {
        match event {
            DiagnosticEvent::Fault { .. } => self.faults.set(self.faults.get() + 1),
            DiagnosticEvent::UpdateFinish | DiagnosticEvent::RenderFinish => {
                self.finishes.set(self.finishes.get() + 1);
            }
            _ => {}
        }
    }
}

#[test]
#[cfg(feature = "std")]
fn callback_panic_emits_one_terminal_fault_without_success_finish() {
    let faults = Rc::new(Cell::new(0));
    let finishes = Rc::new(Cell::new(0));
    let mut builder = AppBuilder::new();
    builder.observer(TerminalObserver {
        faults: Rc::clone(&faults),
        finishes: Rc::clone(&finishes),
    });
    let mut app = builder.build().expect("app");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = app.update_with(0.0, |_world| panic!("callback panic"));
    }));

    assert!(result.is_err());
    assert_eq!(faults.get(), 1);
    assert_eq!(finishes.get(), 0);
}
