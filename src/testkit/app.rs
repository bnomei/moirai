use alloc::vec::Vec;

use crate::app::{App, AppError};
use crate::world::World;

use super::config::ReplayConfig;
use super::error::ReplayRunError;
use super::record::{MetricSample, StepRecord};
use super::report::{ReplayFailure, ReplayReport};
use super::step::StepIndex;

type RecordValidation<S> =
    Result<(ReplayReport<S>, StepRecord<S>), ReplayFailure<ReplayRunError<AppError>, S>>;

/// Capture replay evidence through `App::update_with` after final flush and before frame clearing.
#[allow(clippy::result_large_err)]
pub fn replay_app<S>(
    app: &mut App,
    config: ReplayConfig,
    delta_seconds: f32,
    snapshot: impl Fn(&World) -> S,
    metrics: impl Fn(&World) -> Vec<MetricSample>,
) -> Result<ReplayReport<S>, ReplayFailure<ReplayRunError<AppError>, S>>
where
    S: Eq,
{
    replay_app_loop(
        app,
        config,
        delta_seconds,
        &snapshot,
        &metrics,
        StepIndex::FIRST,
    )
}

#[allow(clippy::result_large_err)]
fn replay_app_loop<S>(
    app: &mut App,
    config: ReplayConfig,
    delta_seconds: f32,
    snapshot: &impl Fn(&World) -> S,
    metrics: &impl Fn(&World) -> Vec<MetricSample>,
    mut step: StepIndex,
) -> Result<ReplayReport<S>, ReplayFailure<ReplayRunError<AppError>, S>>
where
    S: Eq,
{
    let mut report = ReplayReport::new(config.clone());

    loop {
        let record = match capture_app_step(app, step, delta_seconds, snapshot, metrics) {
            Ok(record) => record,
            Err(source) => {
                return Err(ReplayFailure::new(
                    step,
                    report,
                    ReplayRunError::Source(source),
                ));
            }
        };

        let (next_report, record) = validate_record_step(step, report, record)?;
        report = next_report;

        let is_last = step.raw().wrapping_add(1) >= config.steps();
        if config.should_capture(is_last) {
            report.push_record(record);
        }

        if is_last {
            break;
        }
        step = match step.next() {
            Ok(next) => next,
            Err(_) => {
                return Err(ReplayFailure::new(
                    step,
                    report,
                    ReplayRunError::StepOverflow,
                ));
            }
        };
    }

    Ok(report)
}

#[allow(clippy::result_large_err)]
fn validate_record_step<S>(
    expected: StepIndex,
    report: ReplayReport<S>,
    record: StepRecord<S>,
) -> RecordValidation<S>
where
    S: Eq,
{
    if record.step() == expected {
        Ok((report, record))
    } else {
        Err(ReplayFailure::new(
            expected,
            report,
            ReplayRunError::StepMismatch {
                reported: record.step(),
                expected,
            },
        ))
    }
}

fn capture_app_step<S>(
    app: &mut App,
    step: StepIndex,
    delta_seconds: f32,
    snapshot: &impl Fn(&World) -> S,
    metrics: &impl Fn(&World) -> Vec<MetricSample>,
) -> Result<StepRecord<S>, AppError> {
    app.update_with(delta_seconds, |world| {
        StepRecord::new(
            step,
            Some(world.world_tick()),
            snapshot(world),
            metrics(world),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppBuilder;
    use crate::schedule::{stage, System};
    use crate::testkit::{CapturePolicy, ReplayConfig, StepIndex};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Tick(u64);

    #[test]
    fn replay_app_returns_partial_report_on_step_overflow() {
        let mut builder = AppBuilder::new();
        builder
            .add_system(System::new("tick", stage::UPDATE, |_world, _dt| {}))
            .expect("system");
        let mut app = builder.build().expect("app");
        let config = ReplayConfig::new(2, 2, CapturePolicy::EveryStep).expect("config");
        let failure = replay_app_loop(
            &mut app,
            config,
            1.0,
            &|world| Tick(world.world_tick().raw()),
            &|_| Vec::new(),
            StepIndex::from_raw_for_test(u32::MAX),
        )
        .expect_err("overflow");
        assert!(matches!(failure.source(), &ReplayRunError::StepOverflow));
        assert_eq!(failure.step().raw(), u32::MAX);
        assert_eq!(failure.partial_report().step_snapshots().len(), 1);
    }

    #[test]
    fn record_step_validation_reports_mismatch_with_partial_report() {
        let config = ReplayConfig::new(2, 1, CapturePolicy::EveryStep).expect("config");
        let report = ReplayReport::new(config);
        let expected = StepIndex::FIRST;
        let reported = StepIndex::from_raw_for_test(1);
        let record = StepRecord::new(reported, None, Tick(0), Vec::new());
        let failure = validate_record_step(expected, report, record).expect_err("mismatch");
        assert!(matches!(
            failure.source(),
            ReplayRunError::StepMismatch {
                reported: actual,
                expected: wanted,
            } if *actual == reported && *wanted == expected
        ));
    }
}
