use moirai::component::ComponentOptions;
use moirai::world::{World, WorldBuilder};

#[derive(Clone, Copy)]
struct BenchComponent;

const BATCH_SIZES: [usize; 4] = [1, 8, 128, 2_048];

fn command_world(live_entities: usize) -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchComponent>(ComponentOptions::sparse())
        .expect("register benchmark component");
    let mut world = builder.build().expect("build command world");
    for _ in 0..live_entities {
        let _ = world.spawn().expect("spawn live entity");
    }
    world
}

fn queue_reserved_spawns(world: &mut World, count: usize) {
    let mut commands = world.commands().expect("commands");
    for _ in 0..count {
        let _ = commands.spawn().expect("reserve entity");
    }
}

fn queue_insert_payloads(world: &mut World, count: usize) {
    let entity = world.spawn().expect("spawn insert target");
    let mut commands = world.commands().expect("commands");
    for _ in 0..count {
        commands
            .insert(entity, BenchComponent)
            .expect("queue insert");
    }
}

/// Queues exactly `op_count` valid operations while cycling through all four
/// structural command forms. Complete groups leave the world unchanged after
/// a successful flush, which makes them suitable for steady-state reuse cases.
fn queue_mixed_ops(world: &mut World, op_count: usize) {
    let mut commands = world.commands().expect("commands");
    let mut queued = 0;
    while queued < op_count {
        let entity = commands.spawn().expect("queue spawn");
        queued += 1;
        if queued == op_count {
            break;
        }
        commands
            .insert(entity, BenchComponent)
            .expect("queue insert");
        queued += 1;
        if queued == op_count {
            break;
        }
        commands
            .remove::<BenchComponent>(entity)
            .expect("queue remove");
        queued += 1;
        if queued == op_count {
            break;
        }
        commands.despawn(entity).expect("queue despawn");
        queued += 1;
    }
}

fn queue_balanced_entities(world: &mut World, entity_count: usize) {
    queue_mixed_ops(world, entity_count.saturating_mul(4));
}

fn queue_spawn_despawn_entities(world: &mut World, entity_count: usize) {
    let mut commands = world.commands().expect("commands");
    for _ in 0..entity_count {
        let entity = commands.spawn().expect("queue spawn");
        commands.despawn(entity).expect("queue despawn");
    }
}

#[divan::bench(args = BATCH_SIZES)]
fn discard_reserved(bencher: divan::Bencher<'_, '_>, reserved: usize) {
    bencher
        .with_inputs(|| {
            let mut world = command_world(0);
            queue_reserved_spawns(&mut world, reserved);
            world
        })
        .bench_local_refs(|world| {
            world.discard_commands().expect("discard commands");
            divan::black_box(world);
        });
}

#[divan::bench(args = BATCH_SIZES)]
fn discard_insert_payloads(bencher: divan::Bencher<'_, '_>, inserts: usize) {
    bencher
        .with_inputs(|| {
            let mut world = command_world(0);
            queue_insert_payloads(&mut world, inserts);
            world
        })
        .bench_local_refs(|world| {
            world.discard_commands().expect("discard commands");
            divan::black_box(world);
        });
}

#[divan::bench(
    args = [
        (0, 1),
        (0, 8),
        (0, 128),
        (0, 2_048),
        (64, 1),
        (64, 8),
        (64, 128),
        (64, 2_048),
        (1_024, 1),
        (1_024, 8),
        (1_024, 128),
        (1_024, 2_048),
        (16_384, 1),
        (16_384, 8),
        (16_384, 128),
        (16_384, 2_048),
    ]
)]
fn preflight_and_flush_mixed(
    bencher: divan::Bencher<'_, '_>,
    (live_entities, queued_ops): (usize, usize),
) {
    bencher
        .with_inputs(|| {
            let mut world = command_world(live_entities);
            queue_mixed_ops(&mut world, queued_ops);
            world
        })
        .bench_local_refs(|world| {
            let report = world.flush().expect("flush mixed commands");
            divan::black_box(report);
        });
}

#[divan::bench(args = BATCH_SIZES)]
fn failed_preflight_cleanup_reserved(bencher: divan::Bencher<'_, '_>, reserved: usize) {
    bencher
        .with_inputs(|| {
            let mut world = command_world(0);
            let target = world.spawn().expect("spawn failure target");
            let mut commands = world.commands().expect("commands");
            for _ in 0..reserved {
                let _ = commands.spawn().expect("reserve entity");
            }
            commands.despawn(target).expect("queue despawn");
            commands
                .insert(target, BenchComponent)
                .expect("queue invalid-after-despawn insert");
            world
        })
        .bench_local_refs(|world| {
            let error = world.flush().expect_err("preflight must reject batch");
            divan::black_box(error);
        });
}

#[divan::bench(args = BATCH_SIZES)]
fn successful_flush_steady_spawn_despawn(bencher: divan::Bencher<'_, '_>, entities: usize) {
    let mut world = command_world(0);
    queue_spawn_despawn_entities(&mut world, entities);
    world.flush().expect("warm command storage");

    bencher.bench_local(|| {
        queue_spawn_despawn_entities(&mut world, entities);
        let report = world.flush().expect("steady spawn/despawn flush");
        divan::black_box(report);
    });
}

#[divan::bench(args = BATCH_SIZES)]
fn successful_flush_steady_mixed(bencher: divan::Bencher<'_, '_>, entities: usize) {
    let mut world = command_world(0);
    queue_balanced_entities(&mut world, entities);
    world.flush().expect("warm command storage");

    bencher.bench_local(|| {
        queue_balanced_entities(&mut world, entities);
        let report = world.flush().expect("steady flush");
        divan::black_box(report);
    });
}

#[divan::bench(args = [1, 8, 128])]
fn successful_flush_after_16k_op_burst(bencher: divan::Bencher<'_, '_>, entities: usize) {
    let mut world = command_world(0);
    queue_mixed_ops(&mut world, 16_384);
    world.flush().expect("flush burst");

    bencher.bench_local(|| {
        queue_spawn_despawn_entities(&mut world, entities);
        let report = world.flush().expect("post-burst flush");
        divan::black_box(report);
    });
}

fn main() {
    divan::main();
}
