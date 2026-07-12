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
//! Phase 2 publishes `component`, `entity`, `math`, and `world` namespaces plus the
//! matching root re-exports. `moirai::prelude` remains withheld until Phase 4.
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

mod app;
mod command;
mod diagnostics;
mod event;
mod operation;
mod prelude;
mod query;
mod resource;
mod schedule;
mod state;
mod storage;
mod time;

pub mod component;
pub mod entity;
pub mod math;
pub mod world;

#[cfg(feature = "testkit")]
mod testkit;

pub use component::{ComponentId, ComponentOptions, StorageKind};
pub use entity::EntityId;
pub use math::Q16;
pub use world::{World, WorldBuilder, WorldError};