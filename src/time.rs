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
    pub index: u64,
    pub delta: Duration,
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

    pub fn peek_substeps(
        &self,
        frame_delta: Duration,
        config: &FixedConfig,
    ) -> (u32, Option<FixedDebtDropped>) {
        let (run, debt, _) = Self::substep_plan(self.remainder.saturating_add(frame_delta), config);
        (run, debt)
    }

    pub fn plan_substeps(
        &mut self,
        frame_delta: Duration,
        config: &FixedConfig,
    ) -> (u32, Option<FixedDebtDropped>) {
        let total = self.remainder.saturating_add(frame_delta);
        let (run, debt, remainder) = Self::substep_plan(total, config);
        self.remainder = remainder;
        (run, debt)
    }

    pub fn preflight_substeps(&self, substeps: u32) -> Result<(), FixedStepError> {
        if substeps == 0 {
            return Ok(());
        }
        let last = self
            .next_index
            .checked_add(substeps as u64 - 1)
            .ok_or(FixedStepError::Exhausted)?;
        last.checked_add(1).ok_or(FixedStepError::Exhausted)?;
        Ok(())
    }

    fn substep_plan(
        total: Duration,
        config: &FixedConfig,
    ) -> (u32, Option<FixedDebtDropped>, Duration) {
        let delta_nanos = config.delta().as_nanos();
        let total_nanos = total.as_nanos();
        let due = total_nanos / delta_nanos;
        let run = due.min(config.max_substeps() as u128) as u32;
        let dropped = due - run as u128;
        let debt = if dropped > 0 {
            Some(FixedDebtDropped { steps: dropped })
        } else {
            None
        };
        let remainder = duration_from_nanos(total_nanos % delta_nanos);
        (run, debt, remainder)
    }

    pub fn next_step(&mut self, config: &FixedConfig) -> FixedStep {
        let step = FixedStep {
            index: self.next_index,
            delta: config.delta(),
        };
        self.next_index = self.next_index.saturating_add(1);
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
    use super::{FixedAccumulator, FixedConfig};
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
    fn preflight_substeps_zero_is_ok() {
        let accumulator = FixedAccumulator::new();
        accumulator.preflight_substeps(0).expect("zero substeps");
    }

    #[test]
    fn preflight_substeps_reports_exhaustion_near_u64_max() {
        let mut accumulator = FixedAccumulator::new();
        accumulator.set_next_index_for_test(u64::MAX);
        assert!(matches!(
            accumulator.preflight_substeps(1),
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
            FixedAccumulator::default().peek_substeps(
                Duration::from_millis(1),
                &FixedConfig::new(Duration::from_millis(1)).expect("delta")
            ),
            FixedAccumulator::new().peek_substeps(
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

        let (run, debt) = accumulator.plan_substeps(Duration::MAX, &config);

        assert_eq!(run, 8);
        assert_eq!(
            debt.expect("debt").steps,
            Duration::MAX.as_nanos() / config.delta().as_nanos() - 8
        );
        assert!(accumulator.remainder < config.delta());
    }
}
