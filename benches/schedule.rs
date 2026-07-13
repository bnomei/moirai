use divan::{counter::ItemsCount, Bencher};
use moirai::schedule::{stage, Condition, System};
use moirai::AppBuilder;

fn main() {
    divan::main();
}

fn setup() -> AppBuilder {
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("work", stage::UPDATE, |_world, _dt| {}))
        .expect("add");
    builder
}

#[divan::bench]
fn app_update_including_setup() {
    let mut app = setup().build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    divan::black_box(());
}

#[divan::bench]
fn app_update_operation(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(1_usize))
        .with_inputs(|| setup().build().expect("build"))
        .bench_local_refs(|app| {
            app.update(1.0 / 60.0).expect("update");
        });
}

fn skewed_condition(depth: usize) -> Condition {
    let mut condition = Condition::always();
    for _ in 0..depth {
        condition = condition.and(Condition::always());
    }
    condition
}

fn conditional_app(depth: usize) -> moirai::App {
    let mut builder = AppBuilder::new();
    builder
        .add_system(
            System::new("conditional_work", stage::UPDATE, |_world, _dt| {})
                .run_if(skewed_condition(depth)),
        )
        .expect("add conditional system");
    builder.build().expect("build")
}

#[divan::bench(args = [1_usize, 4, 16, 64])]
fn composite_condition_update(bencher: Bencher, depth: usize) {
    bencher
        .counter(ItemsCount::new(depth + 1))
        .with_inputs(|| conditional_app(depth))
        .bench_local_refs(|app| {
            app.update(1.0 / 60.0).expect("update");
        });
}
