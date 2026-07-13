use moirai::component::ComponentOptions;
use moirai::query::{QueryParams, QuerySpec};
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
struct BenchPos(i32);

#[derive(Clone, Copy)]
struct BenchVel(i32);

#[derive(Clone, Copy)]
struct BenchTable(i32);

fn sparse_world(count: usize) -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchPos>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<BenchVel>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    for i in 0..count {
        let entity = world.spawn().expect("spawn");
        world.insert(entity, BenchPos(i as i32)).expect("insert");
        if i % 2 == 0 {
            world.insert(entity, BenchVel(i as i32)).expect("vel");
        }
    }
    world
}

fn mixed_world(count: usize) -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchPos>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<BenchTable>(ComponentOptions::table())
        .expect("register");
    let mut world = builder.build().expect("build");
    for i in 0..count {
        let entity = world.spawn().expect("spawn");
        world.insert(entity, BenchPos(i as i32)).expect("insert");
        if i % 3 == 0 {
            world.insert(entity, BenchTable(i as i32)).expect("table");
        }
    }
    world
}

#[divan::bench]
fn cold_query1_sparse_resolve() {
    let mut world = sparse_world(64);
    let spec = QuerySpec::new().without::<BenchVel>();
    let mut sum = 0i32;
    for _ in 0..32 {
        for (_, pos) in world
            .query::<BenchPos>(&spec, QueryParams::new())
            .expect("query")
        {
            sum += pos.0;
        }
    }
    divan::black_box(sum);
}

#[divan::bench]
fn warm_query1_sparse() {
    let mut world = sparse_world(256);
    let spec = QuerySpec::new();
    for _ in 0..8 {
        for (_, pos) in world
            .query::<BenchPos>(&spec, QueryParams::new())
            .expect("query")
        {
            divan::black_box(pos.0);
        }
    }
    for _ in 0..128 {
        for (_, pos) in world
            .query::<BenchPos>(&spec, QueryParams::new())
            .expect("query")
        {
            divan::black_box(pos.0);
        }
    }
}

#[divan::bench]
fn warm_query2_sparse() {
    let mut world = sparse_world(256);
    let spec = QuerySpec::new();
    for _ in 0..8 {
        for (_, pos, vel) in world
            .query2::<BenchPos, BenchVel>(&spec, QueryParams::new())
            .expect("query2")
        {
            divan::black_box((pos.0, vel.0));
        }
    }
    for _ in 0..128 {
        for (_, pos, vel) in world
            .query2::<BenchPos, BenchVel>(&spec, QueryParams::new())
            .expect("query2")
        {
            divan::black_box((pos.0, vel.0));
        }
    }
}

#[divan::bench]
fn warm_query_cache_hit() {
    let mut world = sparse_world(256);
    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<BenchPos>(spec.clone())
        .expect("cache");
    for _ in 0..8 {
        for (_, pos) in world
            .query::<BenchPos>(&spec, QueryParams::new().membership_cache(&cache))
            .expect("query")
        {
            divan::black_box(pos.0);
        }
    }
    for _ in 0..128 {
        for (_, pos) in world
            .query::<BenchPos>(&spec, QueryParams::new().membership_cache(&cache))
            .expect("query")
        {
            divan::black_box(pos.0);
        }
    }
}

#[divan::bench]
fn closure_mutation_sparse() {
    let mut world = sparse_world(256);
    let spec = QuerySpec::new();
    for _ in 0..8 {
        world
            .for_each_mut::<BenchPos>(&spec, QueryParams::new(), |_entity, pos| {
                pos.0 += 1;
                Ok(())
            })
            .expect("mut");
    }
    for _ in 0..64 {
        world
            .for_each_mut::<BenchPos>(&spec, QueryParams::new(), |_entity, pos| {
                pos.0 += 1;
                Ok(())
            })
            .expect("mut");
    }
}

#[divan::bench]
fn mixed_query2_warm() {
    let mut world = mixed_world(256);
    let spec = QuerySpec::new();
    for _ in 0..64 {
        for (_, pos, table) in world
            .query2::<BenchPos, BenchTable>(&spec, QueryParams::new())
            .expect("query2")
        {
            divan::black_box((pos.0, table.0));
        }
    }
}

fn main() {
    divan::main();
}
