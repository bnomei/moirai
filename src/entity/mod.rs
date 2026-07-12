mod allocator;
mod id;

pub use id::EntityId;
pub(crate) use allocator::{AllocatorError, EntityAllocator};