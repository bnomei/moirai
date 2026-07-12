use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;

use moirai::diagnostics::{DiagnosticEvent, Observer};
use moirai::schedule::{stage, System};
use moirai::AppBuilder;
use moirai::FixedConfig;

struct CountingObserver {
    events: AtomicU32,
}

impl Observer for CountingObserver {
    fn observe(&mut self, event: DiagnosticEvent<'_>) {
        match event {
            DiagnosticEvent::UpdateStart { .. }
            | DiagnosticEvent::UpdateFinish
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
