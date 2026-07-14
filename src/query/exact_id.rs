//! Policy for exact-id query lists when an id is unavailable or does not match.

/// How exact-id queries treat unavailable or non-matching entity ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
pub enum ExactIdPolicy {
    /// Skip ids that are unavailable or fail structural filters.
    #[default]
    SkipUnavailable,
    /// Fail resolution on the first unavailable or policy-violating id.
    ErrorOnUnavailable,
}
