use moirai::component::ComponentOptions;
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
struct TablePos(i32);

#[derive(Clone, Copy)]
struct Velocity(i32);

fn setup() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register pos");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("register vel");
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

#[divan::bench]
fn archetype_move_insert_second_table_component() {
    let mut world = setup();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(1)).expect("insert pos");
    world.insert(entity, Velocity(2)).expect("insert vel");
    let value = world.get::<Velocity>(entity).expect("get").expect("present");
    divan::black_box(value.0);
}

#[divan::bench]
fn deferred_command_flush() {
    let mut world = setup();
    let entity = world.commands().expect("commands").spawn().expect("reserve");
    world
        .commands()
        .expect("commands")
        .insert(entity, TablePos(9))
        .expect("queue");
    world.flush().expect("flush");
    divan::black_box(entity);
}

fn main() {
    divan::main();
}