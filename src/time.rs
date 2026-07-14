use core::fmt;
use core::time::Duration;

/// Monotonic frame counter advanced only by `App`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct WorldTick(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorldTickError {
    Exhausted,
}

/// Fixed simulation substep identity while FixedUpdate runs.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FixedStep {
    /// Index of the first fixed interval represented by this run.
    pub index: u64,
    /// Simulation time represented by this run.
    pub delta: Duration,
    /// Number of fixed intervals represented by this run.
    ///
    /// This is one for ordinary fixed updates. A coalesced update represents
    /// more than one interval and advances the next index by this amount.
    pub steps: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum FixedStepError {
    Exhausted,
}

/// Host-provided fixed timestep configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FixedConfig {
    delta: Duration,
    max_substeps: u32,
    debt_policy: FixedDebtPolicy,
}

/// Policy used when a frame contains more fixed intervals than the cap.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FixedDebtPolicy {
    /// Run up to the cap, discard the remaining whole intervals, and report it.
    DropWithDiagnostic,
    /// Run up to the cap and retain the remaining intervals for future updates.
    Preserve,
    /// Run one update with all overdue whole intervals combined into its delta.
    Coalesce,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FixedConfigError {
    NonPositiveDelta,
    ZeroSubstepCap,
}

/// Debt-preserving fixed-step accumulator owned by `Schedule`.
#[derive(Clone, Debug)]
pub(crate) struct FixedAccumulator {
    remainder: Duration,
    next_index: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FixedDebtDropped {
    pub steps: u128,
}

/// Whole fixed intervals represented by one coalesced FixedUpdate run.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FixedDebtCoalesced {
    pub steps: u128,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum FixedWork {
    Steps(u32),
    Coalesced { steps: u128, delta: Duration },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct FixedPlan {
    pub work: FixedWork,
    pub dropped: Option<FixedDebtDropped>,
    pub coalesced: Option<FixedDebtCoalesced>,
}

/// Monotonic world change counter for component/resource mutation metadata.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct ChangeTick(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangeTickError {
    Exhausted,
}

impl WorldTick {
    pub const ZERO: Self = Self(0);

    pub fn raw(self) -> u64 {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) fn set_raw(&mut self, raw: u64) {
        self.0 = raw;
    }

    pub(crate) fn advance(&mut self) -> Result<Self, WorldTickError> {
        self.0 = self.0.checked_add(1).ok_or(WorldTickError::Exhausted)?;
        Ok(Self(self.0))
    }
}

impl FixedConfig {
    pub const DEFAULT_MAX_SUBSTEPS: u32 = 8;

    pub fn new(delta: Duration) -> Result<Self, FixedConfigError> {
        if delta.as_nanos() == 0 {
            return Err(FixedConfigError::NonPositiveDelta);
        }
        Ok(Self {
            delta,
            max_substeps: Self::DEFAULT_MAX_SUBSTEPS,
            debt_policy: FixedDebtPolicy::DropWithDiagnostic,
        })
    }

    pub fn with_max_substeps(mut self, max_substeps: u32) -> Result<Self, FixedConfigError> {
        if max_substeps == 0 {
            return Err(FixedConfigError::ZeroSubstepCap);
        }
        self.max_substeps = max_substeps;
        Ok(self)
    }

    pub fn delta(&self) -> Duration {
        self.delta
    }

    pub fn max_substeps(&self) -> u32 {
        self.max_substeps
    }

    pub fn with_debt_policy(mut self, debt_policy: FixedDebtPolicy) -> Self {
        self.debt_policy = debt_policy;
        self
    }

