use moirai::component::ComponentOptions;
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
struct TablePos(i32);

#[derive(Clone, Copy)]
struct Velocity(i32);

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct RetainedA([u8; 32]);

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct RetainedB([u8; 32]);

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct RetainedC([u8; 32]);

#[derive(Clone, Copy, Debug)]
enum MigrationCase {
    OneColumnOneEntity,
    FourColumnsOneEntity,
    FourColumns32Entities,
    FourColumns256Entities,
}

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

fn migration_world(
    count: usize,
    retained_columns: usize,
) -> (moirai::world::World, Vec<moirai::EntityId>) {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register pos");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("register vel");
    builder
        .register_component::<RetainedA>(ComponentOptions::table())
        .expect("register retained a");
    builder
        .register_component::<RetainedB>(ComponentOptions::table())
        .expect("register retained b");
    builder
        .register_component::<RetainedC>(ComponentOptions::table())
        .expect("register retained c");
    let mut world = builder.build().expect("build");
    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        let entity = world.spawn().expect("spawn");
        world
            .insert(entity, TablePos(i as i32))
            .expect("insert pos");
        if retained_columns >= 2 {
            world
                .insert(entity, RetainedA([1; 32]))
                .expect("insert retained a");
        }
        if retained_columns >= 3 {
            world
                .insert(entity, RetainedB([2; 32]))
                .expect("insert retained b");
        }
        if retained_columns >= 4 {
            world
                .insert(entity, RetainedC([3; 32]))
                .expect("insert retained c");
        }
        world
            .insert(entity, Velocity(i as i32))
            .expect("insert trigger");
        entities.push(entity);
    }
    (world, entities)
}

// Historical aggregate controls. These intentionally include setup in their timing.
#[divan::bench]
fn table_insert_get_including_setup() {
    let mut world = setup();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(42)).expect("insert");
    let value = world
        .get::<TablePos>(entity)
        .expect("get")
        .expect("present");
    divan::black_box(value.0);
}

#[divan::bench]
fn archetype_move_insert_second_table_component_including_setup() {
    let mut world = setup();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(1)).expect("insert pos");
    world.insert(entity, Velocity(2)).expect("insert vel");
    let value = world
        .get::<Velocity>(entity)
        .expect("get")
        .expect("present");
    divan::black_box(value.0);
}

#[divan::bench]
fn deferred_command_flush_including_setup() {
    let mut world = setup();
    let entity = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    world
        .commands()
        .expect("commands")
        .insert(entity, TablePos(9))
        .expect("queue");
    world.flush().expect("flush");
    divan::black_box(entity);
}

#[divan::bench]
fn table_get_isolated(bencher: divan::Bencher) {
    let mut world = setup();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(42)).expect("insert");
    bencher.bench_local(|| {
        let value = world
            .get::<TablePos>(entity)
            .expect("get")
            .expect("present");
        divan::black_box(value.0)
    });
}

#[divan::bench]
fn archetype_move_insert_second_table_component_isolated(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut world = setup();
            let entity = world.spawn().expect("spawn");
            world.insert(entity, TablePos(1)).expect("insert pos");
            (world, entity)
        })
        .bench_local_refs(|(world, entity)| {
            let replaced = world
                .insert(*entity, Velocity(2))
                .expect("insert second table component");
            divan::black_box(replaced)
        });
}

#[divan::bench(args = [
    MigrationCase::OneColumnOneEntity,
    MigrationCase::FourColumnsOneEntity,
    MigrationCase::FourColumns32Entities,
    MigrationCase::FourColumns256Entities,
])]
fn archetype_migration_remove_insert_cycle(bencher: divan::Bencher, case: MigrationCase) {
    let (retained_columns, entity_count) = match case {
        MigrationCase::OneColumnOneEntity => (1, 1),
        MigrationCase::FourColumnsOneEntity => (4, 1),
        MigrationCase::FourColumns32Entities => (4, 32),
        MigrationCase::FourColumns256Entities => (4, 256),
    };
    let (mut world, entities) = migration_world(entity_count, retained_columns);

    bencher.bench_local(|| {
        for &entity in &entities {
            let removed = world
                .remove::<Velocity>(entity)
                .expect("remove trigger")
                .expect("trigger present");
            divan::black_box(removed.0);
        }
        for (i, &entity) in entities.iter().enumerate() {
            let replaced = world
                .insert(entity, Velocity(i as i32))
                .expect("reinsert trigger");
            divan::black_box(replaced);
        }
    });
}

fn main() {
    divan::main();
}
