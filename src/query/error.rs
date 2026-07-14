//! Query configuration, ownership, borrow, and cache diagnostics.

use alloc::string::String;

/// Failure while resolving, traversing, caching, or mutating through a query.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    /// Query referenced a component type that is not registered in the world schema.
    UnregisteredComponent { name: String },
    /// Query traversal expected a different storage kind for the named component.
    WrongStorageKind { name: String },
    /// Query spec combined incompatible structural or temporal filters.
    ConflictingFilters { detail: String },
    /// Mutable traversal requested the same component type more than once.
    DuplicateMutableComponent { name: String },
    /// Query handle or cursor belongs to a different world owner.
    WrongOwner,
    /// Membership or result cache handle is stale for its slot and generation.
    StaleCache,
    /// Cursor, cache, event, or plan does not match the active query configuration.
    WrongQuery { detail: String },
    /// Result-cache policy cannot serve added/changed moving windows.
    MovingChangeWindow,
    /// Prepared-query materialization policy is incompatible with the resolved plan.
    UnsupportedCachePolicy { detail: String },
    /// Exact-id order conflicts with a result cache that reorders matches.
    ExactIdOrderConflict,
    /// Exact-id list contains the same entity more than once.
    DuplicateExactId { entity: crate::EntityId },
    /// Exact-id policy requires every listed entity to be available.
    MissingExactId { entity: crate::EntityId },
    /// Query traversal cannot borrow world state for the requested operation.
    BorrowConflict { detail: String },
    /// Deferred command or bundle write was rejected before enqueue.
    CommandRejected { detail: String },
    /// Cache handle owner token does not match the active world.
    OwnerMismatch,
    /// Mutable traversal stopped early because a callback returned an error.
    TraversalAborted {
        entity: crate::EntityId,
        detail: String,
    },
}

#[cfg(feature = "std")]
impl core::fmt::Display for QueryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnregisteredComponent { name } => write!(f, "unregistered component '{name}'"),
            Self::WrongStorageKind { name } => write!(f, "wrong storage kind for '{name}'"),
            Self::ConflictingFilters { detail } => write!(f, "conflicting filters: {detail}"),
            Self::DuplicateMutableComponent { name } => {
                write!(f, "duplicate mutable component '{name}'")
            }
            Self::WrongOwner => f.write_str("query handle belongs to another world"),
            Self::StaleCache => f.write_str("stale query cache handle"),
            Self::WrongQuery { detail } => write!(f, "wrong query cursor: {detail}"),
            Self::MovingChangeWindow => f.write_str(
                "added/changed filters require a traversal or membership policy, not Result",
            ),
            Self::UnsupportedCachePolicy { detail } => {
                write!(f, "unsupported cache policy: {detail}")
            }
            Self::ExactIdOrderConflict => f.write_str("exact-id order conflicts with result cache"),
            Self::DuplicateExactId { entity } => {
                write!(f, "exact-id query contains duplicate entity {entity:?}")
            }
            Self::MissingExactId { entity } => {
                write!(f, "exact-id query missing unavailable entity {entity:?}")
            }
            Self::BorrowConflict { detail } => write!(f, "query borrow conflict: {detail}"),
            Self::CommandRejected { detail } => write!(f, "query command rejected: {detail}"),
            Self::OwnerMismatch => f.write_str("query handle owner mismatch"),
            Self::TraversalAborted { entity, detail } => {
                write!(f, "query traversal aborted at {entity:?}: {detail}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for QueryError {}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::world::WorldBuilder;
    use alloc::string::ToString;

    #[test]
    fn display_covers_entity_and_command_diagnostics() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<u8>(ComponentOptions::sparse())
            .expect("component");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("entity");

        assert!(QueryError::DuplicateExactId { entity }
            .to_string()
            .contains("duplicate entity"));
        assert_eq!(
            QueryError::CommandRejected {
                detail: String::from("stale target"),
            }
            .to_string(),
            "query command rejected: stale target"
        );
    }
}