    pub fn debt_policy(&self) -> FixedDebtPolicy {
        self.debt_policy
    }
}

impl FixedAccumulator {
    pub fn new() -> Self {
        Self {
            remainder: Duration::ZERO,
            next_index: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_next_index_for_test(&mut self, next_index: u64) {
        self.next_index = next_index;
    }

    pub fn peek_plan(&self, frame_delta: Duration, config: &FixedConfig) -> FixedPlan {
        Self::substep_plan(self.remainder.saturating_add(frame_delta), config).0
    }

    pub fn plan(&mut self, frame_delta: Duration, config: &FixedConfig) -> FixedPlan {
        let total = self.remainder.saturating_add(frame_delta);
        let (plan, remainder) = Self::substep_plan(total, config);
        self.remainder = remainder;
        plan
    }

    pub fn preflight_steps(&self, steps: u128) -> Result<(), FixedStepError> {
        if steps == 0 {
            return Ok(());
        }
        let steps = u64::try_from(steps).map_err(|_| FixedStepError::Exhausted)?;
        let last = self
            .next_index
            .checked_add(steps - 1)
            .ok_or(FixedStepError::Exhausted)?;
        last.checked_add(1).ok_or(FixedStepError::Exhausted)?;
        Ok(())
    }

    fn substep_plan(total: Duration, config: &FixedConfig) -> (FixedPlan, Duration) {
        let delta_nanos = config.delta().as_nanos();
        let total_nanos = total.as_nanos();
        let due = total_nanos / delta_nanos;
        let run = due.min(config.max_substeps() as u128) as u32;
        let ordinary = FixedPlan {
            work: FixedWork::Steps(run),
            dropped: None,
            coalesced: None,
        };
        if due <= config.max_substeps() as u128 {
            return (ordinary, duration_from_nanos(total_nanos % delta_nanos));
        }

        match config.debt_policy() {
            FixedDebtPolicy::DropWithDiagnostic => (
                FixedPlan {
                    dropped: Some(FixedDebtDropped {
                        steps: due - run as u128,
                    }),
                    ..ordinary
                },
                duration_from_nanos(total_nanos % delta_nanos),
            ),
            FixedDebtPolicy::Preserve => {
                let consumed = delta_nanos.saturating_mul(run as u128);
                (ordinary, duration_from_nanos(total_nanos - consumed))
            }
            FixedDebtPolicy::Coalesce => {
                let delta = duration_from_nanos(delta_nanos.saturating_mul(due));
                (
                    FixedPlan {
                        work: FixedWork::Coalesced { steps: due, delta },
                        dropped: None,
                        coalesced: Some(FixedDebtCoalesced { steps: due }),
                    },
                    duration_from_nanos(total_nanos % delta_nanos),
                )
            }
        }
    }

    pub fn next_step(&mut self, config: &FixedConfig) -> FixedStep {
        let step = FixedStep {
            index: self.next_index,
            delta: config.delta(),
            steps: 1,
        };
        self.next_index = self.next_index.saturating_add(step.steps);
        step
    }

    pub fn next_coalesced(&mut self, steps: u64, delta: Duration) -> FixedStep {
        let step = FixedStep {
            index: self.next_index,
            delta,
            steps,
        };
        self.next_index = self.next_index.saturating_add(steps);
        step
    }
}

fn duration_from_nanos(nanos: u128) -> Duration {
    const NANOS_PER_SECOND: u128 = 1_000_000_000;
    Duration::new(
        (nanos / NANOS_PER_SECOND) as u64,
        (nanos % NANOS_PER_SECOND) as u32,
    )
}

impl Default for FixedAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WorldTick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ChangeTick {
    pub const ZERO: Self = Self(0);

    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }

    pub(crate) fn advance(&mut self) -> Result<Self, ChangeTickError> {
        self.0 = self.0.checked_add(1).ok_or(ChangeTickError::Exhausted)?;
        Ok(Self(self.0))
    }

    pub(crate) fn issue(&mut self) -> Result<Self, ChangeTickError> {
        self.advance()
    }

    pub(crate) fn can_advance_n(&self, count: usize) -> bool {
        let count = count as u64;
        self.0.checked_add(count).is_some()
    }
}

impl fmt::Display for ChangeTick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedAccumulator, FixedConfig, FixedDebtPolicy, FixedWork};
    use core::time::Duration;

