use core::fmt;

/// Monotonic world change counter for component/resource mutation metadata.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct ChangeTick(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangeTickError {
    Exhausted,
}

impl ChangeTick {
    pub const ZERO: Self = Self(0);

    #[allow(dead_code)]
    pub(crate) const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }

    pub(crate) fn advance(&mut self) -> Result<Self, ChangeTickError> {
        self.0 = self.0.checked_add(1).ok_or(ChangeTickError::Exhausted)?;
        Ok(Self(self.0))
    }

    pub(crate) fn issue(&mut self) -> Result<Self, ChangeTickError> {
        self.advance()
    }
}

impl fmt::Display for ChangeTick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}