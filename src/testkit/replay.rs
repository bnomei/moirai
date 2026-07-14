use super::config::ReplayConfig;
use super::driver::ReplayDriver;
use super::error::ReplayRunError;
use super::report::{ReplayFailure, ReplayReport};
use super::step::StepIndex;

/// Run a host-owned replay driver under a finite checked step policy.
///
/// Each successful driver step must return a [`StepRecord`](super::record::StepRecord) whose
/// [`step`](super::record::StepRecord::step) matches the checked step Moirai invoked. A mismatch
/// fails the replay with [`ReplayRunError::StepMismatch`] while retaining any partial report
/// captured before the failure.
#[allow(clippy::result_large_err)]
pub fn run_replay<S, D, F>(
    config: ReplayConfig,
    factory: F,
) -> Result<ReplayReport<S>, ReplayFailure<ReplayRunError<D::Error>, S>>
where
    S: Eq,
    D: ReplayDriver<Snapshot = S>,
    F: FnOnce(u64) -> D,
{
    let mut driver = factory(config.seed());
    run_replay_loop(config, &mut driver, StepIndex::FIRST)
}

fn run_replay_loop<S, D>(
    config: ReplayConfig,
    driver: &mut D,
    mut step: StepIndex,
) -> Result<ReplayReport<S>, ReplayFailure<ReplayRunError<D::Error>, S>>
where
    S: Eq,
    D: ReplayDriver<Snapshot = S>,
{
    let mut report = ReplayReport::new(config.clone());

    loop {
        let record = match driver.step(step) {
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
        step = match advance_replay_step(step) {
            Ok(next) => next,
            Err(source) => {
                return Err(ReplayFailure::new(step, report, source));
            }
        };
    }

    Ok(report)
}

fn advance_replay_step<E>(step: StepIndex) -> Result<StepIndex, ReplayRunError<E>> {
    step.next().map_err(|_| ReplayRunError::StepOverflow)
}

/// Compare two replay reports for exact snapshot equality.
pub fn reports_match<S: Eq>(left: &ReplayReport<S>, right: &ReplayReport<S>) -> bool {
    left.seed() == right.seed()
        && left.config() == right.config()
        && left.step_snapshots().len() == right.step_snapshots().len()
        && left
            .step_snapshots()
            .iter()
            .zip(right.step_snapshots())
            .all(|(left_step, right_step)| {
                left_step.step() == right_step.step()
                    && left_step.world_tick() == right_step.world_tick()
                    && left_step.snapshot() == right_step.snapshot()
                    && metrics_match(left_step.metrics(), right_step.metrics())
            })
}

fn metrics_match(
    left: &[super::record::MetricSample],
    right: &[super::record::MetricSample],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.key() == right.key() && left.value().to_bits() == right.value().to_bits()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    use crate::testkit::{CapturePolicy, ReplayConfig, ReplayDriver, StepRecord};

    struct CountingDriver {
        remaining: u32,
    }

    impl ReplayDriver for CountingDriver {
        type Snapshot = u32;
        type Error = ();

        fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error> {
            if self.remaining == 0 {
                return Err(());
            }
            self.remaining -= 1;
            Ok(StepRecord::new(step, None, step.raw(), Vec::new()))
        }
    }

    struct MismatchDriver;

    impl ReplayDriver for MismatchDriver {
        type Snapshot = u8;
        type Error = ();

        fn step(&mut self, step: StepIndex) -> Result<StepRecord<Self::Snapshot>, Self::Error> {
            Ok(StepRecord::new(
                StepIndex::from_raw(step.raw().wrapping_add(1)),
                None,
                1,
                Vec::new(),
            ))
        }
    }

    #[test]
    fn advance_replay_step_overflow_maps_step_overflow() {
        let step = StepIndex::from_raw(u32::MAX);
        assert!(matches!(
            advance_replay_step::<()>(step),
            Err(ReplayRunError::StepOverflow)
        ));
    }

    #[test]
    fn run_replay_returns_partial_report_on_step_overflow() {
        let config = ReplayConfig::new(7, 2, CapturePolicy::EveryStep).expect("config");
        let mut driver = CountingDriver { remaining: 2 };
        let failure = run_replay_loop(config, &mut driver, StepIndex::from_raw(u32::MAX))
            .expect_err("overflow");
        assert!(matches!(failure.source(), &ReplayRunError::StepOverflow));
        assert_eq!(failure.step().raw(), u32::MAX);
        assert_eq!(failure.partial_report().step_snapshots().len(), 1);
    }

    #[test]
    fn run_replay_returns_partial_report_when_driver_fails() {
        let config = ReplayConfig::new(3, 4, CapturePolicy::EveryStep).expect("config");
        let failure = run_replay(config, |_| CountingDriver { remaining: 1 }).expect_err("fail");
        assert!(matches!(failure.source(), &ReplayRunError::Source(())));
        assert_eq!(failure.partial_report().step_snapshots().len(), 1);
    }

    #[test]
    fn run_replay_rejects_mismatched_step_record() {
        let config = ReplayConfig::new(1, 2, CapturePolicy::EveryStep).expect("config");
        let failure = run_replay(config, |_| MismatchDriver).expect_err("mismatch");
        assert!(matches!(
            failure.source(),
            ReplayRunError::StepMismatch {
                reported,
                expected,
            } if reported.raw() == 1 && expected.raw() == 0
        ));
        assert_eq!(failure.partial_report().step_snapshots().len(), 0);
    }

    #[test]
    fn replay_report_snapshots_iterates_captured_snapshots() {
        let config = ReplayConfig::new(3, 2, CapturePolicy::EveryStep).expect("config");
        let report = run_replay(config, |_| CountingDriver { remaining: 2 }).expect("replay");
        let snapshots: alloc::vec::Vec<_> = report.snapshots().copied().collect();
        assert_eq!(snapshots, alloc::vec![0, 1]);
    }

    #[test]
    fn final_only_skips_intermediate_records() {
        let config = ReplayConfig::new(3, 3, CapturePolicy::FinalOnly).expect("config");
        let report = run_replay(config, |_| CountingDriver { remaining: 3 }).expect("replay");

        assert_eq!(report.step_snapshots().len(), 1);
        assert_eq!(
            report.snapshots().copied().collect::<Vec<_>>(),
            alloc::vec![2]
        );
    }
}
