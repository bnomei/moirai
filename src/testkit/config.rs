use core::fmt;

/// When replay captures host snapshots into the final report.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CapturePolicy {
    /// Capture after every successful step.
    EveryStep,
    /// Capture only after the final configured step.
    FinalOnly,
}

/// Finite replay run policy. The host factory receives `seed`; Moirai owns no RNG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayConfig {
    seed: u64,
    steps: u32,
    capture: CapturePolicy,
}

/// Invalid replay configuration.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayConfigError {
    ZeroSteps,
}

impl ReplayConfig {
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

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn steps(&self) -> u32 {
        self.steps
    }

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
