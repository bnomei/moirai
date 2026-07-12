use crate::component::RegistrationError;
use crate::entity::EntityId;
use alloc::string::String;

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldAllocatorError {
    GenerationOverflow,
    SlotRetired,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldError {
    StaleEntity { entity: EntityId },
    UnregisteredComponent { name: String },
    WrongStorageKind { name: String },
    Registration(RegistrationError),
    Allocator(WorldAllocatorError),
    ChangeTickExhausted,
}

impl From<RegistrationError> for WorldError {
    fn from(value: RegistrationError) -> Self {
        Self::Registration(value)
    }
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
            Self::UnregisteredComponent { name } => {
                write!(f, "unregistered component {name}")
            }
            Self::WrongStorageKind { name } => {
                write!(f, "wrong storage kind for {name}")
            }
            Self::Registration(error) => write!(f, "component registration failed: {error}"),
            Self::Allocator(error) => write!(f, "entity allocator failed: {error}"),
            Self::ChangeTickExhausted => f.write_str("change tick exhausted"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WorldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registration(error) => Some(error),
            Self::Allocator(error) => Some(error),
            _ => None,
        }
    }
}
