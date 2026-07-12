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
//! Phase 1 publishes neither root re-exports nor `moirai::prelude`. Adding a
//! namespace, root type, or prelude name before then is a contract violation.
//!
//! # Privacy boundary
//!
//! Internal modules are not reachable from downstream crates:
//!
//! ```compile_fail
//! use moirai::entity;
//! ```
//!
//! ```compile_fail
//! use moirai::storage;
//! ```
//!
//! ```compile_fail
//! use moirai::command::queue;
//! ```
//!
//! Future public vocabulary is withheld until its owning phase:
//!
//! ```compile_fail
//! use moirai::World;
//! ```
//!
//! ```compile_fail
//! use moirai::prelude::World;
//! ```
//!
//! ```compile_fail
//! use moirai::component::ComponentId;
//! ```

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod app;
mod command;
mod component;
mod diagnostics;
mod entity;
mod event;
mod math;
mod operation;
mod prelude;
mod query;
mod resource;
mod schedule;
mod state;
mod storage;
mod time;
mod world;

#[cfg(feature = "testkit")]
mod testkit;
