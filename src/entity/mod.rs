//! Entity identity and generational allocation for one [`crate::world::World`].
//!
//! [`EntityId`] is the public opaque handle. Allocation and slot lifecycle live in the private
//! generational allocator used by spawn, reserve, and despawn paths.

mod allocator;
mod id;

pub(crate) use allocator::{AllocatorError, EntityAllocator};
pub use id::EntityId;
