use super::*;
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
    assert_eq!(Q16::try_from_f32(input).expect("finite").to_bits(), expected);
}

#[rstest]
fn divide_by_zero_is_rejected() {
    let one = Q16::ONE;
    assert_eq!(one.checked_div(Q16::ZERO), Err(Q16Error::DivisionByZero));
}

#[rstest]
fn checked_add_overflow() {
    assert_eq!(
        Q16::MAX.checked_add(Q16::ONE),
        Err(Q16Error::Overflow)
    );
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