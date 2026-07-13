use moirai::component::{ComponentId, ComponentOptions};
use moirai::query::{ExactIdPolicy, QueryParams, QuerySpec};
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
struct BenchPos(i32);

#[derive(Clone, Copy)]
struct BenchVel(i32);

#[derive(Clone, Copy)]
struct BenchTable(i32);

#[derive(Clone, Copy)]
struct BenchNoise;

#[derive(Clone, Copy)]
struct BenchNever;

#[derive(Clone, Copy)]
struct ArchetypeA;

#[derive(Clone, Copy)]
struct ArchetypeB;

#[derive(Clone, Copy)]
struct ArchetypeC;

#[derive(Clone, Copy)]
struct ArchetypeD;

#[derive(Clone, Copy)]
struct ArchetypeE;

macro_rules! selector_noise_types {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy)]
            struct $name;
        )+

        fn register_selector_noise(builder: &mut WorldBuilder) -> Vec<ComponentId> {
            vec![
                $(
                    builder
                        .register_component::<$name>(ComponentOptions::sparse())
                        .expect("register selector noise"),
                )+
            ]
        }
    };
}

selector_noise_types!(
    SelectorNoise00,
    SelectorNoise01,
    SelectorNoise02,
    SelectorNoise03,
    SelectorNoise04,
    SelectorNoise05,
    SelectorNoise06,
    SelectorNoise07,
    SelectorNoise08,
    SelectorNoise09,
    SelectorNoise10,
    SelectorNoise11,
    SelectorNoise12,
    SelectorNoise13,
    SelectorNoise14,
    SelectorNoise15,
    SelectorNoise16,
    SelectorNoise17,
    SelectorNoise18,
    SelectorNoise19,
    SelectorNoise20,
    SelectorNoise21,
    SelectorNoise22,
    SelectorNoise23,
    SelectorNoise24,
    SelectorNoise25,
    SelectorNoise26,
    SelectorNoise27,
    SelectorNoise28,
    SelectorNoise29,
    SelectorNoise30,
    SelectorNoise31,
);

fn sparse_world(count: usize) -> moirai::world::World {
    sparse_world_inner(count, false)
}

fn sparse_world_with_noise(count: usize) -> moirai::world::World {
    sparse_world_inner(count, true)
}

fn sparse_world_inner(count: usize, register_noise: bool) -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchPos>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<BenchVel>(ComponentOptions::sparse())
        .expect("register");
    if register_noise {
        builder
            .register_component::<BenchNoise>(ComponentOptions::sparse())
            .expect("register noise");
    }
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

fn query2_selector_world() -> (moirai::world::World, Vec<ComponentId>) {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchPos>(ComponentOptions::sparse())
        .expect("register pos");
    builder
        .register_component::<BenchVel>(ComponentOptions::sparse())
        .expect("register vel");
    let selector_ids = register_selector_noise(&mut builder);
    (builder.build().expect("build"), selector_ids)
}

fn empty_table_world(archetype_count: usize) -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<BenchNever>(ComponentOptions::table())
        .expect("register missing table");
    builder
        .register_component::<ArchetypeA>(ComponentOptions::table())
        .expect("register archetype a");
    builder
        .register_component::<ArchetypeB>(ComponentOptions::table())
        .expect("register archetype b");
    builder
        .register_component::<ArchetypeC>(ComponentOptions::table())
        .expect("register archetype c");
    builder
        .register_component::<ArchetypeD>(ComponentOptions::table())
        .expect("register archetype d");
    builder
        .register_component::<ArchetypeE>(ComponentOptions::table())
        .expect("register archetype e");
    let mut world = builder.build().expect("build");

    assert!(
        archetype_count <= 31,
        "five components encode 31 archetypes"
    );
    for mask in 1..=archetype_count {
        let entity = world.spawn().expect("spawn archetype entity");
        if mask & 1 != 0 {
            world.insert(entity, ArchetypeA).expect("insert a");
        }
        if mask & 2 != 0 {
            world.insert(entity, ArchetypeB).expect("insert b");
        }
        if mask & 4 != 0 {
            world.insert(entity, ArchetypeC).expect("insert c");
        }
        if mask & 8 != 0 {
            world.insert(entity, ArchetypeD).expect("insert d");
        }
        if mask & 16 != 0 {
            world.insert(entity, ArchetypeE).expect("insert e");
        }
    }
    world
}

