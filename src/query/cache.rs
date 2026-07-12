use crate::world::WorldOwner;

/// Owner-scoped membership acceleration handle.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QueryCache {
    pub(crate) owner: WorldOwner,
    pub(crate) slot: u32,
    pub(crate) generation: u32,
}

/// Owner-scoped materialized entity-id result handle.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QueryResultCache {
    pub(crate) owner: WorldOwner,
    pub(crate) slot: u32,
    pub(crate) generation: u32,
}
