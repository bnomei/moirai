//! Owner-scoped query cache handles.
//!
//! [`QueryCache`] stores structural membership; added/changed filters still apply at traversal.
//! [`QueryResultCache`] stores a materialized id list and rejects moving change windows.

use crate::world::WorldOwner;

/// Handle to a cached structural membership set for one world owner and query spec.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QueryCache {
    pub(crate) owner: WorldOwner,
    pub(crate) slot: u32,
    pub(crate) generation: u32,
}

/// Handle to a cached, fully materialized entity-id result for one world owner and query spec.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QueryResultCache {
    pub(crate) owner: WorldOwner,
    pub(crate) slot: u32,
    pub(crate) generation: u32,
}
