//! Component registration, storage policy, and dense registry-local handles.
//!
//! [`ComponentOptions`] selects sparse or table storage and tag semantics at registration time.
//! [`ComponentId`] identifies a registered component within one world.

mod options;
mod registry;

pub use options::{ComponentOptions, StorageKind};
pub(crate) use registry::ComponentRegistry;
pub use registry::{ComponentId, RegistrationError};
