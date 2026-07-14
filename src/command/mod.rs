//! Deferred structural mutation queued during system execution.
//!
//! [`Commands`] is the public borrow of the world's command queue. Operations commit at schedule
//! flush boundaries after preflight validation.

mod queue;

pub use queue::Commands;

pub(crate) use queue::{CommandOp, CommandQueue, ErasedComponentValue};
