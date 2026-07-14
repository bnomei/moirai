//! Resolved query plan types shared by collection, cache, and mutation paths.

use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::ExactIdPolicy;

/// Private traversal driver chosen during plan resolution.
#[derive(Clone, Debug)]
pub(crate) enum TraversalSource {
    All,
    Sparse { component_index: usize },
    Table { component_index: usize },
    Exact { ids: Vec<EntityId> },
}

/// Normalized structural query plan with traversal driver and filter indices.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedPlan {
    pub fingerprint: u64,
    pub primary_index: usize,
    pub primary_is_table: bool,
    pub traversal: TraversalSource,
    pub required_indices: Vec<usize>,
    pub without_indices: Vec<usize>,
    pub with_tag_indices: Vec<usize>,
    pub without_tag_indices: Vec<usize>,
    pub added_indices: Vec<usize>,
    pub changed_indices: Vec<usize>,
    pub exact_id_policy: Option<ExactIdPolicy>,
}
