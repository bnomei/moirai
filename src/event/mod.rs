//! Typed event registration, retention policy, and independent readers.
//!
//! Ordinary events are registered through [`EventOptions`] and read with [`EventReader`].
//! Component lifecycle emits [`ComponentAdded`] and [`ComponentRemoved`] on structural commits.

mod component;
mod queue;
mod registry;

pub use component::{ComponentAdded, ComponentRemoved};
pub use queue::{EventReader, EventReaderStart};
pub use registry::{EventId, EventOptions, EventRegistrationError, EventRetention};

pub(crate) use component::ComponentLifecycleRegistry;
pub(crate) use queue::EventStorage;
pub(crate) use registry::EventRegistry;
