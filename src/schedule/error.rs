//! Schedule build-time and runtime control errors (`no_std` with optional `std` display).

use alloc::string::String;
use alloc::vec::Vec;

use crate::operation::StageOperation;
use crate::schedule::system::FlushMode;
use crate::world::WorldError;

/// Schedule construction failure before first execution.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildError {
    /// World still has unflushed deferred commands.
    PendingCommands,
    /// World run guard is active; build requires an idle world.
    WorldRunning,
    /// Change-tick exhaustion poisoned the world.
    WorldMutationPoisoned,
    /// Attached execution lease does not match this schedule.
    LeaseMismatch,
    /// Another compiled schedule already holds a live world lease.
    LiveLeaseAlreadyAttached,
    /// Referenced stage label was never registered.
    UnknownStage { label: String },
    /// Ordering edge names a system that was never added.
    UnknownSystem { label: String },
    /// System or ordering edge names an unregistered set.
    UnknownSystemSet { label: String },
    /// Same system-set label registered twice.
    DuplicateSystemSet { label: String },
    /// Two systems share the same label.
    DuplicateSystemLabel { label: String },
    /// Build-time initializer for a system returned an error.
    SystemInitialization { system: String, detail: String },
    /// Ordering edge would cross Update/Render operations.
    CrossOperationEdge { from: String, to: String },
    /// System ordering edge spans different stages.
    CrossStageSystemEdge { from: String, to: String },
    /// System declared a resource lock for a type not present in the world.
    MissingRequiredResource { name: String },
    /// System event role references an unregistered event or lifecycle channel.
    UnregisteredEventRole { system: String, event: String },
    /// Frame-retained event operation does not match the system's stage operation.
    EventOperationMismatch {
        system: String,
        event: String,
        event_operation: StageOperation,
        system_operation: StageOperation,
    },
    /// Internal frame event consumer has no emitting producer.
    MissingEventProducer { system: String, event: String },
    /// Producer is not ordered before the consumer for the same frame event.
    UnreachableEventProducer {
        producer: String,
        consumer: String,
        event: String,
    },
    /// System depends on itself in the ordering graph.
    SelfEdge { label: String },
    /// Stage-local ordering graph contains a cycle.
    Cycle { path: Vec<String> },
    /// FixedUpdate systems exist but no fixed timestep was configured.
    FixedUpdateWithoutConfig,
    /// Fixed timestep configured without a FixedUpdate stage.
    FixedConfigWithoutFixedUpdate,
    /// Re-adding a stage label with a different operation.
    StageOperationMismatch { label: String },
    /// Flush mode is invalid for the stage's operation.
    InvalidStageFlushMode { label: String, mode: FlushMode },
    /// Flush mode is invalid for the system's stage operation.
    InvalidSystemFlushMode { label: String, mode: FlushMode },
    /// Underlying world construction or attachment failed.
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
    /// Handle was issued by a different schedule instance.
    OwnerMismatch,
    /// Handle index or generation no longer matches the compiled schedule.
    StaleHandle,
    /// Update plan lists the same stage more than once.
    DuplicateStageInPlan,
    /// Update plan includes a Render-only stage.
    NonUpdateStageInPlan,
    /// Startup is implicit on first update and cannot be planned explicitly.
    StartupStageInPlan,
    /// No compiled system matches the label.
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
            Self::DuplicateStageInPlan => f.write_str("update plan contains a duplicate stage"),
            Self::NonUpdateStageInPlan => f.write_str("update plan contains a non-update stage"),
            Self::StartupStageInPlan => f.write_str("update plan cannot select Startup"),
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
    #[cfg(feature = "std")]
    use alloc::string::ToString;

    #[test]
    fn world_error_converts_into_build_error() {
        let error: BuildError = WorldError::NestedRun.into();
        assert!(matches!(
            error,
            BuildError::WorldBuild(WorldError::NestedRun)
        ));
    }

    #[cfg(feature = "std")]
    #[test]
    fn build_error_display_covers_detailed_variants() {
        let errors = [
            BuildError::SystemInitialization {
                system: "init".into(),
                detail: "failed".into(),
            },
            BuildError::UnregisteredEventRole {
                system: "reader".into(),
                event: "event".into(),
            },
            BuildError::EventOperationMismatch {
                system: "reader".into(),
                event: "event".into(),
                event_operation: crate::schedule::StageOperation::Render,
                system_operation: crate::schedule::StageOperation::Update,
            },
            BuildError::MissingEventProducer {
                system: "reader".into(),
                event: "event".into(),
            },
            BuildError::UnreachableEventProducer {
                producer: "writer".into(),
                consumer: "reader".into(),
                event: "event".into(),
            },
            BuildError::InvalidStageFlushMode {
                label: "Render".into(),
                mode: crate::schedule::FlushMode::Stage,
            },
            BuildError::InvalidSystemFlushMode {
                label: "render".into(),
                mode: crate::schedule::FlushMode::AfterSystem,
            },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
