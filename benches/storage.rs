use moirai::component::ComponentOptions;
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
struct BenchComp(u32);

fn setup() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchComp>(ComponentOptions::sparse())
        .expect("register");
    builder.build().expect("build")
}

#[divan::bench]
fn sparse_insert_lookup() {
    let mut world = setup();
    let mut entities = Vec::new();
    for i in 0..128 {
        let entity = world.spawn();
        world
            .insert(entity, BenchComp(i))
            .expect("insert");
        entities.push(entity);
    }
    for (i, entity) in entities.iter().enumerate() {
        let value = world.get::<BenchComp>(*entity).expect("get").expect("present");
        divan::black_box(value.0);
        divan::black_box(i);
    }
}

fn main() {
    divan::main();
}