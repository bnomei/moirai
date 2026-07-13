use moirai::math::Q16Error;
use moirai::Q16;

#[test]
fn q16_saturating_from_f32_non_finite() {
    assert_eq!(Q16::saturating_from_f32(f32::NAN), Err(Q16Error::NotFinite));
    assert_eq!(Q16::saturating_from_f32(f32::INFINITY), Ok(Q16::MAX));
    assert_eq!(Q16::saturating_from_f32(f32::NEG_INFINITY), Ok(Q16::MIN));
}

#[test]
fn q16_checked_sub_underflow() {
    assert_eq!(Q16::MIN.checked_sub(Q16::ONE), Err(Q16Error::Underflow));
}

#[test]
fn q16_saturating_div_clamps_overflow() {
    let tiny = Q16::from_bits(1);
    assert_eq!(Q16::MAX.saturating_div(tiny), Ok(Q16::MAX));
}

#[test]
fn q16_try_from_f32_rejects_nan() {
    assert_eq!(Q16::try_from_f32(f32::NAN), Err(Q16Error::NotFinite));
}
