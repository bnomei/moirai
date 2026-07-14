use alloc::string::String;
use alloc::vec::Vec;

use crate::operation::StageOperation;
use crate::schedule::system::FlushMode;
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
    UnknownStage {
        label: String,
    },
    UnknownSystem {
        label: String,
    },
    UnknownSystemSet {
        label: String,
    },
    DuplicateSystemSet {
        label: String,
    },
    DuplicateSystemLabel {
        label: String,
    },
    SystemInitialization {
        system: String,
        detail: String,
    },
    CrossOperationEdge {
        from: String,
        to: String,
    },
    CrossStageSystemEdge {
        from: String,
        to: String,
    },
    MissingRequiredResource {
        name: String,
    },
    UnregisteredEventRole {
        system: String,
        event: String,
    },
    EventOperationMismatch {
        system: String,
        event: String,
        event_operation: StageOperation,
        system_operation: StageOperation,
    },
    MissingEventProducer {
        system: String,
        event: String,
    },
    UnreachableEventProducer {
        producer: String,
        consumer: String,
        event: String,
    },
    SelfEdge {
        label: String,
    },
    Cycle {
        path: Vec<String>,
    },
    FixedUpdateWithoutConfig,
    FixedConfigWithoutFixedUpdate,
    StageOperationMismatch {
        label: String,
    },
    InvalidStageFlushMode {
        label: String,
        mode: FlushMode,
    },
    InvalidSystemFlushMode {
        label: String,
        mode: FlushMode,
    },
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
            Self::SystemInitialization { system, detail } => {
                write!(f, "system '{system}' initialization failed: {detail}")
            }
            Self::CrossStageSystemEdge { from, to } => {
                write!(f, "cross-stage system edge: {from} -> {to}")
            }
            Self::MissingRequiredResource { name } => {
                write!(f, "missing required resource '{name}'")
            }
            Self::UnregisteredEventRole { system, event } => {
                write!(f, "system '{system}' declares unregistered event role '{event}'")
            }
            Self::EventOperationMismatch {
                system,
                event,
                event_operation,
                system_operation,
            } => write!(
                f,
                "system '{system}' runs in {system_operation:?} but event '{event}' belongs to {event_operation:?}"
            ),
            Self::MissingEventProducer { system, event } => {
                write!(f, "event consumer '{system}' has no producer for '{event}'")
            }
            Self::UnreachableEventProducer {
                producer,
                consumer,
                event,
            } => write!(
                f,
                "event producer '{producer}' is not ordered before consumer '{consumer}' for '{event}'"
            ),
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
            Self::InvalidStageFlushMode { label, mode } => {
                write!(f, "invalid {mode:?} flush mode for stage '{label}'")
            }
            Self::InvalidSystemFlushMode { label, mode } => {
                write!(f, "invalid {mode:?} flush mode for system '{label}'")
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
