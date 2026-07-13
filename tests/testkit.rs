#![cfg(feature = "testkit")]

use core::sync::atomic::{AtomicU32, Ordering};

use moirai::schedule::{stage, System};
use moirai::testkit::{
    replay_app, reports_match, run_replay, CapturePolicy, MetricSample, ReplayConfig,
    ReplayConfigError, ReplayDriver, ReplayRunError, StepIndex, StepRecord,
};
use moirai::AppBuilder;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TickSnapshot(u64);

static UPDATE_COUNT: AtomicU32 = AtomicU32::new(0);

fn counter_app() -> moirai::App {
    UPDATE_COUNT.store(0, Ordering::SeqCst);
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("count", stage::UPDATE, |_world, _dt| {
            UPDATE_COUNT.fetch_add(1, Ordering::SeqCst);
        }))
        .expect("system");
    builder.build().expect("app")
}

#[test]
fn replay_app_captures_post_flush_snapshots() {
    let mut app = counter_app();
    let config = ReplayConfig::new(11, 3, CapturePolicy::EveryStep).expect("config");
    let report = replay_app(
        &mut app,
        config,
        1.0 / 60.0,
        |world| TickSnapshot(world.world_tick().raw()),
        |_world| Vec::<MetricSample>::new(),
    )
    .expect("replay");

    assert_eq!(report.step_snapshots().len(), 3);
    assert_eq!(report.step_snapshots()[0].step(), StepIndex::FIRST);
    assert_eq!(report.step_snapshots()[0].snapshot(), &TickSnapshot(1));
    assert_eq!(report.step_snapshots()[2].snapshot(), &TickSnapshot(3));
}

#[test]
fn replay_app_is_deterministic_for_same_seed_and_config() {
    let config = ReplayConfig::new(99, 4, CapturePolicy::EveryStep).expect("config");

    let first = {
        let mut app = counter_app();
        replay_app(
            &mut app,
            config.clone(),
            1.0 / 60.0,
            |world| TickSnapshot(world.world_tick().raw()),
            |_world| Vec::new(),
        )
        .expect("first")
    };

    let second = {
        let mut app = counter_app();
        replay_app(
            &mut app,
            config,
            1.0 / 60.0,
            |world| TickSnapshot(world.world_tick().raw()),
            |_world| Vec::new(),
        )
        .expect("second")
    };

    assert!(reports_match(&first, &second));
}

#[test]
fn replay_app_final_only_capture_policy() {
    let mut app = counter_app();
    let config = ReplayConfig::new(1, 3, CapturePolicy::FinalOnly).expect("config");
    let report = replay_app(
        &mut app,
        config,
        1.0 / 60.0,
        |world| TickSnapshot(world.world_tick().raw()),
        |_world| Vec::new(),
    )
    .expect("replay");

    assert_eq!(report.step_snapshots().len(), 1);
    assert_eq!(report.step_snapshots()[0].snapshot(), &TickSnapshot(3));
}

struct SeedDriver {
    seed: u64,
}

impl ReplayDriver for SeedDriver {
    type Snapshot = u64;
    type Error = ();

    fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error> {
        Ok(StepRecord::new(
            step,
            None,
            self.seed.wrapping_add(step.raw() as u64),
            Vec::new(),
        ))
    }
}

#[test]
fn run_replay_retains_partial_report_on_failure() {
    struct FailingDriver;

    impl ReplayDriver for FailingDriver {
        type Snapshot = u8;
        type Error = &'static str;

        fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error> {
            if step.raw() == 1 {
                return Err("boom");
            }
            Ok(StepRecord::new(step, None, 1, Vec::new()))
        }
    }

    let config = ReplayConfig::new(5, 3, CapturePolicy::EveryStep).expect("config");
    let failure = run_replay(config, |_| FailingDriver).expect_err("failure");
    assert_eq!(failure.step().raw(), 1);
    assert!(matches!(failure.source(), &ReplayRunError::Source("boom")));
    assert_eq!(failure.partial_report().step_snapshots().len(), 1);
}

#[test]
fn replay_config_zero_steps_rejected() {
    assert!(matches!(
        ReplayConfig::new(1, 0, CapturePolicy::EveryStep),
        Err(ReplayConfigError::ZeroSteps)
    ));
}