    use alloc::format;

    use super::{ChangeTick, FixedConfigError, FixedStepError, WorldTick};

    #[test]
    fn fixed_config_rejects_non_positive_delta() {
        assert!(matches!(
            FixedConfig::new(Duration::ZERO),
            Err(FixedConfigError::NonPositiveDelta)
        ));
    }

    #[test]
    fn fixed_config_rejects_zero_substep_cap() {
        let config = FixedConfig::new(Duration::from_millis(1)).expect("delta");
        assert!(matches!(
            config.with_max_substeps(0),
            Err(FixedConfigError::ZeroSubstepCap)
        ));
    }

    #[test]
    fn preflight_steps_zero_is_ok() {
        let accumulator = FixedAccumulator::new();
        accumulator.preflight_steps(0).expect("zero steps");
    }

    #[test]
    fn preflight_steps_reports_exhaustion_near_u64_max() {
        let mut accumulator = FixedAccumulator::new();
        accumulator.set_next_index_for_test(u64::MAX);
        assert!(matches!(
            accumulator.preflight_steps(1),
            Err(FixedStepError::Exhausted)
        ));
    }

    #[test]
    fn world_tick_and_change_tick_display_format_raw_values() {
        assert_eq!(format!("{}", WorldTick::ZERO), "0");
        assert_eq!(format!("{}", ChangeTick::from_raw(9)), "9");
    }

    #[test]
    fn default_accumulator_matches_new() {
        assert_eq!(
            FixedAccumulator::default().peek_plan(
                Duration::from_millis(1),
                &FixedConfig::new(Duration::from_millis(1)).expect("delta")
            ),
            FixedAccumulator::new().peek_plan(
                Duration::from_millis(1),
                &FixedConfig::new(Duration::from_millis(1)).expect("delta")
            )
        );
    }

    #[test]
    fn huge_deltas_drop_debt_without_iterating_or_preserving_whole_steps() {
        let config = FixedConfig::new(Duration::from_millis(1))
            .expect("positive delta")
            .with_max_substeps(8)
            .expect("cap");
        let mut accumulator = FixedAccumulator::new();

        let plan = accumulator.plan(Duration::MAX, &config);

        assert_eq!(plan.work, FixedWork::Steps(8));
        assert_eq!(
            plan.dropped.expect("debt").steps,
            Duration::MAX.as_nanos() / config.delta().as_nanos() - 8
        );
        assert!(accumulator.remainder < config.delta());
    }

    #[test]
    fn preserve_debt_keeps_unrun_whole_steps() {
        let config = FixedConfig::new(Duration::from_millis(10))
            .expect("positive delta")
            .with_max_substeps(2)
            .expect("cap")
            .with_debt_policy(FixedDebtPolicy::Preserve);
        let mut accumulator = FixedAccumulator::new();

        let first = accumulator.plan(Duration::from_millis(50), &config);
        assert_eq!(first.work, FixedWork::Steps(2));
        assert!(first.dropped.is_none());
        let second = accumulator.plan(Duration::ZERO, &config);
        assert_eq!(second.work, FixedWork::Steps(2));
    }

    #[test]
    fn coalesce_represents_all_overdue_intervals_once() {
        let config = FixedConfig::new(Duration::from_millis(10))
            .expect("positive delta")
            .with_max_substeps(2)
            .expect("cap")
            .with_debt_policy(FixedDebtPolicy::Coalesce);
        let mut accumulator = FixedAccumulator::new();

        let plan = accumulator.plan(Duration::from_millis(55), &config);
        assert_eq!(
            plan.work,
            FixedWork::Coalesced {
                steps: 5,
                delta: Duration::from_millis(50),
            }
        );
        assert_eq!(plan.coalesced.expect("coalesced").steps, 5);
        assert_eq!(accumulator.remainder, Duration::from_millis(5));
    }
}
