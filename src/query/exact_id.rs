/// Policy for exact-id queries when an id is unavailable or does not match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
pub enum ExactIdPolicy {
    /// Skip unavailable ids without error.
    #[default]
    SkipUnavailable,
    /// Fail the query on the first unavailable id.
    ErrorOnUnavailable,
}
