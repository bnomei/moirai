use alloc::string::String;
use alloc::vec::Vec;

use crate::world::WorldError;

/// Schedule construction failure before first execution.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildError {
    PendingCommands,
    WorldRunning,
    WorldMutationPoisoned,
    LeaseMismatch,
    LiveLeaseAlreadyAttached,
    UnknownStage { label: String },
    UnknownSystem { label: String },
    UnknownSystemSet { label: String },
    DuplicateSystemSet { label: String },
    DuplicateSystemLabel { label: String },
    CrossOperationEdge { from: String, to: String },
    CrossStageSystemEdge { from: String, to: String },
    MissingRequiredResource { name: String },
    SelfEdge { label: String },
    Cycle { path: Vec<String> },
    FixedUpdateWithoutConfig,
    FixedConfigWithoutFixedUpdate,
    StageOperationMismatch { label: String },
    WorldBuild(WorldError),
}

impl From<WorldError> for BuildError {
    fn from(value: WorldError) -> Self {
        Self::WorldBuild(value)
    }
}

/// Runtime schedule control failure.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    OwnerMismatch,
    StaleHandle,
    SystemNotFound { label: String },
}

#[cfg(feature = "std")]
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PendingCommands => f.write_str("world has pending commands"),
            Self::WorldRunning => f.write_str("world is running"),
            Self::WorldMutationPoisoned => f.write_str("world mutation is poisoned"),
            Self::LeaseMismatch => f.write_str("world and schedule execution lease mismatch"),
            Self::LiveLeaseAlreadyAttached => {
                f.write_str("world already has a live compiled schedule lease")
            }
            Self::UnknownStage { label } => write!(f, "unknown stage '{label}'"),
            Self::UnknownSystem { label } => write!(f, "unknown system '{label}'"),
            Self::UnknownSystemSet { label } => write!(f, "unknown system set '{label}'"),
            Self::DuplicateSystemSet { label } => write!(f, "duplicate system set '{label}'"),
            Self::DuplicateSystemLabel { label } => write!(f, "duplicate system label '{label}'"),
            Self::CrossStageSystemEdge { from, to } => {
                write!(f, "cross-stage system edge: {from} -> {to}")
            }
            Self::MissingRequiredResource { name } => {
                write!(f, "missing required resource '{name}'")
            }
            Self::CrossOperationEdge { from, to } => {
                write!(f, "ordering edge crosses operations: {from} -> {to}")
            }
            Self::SelfEdge { label } => write!(f, "system cannot depend on itself: {label}"),
            Self::Cycle { path } => write!(f, "schedule cycle: {}", path.join(" -> ")),
            Self::FixedUpdateWithoutConfig => {
                f.write_str("FixedUpdate systems require fixed configuration")
            }
            Self::FixedConfigWithoutFixedUpdate => {
                f.write_str("fixed configuration requires a FixedUpdate stage")
            }
            Self::StageOperationMismatch { label } => {
                write!(f, "stage operation mismatch for '{label}'")
            }
            Self::WorldBuild(error) => write!(f, "world build failed: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("schedule handle belongs to a different schedule"),
            Self::StaleHandle => f.write_str("stale schedule handle"),
            Self::SystemNotFound { label } => write!(f, "system not found: {label}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BuildError {}

#[cfg(feature = "std")]
impl std::error::Error for ScheduleError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldError;

    #[test]
    fn world_error_converts_into_build_error() {
        let error: BuildError = WorldError::NestedRun.into();
        assert!(matches!(
            error,
            BuildError::WorldBuild(WorldError::NestedRun)
        ));
    }
}
