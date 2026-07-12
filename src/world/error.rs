use alloc::string::String;

use crate::component::RegistrationError;
use crate::entity::EntityId;

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldAllocatorError {
    GenerationOverflow,
    SlotRetired,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldError {
    StaleEntity {
        entity: EntityId,
    },
    UnregisteredComponent {
        name: String,
    },
    WrongStorageKind {
        name: String,
    },
    Registration(RegistrationError),
    Allocator(WorldAllocatorError),
    ChangeTickExhausted,
}

impl From<RegistrationError> for WorldError {
    fn from(value: RegistrationError) -> Self {
        Self::Registration(value)
    }
}