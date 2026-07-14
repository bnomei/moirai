//! Checked replay step indexing for finite deterministic runs.

/// Opaque checked replay step index. The first step is always zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StepIndex(u32);

/// Checked replay step index overflow.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct StepIndexError;

impl StepIndex {
    /// First replay step in every finite run.
    pub const FIRST: Self = Self(0);

    /// Underlying step counter used by replay loops and driver contracts.
    pub fn raw(self) -> u32 {
        self.0
    }

    /// Advance to the next checked replay step or return [`StepIndexError`] on overflow.
    pub fn next(self) -> Result<Self, StepIndexError> {
        let next = self.0.checked_add(1).ok_or(StepIndexError)?;
        Ok(Self(next))
    }

    /// Construct a replay step index from a host-provided raw value.
    ///
    /// This supports replay drivers that resume from persisted evidence. Use [`Self::next`]
    /// when advancing a step so overflow remains explicit.
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}
