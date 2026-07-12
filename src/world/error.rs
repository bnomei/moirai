use alloc::string::String;
use crate::component::RegistrationError;
use crate::entity::EntityId;
use crate::time::ChangeTick;

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldAllocatorError {
    GenerationOverflow,
    SlotRetired,
}

/// Summary of a committed structural command batch.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FlushReport {
    pub commands_applied: usize,
    pub change_tick: ChangeTick,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlushError {
    CommandValidation {
        index: usize,
        detail: String,
    },
    ChangeTickExhausted,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldError {
    StaleEntity { entity: EntityId },
    EntityNotLive { entity: EntityId },
    UnregisteredComponent { name: String },
    WrongStorageKind { name: String },
    Registration(RegistrationError),
    Allocator(WorldAllocatorError),
    ChangeTickExhausted,
    StructuralMutationDuringRun,
    StructuralCommandsDuringRender,
    FlushDuringRun,
    DiscardDuringRun,
    Flush(FlushError),
    UnregisteredResource { name: String },
    ResourceInUse { name: String },
    ResourceScoped { name: String },
    UnregisteredEvent { name: String },
    EventChannelClosed,
    NestedRun,
}

impl From<RegistrationError> for WorldError {
    fn from(value: RegistrationError) -> Self {
        Self::Registration(value)
    }
}

impl From<FlushError> for WorldError {
    fn from(value: FlushError) -> Self {
        Self::Flush(value)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventReadError {
    Lagged { dropped: u64 },
    ChannelClosed,
    UnregisteredEvent { name: String },
}

#[cfg(feature = "std")]
impl core::fmt::Display for WorldAllocatorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::GenerationOverflow => f.write_str("entity generation overflow"),
            Self::SlotRetired => f.write_str("entity slot retired"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WorldAllocatorError {}

#[cfg(feature = "std")]
impl core::fmt::Display for FlushError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CommandValidation { index, detail } => {
                write!(f, "command {index} failed validation: {detail}")
            }
            Self::ChangeTickExhausted => f.write_str("change tick exhausted during flush"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FlushError {}

#[cfg(feature = "std")]
impl core::fmt::Display for WorldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StaleEntity { entity } => {
                write!(
                    f,
                    "stale entity {:?}:{:?}",
                    entity.slot(),
                    entity.generation()
                )
            }
            Self::EntityNotLive { entity } => {
                write!(f, "entity {:?}:{:?} is not live", entity.slot(), entity.generation())
            }
            Self::UnregisteredComponent { name } => {
                write!(f, "unregistered component {name}")
            }
            Self::WrongStorageKind { name } => {
                write!(f, "wrong storage kind for {name}")
            }
            Self::Registration(error) => write!(f, "component registration failed: {error}"),
            Self::Allocator(error) => write!(f, "entity allocator failed: {error}"),
            Self::ChangeTickExhausted => f.write_str("change tick exhausted"),
            Self::StructuralMutationDuringRun => {
                f.write_str("structural mutation is deferred while the world is running")
            }
            Self::StructuralCommandsDuringRender => {
                f.write_str("structural commands are unavailable during render")
            }
            Self::FlushDuringRun => f.write_str("flush is idle-only"),
            Self::DiscardDuringRun => f.write_str("discard_commands is idle-only"),
            Self::Flush(error) => write!(f, "flush failed: {error}"),
            Self::UnregisteredResource { name } => write!(f, "unregistered resource {name}"),
            Self::ResourceInUse { name } => write!(f, "resource {name} is in use"),
            Self::ResourceScoped { name } => write!(f, "resource {name} is scoped"),
            Self::UnregisteredEvent { name } => write!(f, "unregistered event {name}"),
            Self::EventChannelClosed => f.write_str("event channel is closed"),
            Self::NestedRun => f.write_str("nested world execution is not supported"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WorldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registration(error) => Some(error),
            Self::Allocator(error) => Some(error),
            Self::Flush(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for EventReadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lagged { dropped } => write!(f, "event reader lagged by {dropped} events"),
            Self::ChannelClosed => f.write_str("event channel is closed"),
            Self::UnregisteredEvent { name } => write!(f, "unregistered event {name}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EventReadError {}