#[test]
fn reports_match_detects_snapshot_divergence() {
    let config = ReplayConfig::new(7, 2, CapturePolicy::EveryStep).expect("config");
    let first = {
        let mut app = counter_app();
        replay_app(
            &mut app,
            config.clone(),
            1.0 / 60.0,
            |world| TickSnapshot(world.world_tick().raw()),
            |_world| Vec::new(),
        )
        .expect("first")
    };
    let second = {
        let mut app = counter_app();
        replay_app(
            &mut app,
            config,
            1.0 / 60.0,
            |world| TickSnapshot(world.world_tick().raw() + 99),
            |_world| Vec::new(),
        )
        .expect("second")
    };
    assert!(!reports_match(&first, &second));
}

#[test]
fn reports_match_treats_identical_nan_metric_bits_as_equal() {
    let config = ReplayConfig::new(7, 1, CapturePolicy::EveryStep).expect("config");
    let first = run_replay(config.clone(), |_| SeedDriver { seed: 1 }).expect("first");
    let second = run_replay(config, |_| SeedDriver { seed: 1 }).expect("second");
    assert!(reports_match(&first, &second));

    let config = ReplayConfig::new(7, 1, CapturePolicy::EveryStep).expect("config");
    let first = replay_app(
        &mut counter_app(),
        config.clone(),
        1.0 / 60.0,
        |world| TickSnapshot(world.world_tick().raw()),
        |_| vec![MetricSample::new("nan", f64::NAN)],
    )
    .expect("first nan");
    let second = replay_app(
        &mut counter_app(),
        config,
        1.0 / 60.0,
        |world| TickSnapshot(world.world_tick().raw()),
        |_| vec![MetricSample::new("nan", f64::NAN)],
    )
    .expect("second nan");
    assert!(reports_match(&first, &second));
}

#[test]
fn run_replay_passes_seed_to_factory_contract() {
    let config = ReplayConfig::new(42, 2, CapturePolicy::EveryStep).expect("config");
    let report = run_replay(config, |seed| SeedDriver { seed }).expect("replay");
    assert_eq!(report.step_snapshots()[0].snapshot(), &42);
    assert_eq!(report.step_snapshots()[1].snapshot(), &43);
}

#[test]
fn replay_app_returns_partial_report_when_update_fails() {
    use moirai::AppError;

    let mut app = counter_app();
    app.world_mut().set_world_tick_for_test(u64::MAX);
    let config = ReplayConfig::new(1, 2, CapturePolicy::EveryStep).expect("config");
    let failure = replay_app(
        &mut app,
        config,
        1.0 / 60.0,
        |world| TickSnapshot(world.world_tick().raw()),
        |_world| Vec::new(),
    )
    .expect_err("tick exhaustion");
    assert!(matches!(
        failure.source(),
        &ReplayRunError::Source(AppError::WorldTickExhausted)
    ));
    assert_eq!(failure.step(), StepIndex::FIRST);
}

#[test]
fn replay_config_error_display_reports_zero_steps() {
    assert_eq!(
        format!("{}", ReplayConfigError::ZeroSteps),
        "replay config requires a non-zero step count"
    );
}

#[test]
fn run_replay_rejects_mismatched_driver_step_index() {
    struct MismatchDriver;

    impl ReplayDriver for MismatchDriver {
        type Snapshot = u8;
        type Error = ();

        fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error> {
            Ok(StepRecord::new(
                StepIndex::from_raw(step.raw().wrapping_add(9)),
                None,
                1,
                Vec::new(),
            ))
        }
    }

    let config = ReplayConfig::new(1, 1, CapturePolicy::EveryStep).expect("config");
    let failure = run_replay(config, |_| MismatchDriver).expect_err("mismatch");
    assert!(matches!(
        failure.source(),
        ReplayRunError::StepMismatch { .. }
    ));
}

#[test]
fn replay_report_accessors_expose_seed_config_and_snapshots() {
    let config = ReplayConfig::new(55, 2, CapturePolicy::EveryStep).expect("config");
    let report = run_replay(config.clone(), |seed| SeedDriver { seed }).expect("replay");
    assert_eq!(report.seed(), 55);
    assert_eq!(report.config(), &config);
    let snapshots: Vec<_> = report.snapshots().collect();
    assert_eq!(snapshots, vec![&55, &56]);
}
