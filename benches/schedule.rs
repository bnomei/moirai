use moirai::schedule::{stage, System};
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
fn app_update() {
    let mut app = setup().build().expect("build");
    app.update(1.0 / 60.0).expect("update");
    divan::black_box(());
}
