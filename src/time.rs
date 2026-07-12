use core::fmt;

/// Monotonic frame counter advanced only by `App`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct WorldTick(u64);

/// Monotonic world change counter for component/resource mutation metadata.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct ChangeTick(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangeTickError {
    Exhausted,
}

impl WorldTick {
    pub const ZERO: Self = Self(0);

    pub fn raw(self) -> u64 {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) fn set_raw(&mut self, raw: u64) {
        self.0 = raw;
    }
}

impl fmt::Display for WorldTick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ChangeTick {
    pub const ZERO: Self = Self(0);

    pub const fn from_raw(raw: u64) -> Self {
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
