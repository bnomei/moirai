//! Finite replay run policy and per-step record capture selection.

use core::fmt;

/// When replay step records are retained in the final [`ReplayReport`](super::report::ReplayReport).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CapturePolicy {
    /// Capture after every successful step.
    EveryStep,
    /// Capture only after the final configured step.
    FinalOnly,
}

/// Finite replay run policy for [`run_replay`](super::replay::run_replay) and [`replay_app`](super::app::replay_app).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayConfig {
    seed: u64,
    steps: u32,
    capture: CapturePolicy,
}

/// Invalid [`ReplayConfig`] for a finite replay run.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayConfigError {
    /// Replay cannot run with zero steps.
    ZeroSteps,
}

impl ReplayConfig {
    /// Build a finite replay policy. `seed` is forwarded to the host driver fixture; Moirai owns no RNG.
    pub fn new(seed: u64, steps: u32, capture: CapturePolicy) -> Result<Self, ReplayConfigError> {
        if steps == 0 {
            return Err(ReplayConfigError::ZeroSteps);
        }
        Ok(Self {
            seed,
            steps,
            capture,
        })
    }

    /// Replay seed recorded in the report and passed to the driver fixture.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Maximum successful replay steps before the run ends.
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// When step records are captured into the final report.
    pub fn capture(&self) -> CapturePolicy {
        self.capture
    }

    pub(crate) fn should_capture(&self, is_last: bool) -> bool {
        match self.capture {
            CapturePolicy::EveryStep => true,
            CapturePolicy::FinalOnly => is_last,
        }
    }
}

impl fmt::Display for ReplayConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ZeroSteps => f.write_str("replay config requires a non-zero step count"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ReplayConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_and_capture_policy_round_trip() {
        let every = ReplayConfig::new(7, 3, CapturePolicy::EveryStep).expect("every");
        assert_eq!(every.seed(), 7);
        assert_eq!(every.steps(), 3);
        assert_eq!(every.capture(), CapturePolicy::EveryStep);
        assert!(every.should_capture(false));

        let final_only = ReplayConfig::new(8, 2, CapturePolicy::FinalOnly).expect("final");
        assert!(!final_only.should_capture(false));
        assert!(final_only.should_capture(true));
    }
}
