//! Moirai is a single-threaded, `no_std + alloc` entity-component-system library.
//!
//! The public surface is a set of concept-level namespaces, curated root re-exports,
//! and a smaller system-authoring prelude. Implementation modules such as allocators,
//! registries, storage engines, command queues, and schedule runners stay private.
//!
//! # Crate root and prelude admission
//!
//! Common application, world, schedule, query, bundle, state, and time vocabulary is
//! available at the crate root. [`prelude`] contains the subset normally needed to
//! author systems. Advanced construction helpers such as [`world::BundleWriter`] stay
//! in their semantic namespace.
//!
//! # Privacy boundary
//!
//! Internal modules are not reachable from downstream crates:
//!
//! ```compile_fail
//! use moirai::entity::allocator;
//! ```
//!
//! ```compile_fail
//! use moirai::storage;
//! ```
//!
//! ```compile_fail
//! use moirai::component::registry;
//! ```

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod app;
mod command;
pub mod diagnostics;
pub mod event;
mod operation;
pub mod prelude;
pub mod query;
mod resource;
mod revision;
pub mod schedule;
pub mod state;
mod storage;
mod time;

#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod bench_internals;

mod entity;

pub mod component;
pub mod math;
pub mod world;

#[cfg(feature = "testkit")]
pub mod testkit;
#[cfg(any(test, feature = "testkit"))]
#[cfg_attr(not(feature = "testkit"), allow(dead_code))]
#[path = "testkit/ext.rs"]
mod testkit_ext;

pub use app::{App, AppBuilder, AppError, AppFault, BuildError};
pub use component::{ComponentId, ComponentOptions, StorageKind};
pub use entity::EntityId;
pub use event::{
    ComponentAdded, ComponentRemoved, EventId, EventOptions, EventReader, EventReaderStart,
    EventRegistrationError, EventRetention,
};
pub use math::Q16;
pub use operation::StageOperation;
pub use query::{
    ExactIdPolicy, PreparedQuery1, PreparedQuery2, Query1, Query2, QueryCommands, QueryCursor,
    QueryEffects, QueryError, QueryPolicy, QuerySpec, QueryWindow,
};
pub use revision::{Revision, RevisionExhausted, RevisionKey};
pub use schedule::stage;
pub use schedule::{
    Condition, ConditionError, FlushMode, Schedule, ScheduleBuilder, ScheduleError, StageId,
    System, SystemId, SystemInitContext, SystemSet,
};
pub use state::{apply, State, StateError};
pub use time::{ChangeTick, FixedConfig, FixedStep, WorldTick};
pub use world::{
    Bundle, Commands, DenseEntityScratch, DynamicBundle, EntityScratchError, World, WorldBuilder,
    WorldError,
};
