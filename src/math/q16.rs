use core::cmp::Ordering;
use core::fmt;

/// Conventional Q16.16 fixed-point value with private bits.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Q16(i32);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Q16Error {
    Overflow,
    Underflow,
    DivisionByZero,
    NotFinite,
    OutOfRange,
}

impl Q16 {
    pub const FRAC_BITS: u32 = 16;
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1 << Self::FRAC_BITS);
    pub const MIN: Self = Self(i32::MIN);
    pub const MAX: Self = Self(i32::MAX);

    pub const fn from_bits(bits: i32) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> i32 {
        self.0
    }

    pub fn from_i32(value: i32) -> Result<Self, Q16Error> {
        value
            .checked_mul(Self::ONE.0)
            .map(Self)
            .ok_or(Q16Error::Overflow)
    }

    pub fn saturating_from_i32(value: i32) -> Self {
        Self(value.saturating_mul(Self::ONE.0))
    }

    pub fn try_from_f32(value: f32) -> Result<Self, Q16Error> {
        if !value.is_finite() {
            return Err(Q16Error::NotFinite);
        }
        let scaled = value * (Self::ONE.0 as f32);
        let rounded = round_half_away_f32(scaled);
        if rounded > i32::MAX as f32 || rounded < i32::MIN as f32 {
            return Err(Q16Error::OutOfRange);
        }
        Ok(Self(rounded as i32))
    }

    pub fn saturating_from_f32(value: f32) -> Result<Self, Q16Error> {
        if value.is_nan() {
            return Err(Q16Error::NotFinite);
        }
        if value.is_infinite() {
            return Ok(if value.is_sign_positive() { Self::MAX } else { Self::MIN });
        }
        let scaled = value * (Self::ONE.0 as f32);
        let rounded = round_half_away_f32(scaled);
        Ok(Self(rounded.clamp(i32::MIN as f32, i32::MAX as f32) as i32))
    }

    pub fn to_f32(self) -> f32 {
        self.0 as f32 / (Self::ONE.0 as f32)
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, Q16Error> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(Q16Error::Overflow)
    }

    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, Q16Error> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(Q16Error::Underflow)
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, Q16Error> {
        let product = (self.0 as i64) * (rhs.0 as i64);
        round_div_fixed(product, Self::FRAC_BITS)
    }

    pub fn saturating_mul(self, rhs: Self) -> Self {
        self.checked_mul(rhs)
            .unwrap_or(if (self.0 > 0) == (rhs.0 > 0) {
                Self::MAX
            } else {
                Self::MIN
            })
    }

    pub fn checked_div(self, rhs: Self) -> Result<Self, Q16Error> {
        if rhs.0 == 0 {
            return Err(Q16Error::DivisionByZero);
        }
        let numerator = (self.0 as i64) << Self::FRAC_BITS;
        round_div_i64(numerator, rhs.0 as i64)
    }

    pub fn saturating_div(self, rhs: Self) -> Result<Self, Q16Error> {
        if rhs.0 == 0 {
            return Err(Q16Error::DivisionByZero);
        }
        let numerator = (self.0 as i64) << Self::FRAC_BITS;
        Ok(Self(
            round_half_away_i64(numerator, rhs.0 as i64)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
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

fn round_half_away_f32(value: f32) -> f32 {
    if value >= 0.0 {
        (value + 0.5) as i32 as f32
    } else {
        (value - 0.5) as i32 as f32
    }
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
    i32::try_from(value)
        .map(Q16)
        .map_err(|_| {
            if value > i32::MAX as i64 {
                Q16Error::Overflow
            } else {
                Q16Error::Underflow
            }
        })
}

fn round_div_i64(numerator: i64, divisor: i64) -> Result<Q16, Q16Error> {
    let value = round_half_away_i64(numerator, divisor);
    i32::try_from(value)
        .map(Q16)
        .map_err(|_| {
            if value > i32::MAX as i64 {
                Q16Error::Overflow
            } else {
                Q16Error::Underflow
            }
        })
}


#[cfg(test)]
mod tests;