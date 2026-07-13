use moirai::math::{Q16Error, Q16};

const ONE_BITS: i32 = 1 << Q16::FRAC_BITS;
const HALF_BITS: i32 = ONE_BITS / 2;

// The corpora are deliberately static so every revision sees identical operands. Passing each
// pair through black_box keeps the values dynamic from the optimizer's point of view while
// leaving input construction outside the timed operation.
const MUL_CORPUS: &[(i32, i32)] = &[
    (ONE_BITS, ONE_BITS),
    (81_920, HALF_BITS),
    (-81_920, HALF_BITS),
    (98_304, -131_072),
    (1, HALF_BITS),
    (1, HALF_BITS + 1),
    (-1, HALF_BITS),
    (-1, HALF_BITS + 1),
    (i32::MAX, ONE_BITS),
    (i32::MIN, ONE_BITS),
    (i32::MAX, i32::MAX),
    (i32::MIN, ONE_BITS + 1),
    (1 << 30, 1 << 18),
    (-(1 << 30), 1 << 18),
    (12_345_679, 23_456_789),
    (-12_345_679, 23_456_789),
];

const DIV_CORPUS: &[(i32, i32)] = &[
    (ONE_BITS, ONE_BITS),
    (ONE_BITS, HALF_BITS),
    (-ONE_BITS, HALF_BITS),
    (98_304, -131_072),
    (1, 2),
    (-1, 2),
    (3, 2),
    (-3, 2),
    (i32::MAX, ONE_BITS),
    (i32::MIN, ONE_BITS),
    (1 << 30, 1),
    (i32::MIN, HALF_BITS),
    (ONE_BITS, 0),
    (i32::MIN, -1),
    (12_345_679, 23_457),
    (-12_345_679, 23_457),
];

const FLOAT_CORPUS: &[f32] = &[
    0.0,
    -0.0,
    0.5 / ONE_BITS as f32,
    -0.5 / ONE_BITS as f32,
    1.5 / ONE_BITS as f32,
    -1.5 / ONE_BITS as f32,
    0.5,
    -0.5,
    1.25,
    -1.25,
    32_767.0,
    -32_768.0,
    32_768.0,
    -32_769.0,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
];

const TO_FLOAT_CORPUS: &[i32] = &[
    0,
    1,
    -1,
    HALF_BITS,
    -HALF_BITS,
    ONE_BITS,
    -ONE_BITS,
    81_920,
    -81_920,
    1 << 30,
    -(1 << 30),
    i32::MAX,
    i32::MIN,
];

fn result_code(result: Result<Q16, Q16Error>) -> u64 {
    match result {
        Ok(value) => value.to_bits() as u32 as u64,
        Err(Q16Error::Overflow) => 1 << 32,
        Err(Q16Error::Underflow) => 2 << 32,
        Err(Q16Error::DivisionByZero) => 3 << 32,
        Err(Q16Error::NotFinite) => 4 << 32,
        Err(Q16Error::OutOfRange) => 5 << 32,
        Err(_) => 6 << 32,
    }
}

fn fold_result(checksum: u64, result: Result<Q16, Q16Error>) -> u64 {
    checksum.rotate_left(5) ^ result_code(result)
}

#[divan::bench]
fn q16_mul_chain_constant_half_control() -> i32 {
    let mut value = divan::black_box(Q16::from_bits(81_920));
    let half = divan::black_box(Q16::from_bits(HALF_BITS));
    for _ in 0..64 {
        value = value.checked_mul(half).expect("constant half multiply");
    }
    divan::black_box(value.to_bits())
}

#[divan::bench]
fn checked_mul_dynamic_corpus() -> u64 {
    let mut checksum = 0;
    for &(left, right) in MUL_CORPUS {
        let (left, right) = divan::black_box((left, right));
        checksum = fold_result(
            checksum,
            Q16::from_bits(left).checked_mul(Q16::from_bits(right)),
        );
    }
    divan::black_box(checksum)
}

#[divan::bench]
fn saturating_mul_dynamic_corpus() -> u64 {
    let mut checksum = 0_u64;
    for &(left, right) in MUL_CORPUS {
        let (left, right) = divan::black_box((left, right));
        let value = Q16::from_bits(left).saturating_mul(Q16::from_bits(right));
        checksum = checksum.rotate_left(5) ^ value.to_bits() as u32 as u64;
    }
    divan::black_box(checksum)
}

#[divan::bench]
fn checked_div_dynamic_corpus() -> u64 {
    let mut checksum = 0;
    for &(numerator, denominator) in DIV_CORPUS {
        let (numerator, denominator) = divan::black_box((numerator, denominator));
        checksum = fold_result(
            checksum,
            Q16::from_bits(numerator).checked_div(Q16::from_bits(denominator)),
        );
    }
    divan::black_box(checksum)
}

#[divan::bench]
fn saturating_div_dynamic_corpus() -> u64 {
    let mut checksum = 0;
    for &(numerator, denominator) in DIV_CORPUS {
        let (numerator, denominator) = divan::black_box((numerator, denominator));
        checksum = fold_result(
            checksum,
            Q16::from_bits(numerator).saturating_div(Q16::from_bits(denominator)),
        );
    }
    divan::black_box(checksum)
}

#[divan::bench]
fn try_from_f32_dynamic_corpus() -> u64 {
    let mut checksum = 0;
    for &value in FLOAT_CORPUS {
        checksum = fold_result(checksum, Q16::try_from_f32(divan::black_box(value)));
    }
    divan::black_box(checksum)
}

#[divan::bench]
fn saturating_from_f32_dynamic_corpus() -> u64 {
    let mut checksum = 0;
    for &value in FLOAT_CORPUS {
        checksum = fold_result(checksum, Q16::saturating_from_f32(divan::black_box(value)));
    }
    divan::black_box(checksum)
}

#[divan::bench]
fn to_f32_dynamic_corpus() -> u32 {
    let mut checksum = 0_u32;
    for &bits in TO_FLOAT_CORPUS {
        let value = Q16::from_bits(divan::black_box(bits)).to_f32();
        checksum = checksum.rotate_left(5) ^ value.to_bits();
    }
    divan::black_box(checksum)
}

fn main() {
    divan::main();
}
