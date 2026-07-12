use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::query::ExactIdPolicy;

/// Private traversal driver chosen during plan resolution.
#[derive(Clone, Debug)]
pub(crate) enum TraversalSource {
    Sparse { component_index: usize },
    Table { component_index: usize },
    Exact { ids: Vec<EntityId> },
}

/// Resolved structural query plan.
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
    pub added_index: Option<usize>,
    pub changed_index: Option<usize>,
    pub exact_id_policy: Option<ExactIdPolicy>,
}
