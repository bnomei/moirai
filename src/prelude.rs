//! System-authoring essentials for Moirai hosts.
//!
//! Import this module when writing systems and small apps without pulling the full crate root.
//! It re-exports application wiring, world mutation, scheduling, queries, events, state, time,
//! and revision vocabulary commonly referenced from system bodies.

pub use crate::{
    App, AppBuilder, Bundle, Commands, ComponentOptions, DenseEntityScratch, DynamicBundle,
    EntityId, EntityScratchError, ExactIdPolicy, FlushMode, PreparedQuery1, PreparedQuery2,
    QueryCursor, QueryError, QueryPolicy, QuerySpec, QueryWindow, Revision, RevisionKey,
    StageOperation, State, StateError, StorageKind, System, SystemSet, UpdatePlan, World,
    WorldTick,
};
