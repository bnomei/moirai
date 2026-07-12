mod options;
mod registry;

pub use options::{ComponentOptions, StorageKind};
pub(crate) use registry::ComponentRegistry;
pub use registry::{ComponentId, RegistrationError};