// Historical aggregate controls. These intentionally include world construction and warmup.
#[divan::bench]
fn cold_query1_sparse_resolve_including_setup() {
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
fn warm_query1_sparse_including_setup() {
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
fn warm_query2_sparse_including_setup() {
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
fn warm_query_cache_hit_including_setup() {
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
fn closure_mutation_sparse_including_setup() {
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
fn mixed_query2_warm_including_setup() {
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

#[divan::bench(args = [1, 4, 16, 64, 256])]
fn exact_id_query_construction(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let ids = world
        .query_ids(&QuerySpec::new(), QueryParams::new())
        .expect("ids")
        .collect();
    let spec = QuerySpec::new().exact_ids(ids, ExactIdPolicy::SkipUnavailable);
    let _ = world
        .query::<BenchPos>(&spec, QueryParams::new())
        .expect("warm exact plan");

    bencher.bench_local(|| {
        let query = world
            .query::<BenchPos>(divan::black_box(&spec), QueryParams::new())
            .expect("exact query");
        divan::black_box(query);
    });
}

#[divan::bench(args = [1, 4, 16, 64, 256])]
fn exact_id_query_full_exhaustion(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let ids = world
        .query_ids(&QuerySpec::new(), QueryParams::new())
        .expect("ids")
        .collect();
    let spec = QuerySpec::new().exact_ids(ids, ExactIdPolicy::SkipUnavailable);
    let _ = world
        .query::<BenchPos>(&spec, QueryParams::new())
        .expect("warm exact plan");

    bencher.bench_local(|| {
        let sum: i32 = world
            .query::<BenchPos>(divan::black_box(&spec), QueryParams::new())
            .expect("exact query")
            .map(|(_, pos)| pos.0)
            .sum();
        divan::black_box(sum)
    });
}

#[divan::bench(args = [0, 1, 16, 256, 4096])]
fn query_ids_result_cache_hit(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let spec = QuerySpec::new();
    let cache = world
        .build_entity_query_result_cache(spec.clone())
        .expect("result cache");
    let _ = world
        .query_ids(&spec, QueryParams::new().result_cache(&cache))
        .expect("warm result cache")
        .count();

    bencher.bench_local(|| {
        let count = world
            .query_ids(&spec, QueryParams::new().result_cache(&cache))
            .expect("cached ids")
            .count();
        divan::black_box(count)
    });
}

#[divan::bench(args = [0, 1, 64, 4096])]
fn query1_sparse_scan_isolated(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let spec = QuerySpec::new();
    let _ = world
        .query::<BenchPos>(&spec, QueryParams::new())
        .expect("warm plan")
        .count();

    bencher.bench_local(|| {
        let sum: i32 = world
            .query::<BenchPos>(&spec, QueryParams::new())
            .expect("query")
            .map(|(_, pos)| pos.0)
            .sum();
        divan::black_box(sum)
    });
}

#[divan::bench(args = [0, 1, 64, 4096])]
fn query2_sparse_probe_isolated(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let spec = QuerySpec::new();
    let _ = world
        .query2::<BenchPos, BenchVel>(&spec, QueryParams::new())
        .expect("warm plan")
        .count();

    bencher.bench_local(|| {
        let sum: i32 = world
            .query2::<BenchPos, BenchVel>(&spec, QueryParams::new())
            .expect("query2")
            .map(|(_, pos, vel)| pos.0 + vel.0)
            .sum();
        divan::black_box(sum)
    });
}

#[divan::bench(args = [0, 1, 64, 4096])]
fn query2_mixed_probe_isolated(bencher: divan::Bencher, count: usize) {
    let mut world = mixed_world(count);
    let spec = QuerySpec::new();
    let _ = world
        .query2::<BenchPos, BenchTable>(&spec, QueryParams::new())
        .expect("warm plan")
        .count();

    bencher.bench_local(|| {
        let sum: i32 = world
            .query2::<BenchPos, BenchTable>(&spec, QueryParams::new())
            .expect("query2")
            .map(|(_, pos, table)| pos.0 + table.0)
            .sum();
        divan::black_box(sum)
    });
}

#[divan::bench(args = [0, 8, 32])]
fn query2_empty_warm_plan(bencher: divan::Bencher, selector_count: usize) {
    let (mut world, selector_ids) = query2_selector_world();
    assert!(selector_count <= selector_ids.len());
    let mut spec = QuerySpec::new();
    for selector in selector_ids.into_iter().take(selector_count) {
        spec = spec.without_id(selector);
    }
    let _ = world
        .query2::<BenchPos, BenchVel>(&spec, QueryParams::new())
        .expect("warm plan")
        .count();

    bencher.bench_local(|| {
        let count = world
            .query2::<BenchPos, BenchVel>(divan::black_box(&spec), QueryParams::new())
            .expect("query2")
            .count();
        divan::black_box(count)
    });
}

#[divan::bench(args = [256, 4096])]
fn membership_cache_stable_hit(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<BenchPos>(spec.clone())
        .expect("cache");
    let _ = world
        .query::<BenchPos>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("warm cache")
        .count();

    bencher.bench_local(|| {
        let count = world
            .query::<BenchPos>(&spec, QueryParams::new().membership_cache(&cache))
            .expect("cached query")
            .count();
        divan::black_box(count)
    });
}

#[divan::bench(args = [256, 4096])]
fn membership_cache_unrelated_invalidation(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world_with_noise(count);
    let churn = world.spawn().expect("churn entity");
    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<BenchPos>(spec.clone())
        .expect("cache");
    let mut present = false;

    bencher.bench_local(|| {
        if present {
            let _ = world.remove::<BenchNoise>(churn).expect("remove noise");
        } else {
            let _ = world.insert(churn, BenchNoise).expect("insert noise");
        }
        present = !present;
        let count = world
            .query::<BenchPos>(&spec, QueryParams::new().membership_cache(&cache))
            .expect("cached query")
            .count();
        divan::black_box(count)
    });
}

#[divan::bench(args = [256, 4096])]
fn membership_cache_relevant_invalidation(bencher: divan::Bencher, count: usize) {
    let mut world = sparse_world(count);
    let churn = world.spawn().expect("churn entity");
    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<BenchPos>(spec.clone())
        .expect("cache");
    let mut present = false;

    bencher.bench_local(|| {
        if present {
            let _ = world.remove::<BenchPos>(churn).expect("remove position");
        } else {
            let _ = world
                .insert(churn, BenchPos(count as i32))
                .expect("insert position");
        }
        present = !present;
        let count = world
            .query::<BenchPos>(&spec, QueryParams::new().membership_cache(&cache))
            .expect("cached query")
            .count();
        divan::black_box(count)
    });
}

#[divan::bench(args = [0, 4, 16])]
fn empty_table_cache_warm_hit(bencher: divan::Bencher, archetype_count: usize) {
    let mut world = empty_table_world(archetype_count);
    let spec = QuerySpec::new();
    let _ = world
        .query::<BenchNever>(&spec, QueryParams::new())
        .expect("warm empty table query")
        .count();

    bencher.bench_local(|| {
        let count = world
            .query::<BenchNever>(&spec, QueryParams::new())
            .expect("empty table query")
            .count();
        divan::black_box(count)
    });
}

fn main() {
    divan::main();
}
