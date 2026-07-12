mod options;
mod registry;

pub use options::{ComponentOptions, StorageKind};
pub use registry::{ComponentId, RegistrationError};
pub(crate) use registry::ComponentRegistry;