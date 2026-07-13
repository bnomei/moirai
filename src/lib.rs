//! Moirai is a single-threaded, `no_std + alloc` entity-component-system library.
//!
//! This crate publishes concept-level namespaces and curated root re-exports as each
//! owning phase lands real behavior. Implementation modules such as allocators,
//! registries, storage engines, command queues, and schedule runners stay private
//! from their first commit.
//!
//! # Crate root and prelude admission
//!
//! A name may appear on the crate root or in `moirai::prelude` only when:
//!
//! 1. its owning phase has implemented the real invariant, and
//! 2. a public-API test proves the import path.
//!
//! Phase 2 publishes `component`, `math`, and `world` namespaces plus curated root
//! re-exports. `EntityId` is re-exported at the crate root; the physical `entity`
//! module stays private. `moirai::prelude` remains withheld until Phase 4.
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
pub mod schedule;
pub mod state;
mod storage;
mod time;

mod entity;

pub mod component;
pub mod math;
pub mod world;

#[cfg(feature = "testkit")]
pub mod testkit;

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
    ExactIdPolicy, Query1, Query2, QueryCache, QueryCommands, QueryCursor, QueryEffects,
    QueryError, QueryParams, QueryResultCache, QuerySpec,
};
pub use schedule::stage;
pub use schedule::{
    Condition, FlushMode, Schedule, ScheduleBuilder, ScheduleError, System, SystemId, SystemSet,
};
pub use state::{apply, State, StateError};
pub use time::{ChangeTick, FixedConfig, FixedStep, WorldTick};
pub use world::{Commands, DynamicBundle, World, WorldBuilder, WorldError};
