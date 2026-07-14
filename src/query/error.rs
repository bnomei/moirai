use alloc::string::String;

/// Query configuration, ownership, borrow, and cache diagnostics.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    UnregisteredComponent {
        name: String,
    },
    WrongStorageKind {
        name: String,
    },
    ConflictingFilters {
        detail: String,
    },
    DuplicateMutableComponent {
        name: String,
    },
    WrongOwner,
    StaleCache,
    WrongQuery {
        detail: String,
    },
    MovingChangeWindow,
    UnsupportedCachePolicy {
        detail: String,
    },
    ExactIdOrderConflict,
    DuplicateExactId {
        entity: crate::EntityId,
    },
    MissingExactId {
        entity: crate::EntityId,
    },
    BorrowConflict {
        detail: String,
    },
    CommandRejected {
        detail: String,
    },
    OwnerMismatch,
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
