use moirai::component::ComponentOptions;
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
struct TablePos(i32);

fn setup() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register");
    builder.build().expect("build")
}

#[divan::bench]
fn table_insert_get() {
    let mut world = setup();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(42)).expect("insert");
    let value = world.get::<TablePos>(entity).expect("get").expect("present");
    divan::black_box(value.0);
}

fn main() {
    divan::main();
}
