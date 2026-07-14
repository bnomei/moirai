//! Checked monotonic revisions and fixed-width revision keys for cache identity.
//!
//! [`Revision`] backs query and storage invalidation counters. [`RevisionKey`] groups ordered
//! revision fields into deterministic comparison keys.

use core::fmt;

/// A checked monotonic revision counter.
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Revision(u64);

impl Revision {
    /// The initial revision.
    pub const ZERO: Self = Self(0);

    /// Returns the raw revision value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advances this revision, leaving it unchanged if the counter is exhausted.
    pub fn advance(&mut self) -> Result<(), RevisionExhausted> {
        let next = self.0.checked_add(1).ok_or(RevisionExhausted)?;
        self.0 = next;
        Ok(())
    }
}

/// Returned when a [`Revision`] cannot advance without overflowing.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RevisionExhausted;

impl fmt::Display for RevisionExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("revision exhausted")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RevisionExhausted {}

/// A fixed-width collection of revisions suitable for cache keys.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RevisionKey<const N: usize>([Revision; N]);

impl<const N: usize> RevisionKey<N> {
    /// Creates a key from its ordered revision fields.
    pub const fn new(revisions: [Revision; N]) -> Self {
        Self(revisions)
    }

    /// Returns the ordered revision fields.
    pub const fn as_array(&self) -> &[Revision; N] {
        &self.0
    }
}

impl<const N: usize> From<[Revision; N]> for RevisionKey<N> {
    fn from(revisions: [Revision; N]) -> Self {
        Self::new(revisions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn revision_advances_from_zero() {
        let mut revision = Revision::ZERO;
        revision.advance().expect("advance");
        assert_eq!(revision.get(), 1);
    }

    #[test]
    fn exhausted_revision_is_unchanged() {
        let mut revision = Revision(u64::MAX);
        assert_eq!(revision.advance(), Err(RevisionExhausted));
        assert_eq!(revision.get(), u64::MAX);
        assert_eq!(RevisionExhausted.to_string(), "revision exhausted");
    }

    #[test]
    fn key_round_trips_its_array() {
        let key = RevisionKey::from([Revision::ZERO; 3]);
        assert_eq!(key.as_array(), &[Revision::ZERO; 3]);
    }
}
