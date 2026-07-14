//! Typed world resources with change ticks, borrow locks, and scoped take/restore.
//!
//! [`ResourceStore`] backs [`crate::world::World`] resource insertion, mutation metadata, and
//! schedule-time scoped access.

mod store;

pub(crate) use store::{ResourceStore, ScopedResource};
