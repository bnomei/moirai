use super::*;
use alloc::format;
use core::mem::{align_of, size_of};
use rstest::rstest;

#[rstest]
fn transparent_layout() {
    assert_eq!(size_of::<Q16>(), size_of::<i32>());
    assert_eq!(align_of::<Q16>(), align_of::<i32>());
}

#[rstest]
fn bit_round_trip() {
    let bits = 42_000;
    assert_eq!(Q16::from_bits(bits).to_bits(), bits);
}

#[rstest]
#[case(0.5, 32_768)]
#[case(-0.5, -32_768)]
#[case(1.5, 98_304)]
#[case(-1.5, -98_304)]
fn half_away_from_zero_f32(#[case] input: f32, #[case] expected: i32) {
    assert_eq!(
        Q16::try_from_f32(input).expect("finite").to_bits(),
        expected
    );
}

#[rstest]
fn out_of_range_f32_is_rejected() {
    assert_eq!(Q16::try_from_f32(32768.0), Err(Q16Error::OutOfRange));
}

#[rstest]
fn divide_by_zero_is_rejected() {
    let one = Q16::ONE;
    assert_eq!(one.checked_div(Q16::ZERO), Err(Q16Error::DivisionByZero));
}

#[rstest]
fn checked_add_overflow() {
    assert_eq!(Q16::MAX.checked_add(Q16::ONE), Err(Q16Error::Overflow));
}

#[rstest]
fn reference_mul_matches_wider_integer_model() {
    let a = Q16::try_from_f32(1.25).expect("a");
    let b = Q16::try_from_f32(2.0).expect("b");
    let product = (a.to_bits() as i64) * (b.to_bits() as i64);
    let expected = ((product + (1 << 15)) >> 16) as i32;
    assert_eq!(a.checked_mul(b).expect("mul").to_bits(), expected);
}

#[rstest]
fn from_i32_round_trip_small_values() {
    assert_eq!(Q16::from_i32(3).expect("from").to_bits(), 3 << 16);
    assert_eq!(Q16::from_i32(-2).expect("neg").to_bits(), -2 << 16);
}

#[rstest]
fn from_i32_overflow_is_rejected() {
    assert_eq!(Q16::from_i32(i32::MAX), Err(Q16Error::Overflow));
}

#[rstest]
fn saturating_from_i32_clamps() {
    assert_eq!(Q16::saturating_from_i32(i32::MAX), Q16::MAX);
    assert_eq!(Q16::saturating_from_i32(i32::MIN), Q16::MIN);
}

#[rstest]
fn saturating_arithmetic_clamps() {
    assert_eq!(Q16::MAX.saturating_add(Q16::ONE), Q16::MAX);
    assert_eq!(Q16::MIN.saturating_sub(Q16::ONE), Q16::MIN);
    assert_eq!(Q16::MAX.saturating_mul(Q16::MAX), Q16::MAX);
}

#[rstest]
fn to_f32_recovers_approximate_value() {
    let q = Q16::try_from_f32(1.5).expect("q");
    assert!((q.to_f32() - 1.5).abs() < 0.001);
}

#[rstest]
fn ordering_is_stable() {
    let low = Q16::try_from_f32(1.0).expect("low");
    let high = Q16::try_from_f32(2.0).expect("high");
    assert!(low < high);
    assert_eq!(low.cmp(&high), core::cmp::Ordering::Less);
}

#[rstest]
fn saturating_from_f32_finite_and_infinite() {
    assert_eq!(
        Q16::saturating_from_f32(1.25).expect("finite").to_bits(),
        Q16::try_from_f32(1.25).expect("try").to_bits()
    );
    assert_eq!(Q16::saturating_from_f32(f32::INFINITY), Ok(Q16::MAX));
    assert_eq!(Q16::saturating_from_f32(f32::NEG_INFINITY), Ok(Q16::MIN));
    assert_eq!(Q16::saturating_from_f32(f32::NAN), Err(Q16Error::NotFinite));
}

#[rstest]
fn saturating_mul_negative_overflow_clamps_to_min() {
    assert_eq!(Q16::MIN.saturating_mul(Q16::ONE), Q16::MIN);
}

#[rstest]
fn saturating_div_by_zero_reports_error() {
    let one = Q16::ONE;
    assert_eq!(one.saturating_div(Q16::ZERO), Err(Q16Error::DivisionByZero));
}

#[rstest]
fn saturating_div_rounds_and_clamps() {
    let half = Q16::try_from_f32(0.5).expect("half");
    assert_eq!(
        half.saturating_div(Q16::ONE).expect("div").to_bits(),
        half.to_bits()
    );
    assert_eq!(
        Q16::MIN
            .saturating_div(Q16::ONE)
            .expect("underflow")
            .to_bits(),
        Q16::MIN.to_bits()
    );
}

#[rstest]
fn debug_formats_as_float() {
    let q = Q16::try_from_f32(1.5).expect("q");
    assert!(format!("{q:?}").contains("1.5"));
}

#[rstest]
fn checked_mul_underflow_is_rejected() {
    assert_eq!(
        Q16::MIN.checked_mul(Q16::from_bits(65_537)),
        Err(Q16Error::Underflow)
    );
}

#[rstest]
fn checked_div_underflow_is_rejected() {
    let min = Q16::MIN;
    let half = Q16::from_bits(1 << 15);
    assert_eq!(min.checked_div(half), Err(Q16Error::Underflow));
}

#[rstest]
fn checked_div_reports_overflow_for_large_quotient() {
    let huge = Q16::from_bits(1 << 30);
    let tiny = Q16::from_bits(1);
    assert_eq!(huge.checked_div(tiny), Err(Q16Error::Overflow));
}

#[rstest]
fn checked_div_success_and_overflow_paths() {
    let one = Q16::ONE;
    let half = Q16::try_from_f32(0.5).expect("half");
    assert_eq!(
        one.checked_div(half).expect("div").to_bits(),
        one.to_bits() << 1
    );
    let huge = Q16::from_bits(1 << 30);
    assert_eq!(huge.checked_mul(huge), Err(Q16Error::Overflow));
}

#[rstest]
fn randomized_mul_matches_reference() {
    let mut seed = 0xABCD_u32;
    for _ in 0..500 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1);
        let ai = (seed % 2_000) as i32 - 1_000;
        let bi = ((seed >> 8) % 2_000) as i32 - 1_000;
        let a = Q16::from_bits(ai);
        let b = Q16::from_bits(bi);
        let product = (ai as i64) * (bi as i64);
        let expected = (product + (1 << 15)) >> 16;
        if let Ok(result) = a.checked_mul(b) {
            if (i32::MIN as i64..=i32::MAX as i64).contains(&expected) {
                assert_eq!(result, Q16(expected as i32));
            }
        }
    }
}

#[rstest]
fn round_half_away_i64_zero_denominator_returns_zero() {
    assert_eq!(super::round_half_away_i64_for_test(9, 0), 0);
}
