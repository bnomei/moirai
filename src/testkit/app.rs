use alloc::vec::Vec;

use crate::app::{App, AppError};
use crate::world::World;

use super::config::ReplayConfig;
use super::error::ReplayRunError;
use super::record::{MetricSample, StepRecord};
use super::report::{ReplayFailure, ReplayReport};
use super::step::StepIndex;

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

        if record.step() != step {
            return Err(ReplayFailure::new(
                step,
                report,
                ReplayRunError::StepMismatch {
                    reported: record.step(),
                    expected: step,
                },
            ));
        }

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
}
