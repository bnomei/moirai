/// Opaque checked replay step index. The first step is always zero.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StepIndex(u32);

/// Step index overflow.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct StepIndexError;

impl StepIndex {
    pub const FIRST: Self = Self(0);

    pub fn raw(self) -> u32 {
        self.0
    }

    pub fn next(self) -> Result<Self, StepIndexError> {
        let next = self.0.checked_add(1).ok_or(StepIndexError)?;
        Ok(Self(next))
    }

    /// Construct a step index from a raw value. Intended for host tests and drivers.
    #[cfg(feature = "testkit")]
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[cfg(test)]
    pub(crate) fn from_raw_for_test(raw: u32) -> Self {
        Self(raw)
    }
}
