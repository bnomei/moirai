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

fn populated_world(count: usize, stride: usize) -> (moirai::world::World, Vec<moirai::EntityId>) {
    let mut world = setup();
    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        let entity = world.spawn().expect("spawn");
        if i % stride == 0 {
            world.insert(entity, BenchComp(i as u32)).expect("insert");
        }
        entities.push(entity);
    }
    (world, entities)
}

// Historical aggregate control. This intentionally includes construction and insertion.
#[divan::bench]
fn sparse_insert_lookup_including_setup() {
    let mut world = setup();
    let mut entities = Vec::new();
    for i in 0..128 {
        let entity = world.spawn().expect("spawn");
        world.insert(entity, BenchComp(i)).expect("insert");
        entities.push(entity);
    }
    for (i, entity) in entities.iter().enumerate() {
        let value = world
            .get::<BenchComp>(*entity)
            .expect("get")
            .expect("present");
        divan::black_box(value.0);
        divan::black_box(i);
    }
}

#[divan::bench(args = [256, 4096])]
fn sparse_lookup_full_occupancy(bencher: divan::Bencher, high_water: usize) {
    let (world, entities) = populated_world(high_water, 1);
    bencher.bench_local(|| {
        let mut sum = 0u32;
        for &entity in &entities {
            if let Some(value) = world.get::<BenchComp>(entity).expect("get") {
                sum = sum.wrapping_add(value.0);
            }
        }
        divan::black_box(sum)
    });
}

#[divan::bench(args = [256, 4096])]
fn sparse_lookup_low_occupancy(bencher: divan::Bencher, high_water: usize) {
    let (world, entities) = populated_world(high_water, 64);
    bencher.bench_local(|| {
        let mut sum = 0u32;
        for &entity in &entities {
            if let Some(value) = world.get::<BenchComp>(entity).expect("get") {
                sum = sum.wrapping_add(value.0);
            }
        }
        divan::black_box(sum)
    });
}

#[divan::bench(args = [256, 4096])]
fn sparse_high_water_first_insert(bencher: divan::Bencher, high_water: usize) {
    bencher
        .with_inputs(|| {
            let mut world = setup();
            let mut target = None;
            for _ in 0..high_water {
                target = Some(world.spawn().expect("spawn"));
            }
            (world, target.expect("non-empty high-water input"))
        })
        .bench_local_refs(|(world, target)| {
            let replaced = world
                .insert(*target, BenchComp(1))
                .expect("high-water insert");
            divan::black_box(replaced)
        });
}

#[divan::bench(args = [256, 4096])]
fn sparse_insert_remove_reuse(bencher: divan::Bencher, high_water: usize) {
    let (mut world, entities) = populated_world(high_water, 1);
    let target = *entities.last().expect("non-empty world");
    bencher.bench_local(|| {
        let value = world
            .remove::<BenchComp>(target)
            .expect("remove")
            .expect("present");
        let replaced = world.insert(target, value).expect("reinsert");
        divan::black_box(replaced)
    });
}

fn main() {
    divan::main();
}
