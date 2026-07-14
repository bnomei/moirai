//! Q16.16 fixed-point arithmetic with checked and saturating operations.
//!
//! Values use 16 fractional bits. Conversions from `f32` round half away from zero.

use core::cmp::Ordering;
use core::fmt;

/// Conventional Q16.16 fixed-point value with private raw bits.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Q16(i32);

/// Checked or saturating [`Q16`] operation failure.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Q16Error {
    /// Result exceeded representable range on addition or multiplication.
    Overflow,
    /// Result fell below representable range on subtraction or multiplication.
    Underflow,
    /// Division by zero.
    DivisionByZero,
    /// Floating-point input was NaN or infinite in a strict conversion.
    NotFinite,
    /// Floating-point input rounded outside `i32` range.
    OutOfRange,
}

impl Q16 {
    /// Number of fractional bits in the Q16.16 layout.
    pub const FRAC_BITS: u32 = 16;
    /// Zero fixed-point value.
    pub const ZERO: Self = Self(0);
    /// One whole unit in fixed-point representation.
    pub const ONE: Self = Self(1 << Self::FRAC_BITS);
    /// Minimum representable value.
    pub const MIN: Self = Self(i32::MIN);
    /// Maximum representable value.
    pub const MAX: Self = Self(i32::MAX);

    /// Constructs a value from raw fixed-point bits.
    pub const fn from_bits(bits: i32) -> Self {
        Self(bits)
    }

    /// Returns the raw fixed-point bit pattern.
    pub const fn to_bits(self) -> i32 {
        self.0
    }

    /// Converts a whole `i32` into fixed-point with checked scaling.
    pub fn from_i32(value: i32) -> Result<Self, Q16Error> {
        value
            .checked_mul(Self::ONE.0)
            .map(Self)
            .ok_or(Q16Error::Overflow)
    }

    /// Converts a whole `i32` into fixed-point with saturating scaling.
    pub fn saturating_from_i32(value: i32) -> Self {
        Self(value.saturating_mul(Self::ONE.0))
    }

    /// Converts `f32` to fixed-point with checked finite range and rounding.
    pub fn try_from_f32(value: f32) -> Result<Self, Q16Error> {
        if !value.is_finite() {
            return Err(Q16Error::NotFinite);
        }
        let scaled = value * (Self::ONE.0 as f32);
        Ok(Self(round_half_away_to_i32(scaled)?))
    }

    /// Converts `f32` to fixed-point, saturating infinities and rejecting NaN.
    pub fn saturating_from_f32(value: f32) -> Result<Self, Q16Error> {
        if value.is_nan() {
            return Err(Q16Error::NotFinite);
        }
        if value.is_infinite() {
            return Ok(if value.is_sign_positive() {
                Self::MAX
            } else {
                Self::MIN
            });
        }
        let scaled = value * (Self::ONE.0 as f32);
        let rounded = round_half_away_to_i64(scaled).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        Ok(Self(rounded))
    }

    /// Converts this value to `f32` for host-side display or interop.
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / (Self::ONE.0 as f32)
    }

    /// Checked fixed-point addition.
    pub fn checked_add(self, rhs: Self) -> Result<Self, Q16Error> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(Q16Error::Overflow)
    }

    /// Saturating fixed-point addition.
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Checked fixed-point subtraction.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Q16Error> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(Q16Error::Underflow)
    }

    /// Saturating fixed-point subtraction.
    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Checked fixed-point multiplication with rounding.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, Q16Error> {
        let product = (self.0 as i64) * (rhs.0 as i64);
        round_div_fixed(product, Self::FRAC_BITS)
    }

    /// Saturating fixed-point multiplication with rounding.
    pub fn saturating_mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs)
            .unwrap_or(if (self.0 > 0) == (rhs.0 > 0) {
                Self::MAX
            } else {
                Self::MIN
            })
    }

    /// Checked fixed-point division with rounding.
    pub fn checked_div(self, rhs: Self) -> Result<Self, Q16Error> {
        if rhs.0 == 0 {
            return Err(Q16Error::DivisionByZero);
        }
        let numerator = (self.0 as i64) << Self::FRAC_BITS;
        round_div_i64(numerator, rhs.0 as i64)
    }

    /// Saturating fixed-point division with rounding.
    pub fn saturating_div(self, rhs: Self) -> Result<Self, Q16Error> {
        if rhs.0 == 0 {
            return Err(Q16Error::DivisionByZero);
        }
        let numerator = (self.0 as i64) << Self::FRAC_BITS;
        Ok(Self(
            round_half_away_i64(numerator, rhs.0 as i64).clamp(i32::MIN as i64, i32::MAX as i64)
                as i32,
        ))
    }
}

impl PartialOrd for Q16 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Q16 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl fmt::Debug for Q16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Q16({})", self.to_f32())
    }
}

#[cfg(feature = "std")]
impl fmt::Display for Q16Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => f.write_str("Q16 overflow"),
            Self::Underflow => f.write_str("Q16 underflow"),
            Self::DivisionByZero => f.write_str("Q16 division by zero"),
            Self::NotFinite => f.write_str("Q16 input is not finite"),
            Self::OutOfRange => f.write_str("Q16 input is out of range"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Q16Error {}

fn round_half_away_to_i32(value: f32) -> Result<i32, Q16Error> {
    i32::try_from(round_half_away_to_i64(value)).map_err(|_| Q16Error::OutOfRange)
}

fn round_half_away_to_i64(value: f32) -> i64 {
    if value >= 0.0 {
        (value as f64 + 0.5) as i64
    } else {
        (value as f64 - 0.5) as i64
    }
}

#[cfg(test)]
pub(crate) fn round_half_away_i64_for_test(numerator: i64, denominator: i64) -> i64 {
    round_half_away_i64(numerator, denominator)
}

fn round_half_away_i64(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return 0;
    }
    let adj = if (numerator >= 0) == (denominator >= 0) {
        denominator / 2
    } else {
        -denominator / 2
    };
    (numerator + adj) / denominator
}

fn round_div_fixed(numerator: i64, shift: u32) -> Result<Q16, Q16Error> {
    let divisor = 1i64 << shift;
    let value = round_half_away_i64(numerator, divisor);
    i32::try_from(value).map(Q16).map_err(|_| {
        if value > i32::MAX as i64 {
            Q16Error::Overflow
        } else {
            Q16Error::Underflow
        }
    })
}

fn round_div_i64(numerator: i64, divisor: i64) -> Result<Q16, Q16Error> {
    let value = round_half_away_i64(numerator, divisor);
    i32::try_from(value).map(Q16).map_err(|_| {
        if value > i32::MAX as i64 {
            Q16Error::Overflow
        } else {
            Q16Error::Underflow
        }
    })
}

#[cfg(test)]
mod tests;
