use moirai::math::Q16;

#[divan::bench]
fn q16_mul_chain() {
    let mut value = Q16::try_from_f32(1.25).expect("seed");
    for _ in 0..64 {
        value = value
            .checked_mul(Q16::try_from_f32(0.5).expect("half"))
            .expect("mul");
        divan::black_box(value.to_bits());
    }
}

fn main() {
    divan::main();
}
