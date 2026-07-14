use core::time::Duration;

use divan::{counter::ItemsCount, Bencher};
use moirai::schedule::{stage, Condition, System};
use moirai::world::WorldBuilder;
use moirai::{AppBuilder, DenseEntityScratch, FixedConfig, Revision, RevisionKey};

fn main() {
    divan::main();
}

#[derive(Clone, Copy)]
struct FrameState(u64);

fn entities(count: usize) -> (moirai::World, Vec<moirai::EntityId>) {
    let mut world = WorldBuilder::new().build().expect("world");
    let ids = (0..count).map(|_| world.spawn().expect("spawn")).collect();
    (world, ids)
}

#[divan::bench(args = [64_usize, 1_024, 16_384])]
fn dense_scratch_insert_get_clear(bencher: Bencher, count: usize) {
    bencher
        .counter(ItemsCount::new(count * 2))
        .with_inputs(|| {
            let (world, ids) = entities(count);
            let scratch = DenseEntityScratch::with_capacity(&world, count);
            (world, ids, scratch)
        })
        .bench_local_refs(|(world, ids, scratch)| {
            for (index, &entity) in ids.iter().enumerate() {
                scratch.insert(world, entity, index).expect("insert");
            }
            for &entity in ids.iter() {
                divan::black_box(scratch.get(world, entity).expect("get"));
            }
            scratch.clear();
        });
}

#[divan::bench(args = [64_usize, 1_024, 16_384])]
fn dense_scratch_retain_sparse_live_set(bencher: Bencher, count: usize) {
    bencher
        .counter(ItemsCount::new(count))
        .with_inputs(|| {
            let (mut world, ids) = entities(count);
            let mut scratch = DenseEntityScratch::with_capacity(&world, count);
            for (index, &entity) in ids.iter().enumerate() {
                scratch.insert(&world, entity, index).expect("insert");
            }
            for &entity in ids.iter().step_by(4) {
                world.despawn(entity).expect("despawn");
            }
            (world, scratch)
        })
        .bench_local_refs(|(world, scratch)| {
            divan::black_box(scratch.retain_live(world).expect("retain"));
        });
}

fn resource_world() -> moirai::World {
    let mut builder = WorldBuilder::new();
    builder.insert_resource(FrameState(1));
    builder.build().expect("world")
}

#[divan::bench]
fn resource_scope_ref_present(bencher: Bencher) {
    bencher
        .with_inputs(resource_world)
        .bench_local_refs(|world| {
            world
                .resource_scope_ref::<FrameState, _>(|state, _| {
                    divan::black_box(state.expect("present").0)
                })
                .expect("scope")
        });
}

#[divan::bench]
fn resource_scope_mut_present(bencher: Bencher) {
    bencher
        .with_inputs(resource_world)
        .bench_local_refs(|world| {
            world
                .resource_scope_mut::<FrameState, _>(|state, _| {
                    let state = state.expect("present");
                    state.0 = state.0.wrapping_add(1);
                })
                .expect("scope")
        });
}

#[derive(Clone, Copy, Debug)]
enum RevisionComparison {
    Equal,
    UnequalEarly,
    UnequalLate,
}

fn revision_comparison_cases() -> impl Iterator<Item = (usize, RevisionComparison)> {
    [1_usize, 8, 64].into_iter().flat_map(|repeats| {
        [
            RevisionComparison::Equal,
            RevisionComparison::UnequalEarly,
            RevisionComparison::UnequalLate,
        ]
        .into_iter()
        .map(move |comparison| (repeats, comparison))
    })
}

#[divan::bench(args = revision_comparison_cases())]
fn revision_key_compare(bencher: Bencher, case: (usize, RevisionComparison)) {
    let (repeats, comparison) = case;
    let mut revision = Revision::ZERO;
    revision.advance().expect("advance");
    let left = RevisionKey::new([revision, Revision::ZERO, revision, Revision::ZERO]);
    let right = match comparison {
        RevisionComparison::Equal => left,
        RevisionComparison::UnequalEarly => {
            RevisionKey::new([Revision::ZERO, Revision::ZERO, revision, Revision::ZERO])
        }
        RevisionComparison::UnequalLate => {
            RevisionKey::new([revision, Revision::ZERO, revision, revision])
        }
    };
    bencher.counter(ItemsCount::new(repeats)).bench_local(|| {
        for _ in 0..repeats {
            let dynamic_left = divan::black_box(left);
            let dynamic_right = divan::black_box(right);
            divan::black_box(dynamic_left == dynamic_right);
        }
    });
}

fn local_state_app() -> moirai::App {
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::with_local(
            "local-state",
            stage::UPDATE,
            |_| Ok(0_u64),
            |_world, _dt, local| {
                *local = local.wrapping_add(1);
                divan::black_box(*local);
                Ok(())
            },
        ))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("warm local app");
    app
}

#[divan::bench]
fn system_local_update(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(1_usize))
        .with_inputs(local_state_app)
        .bench_local_refs(|app| app.update(1.0 / 60.0).expect("update"));
}

fn plain_closure_state_app() -> moirai::App {
    let mut local = 0_u64;
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new(
            "plain-closure-state",
            stage::UPDATE,
            move |_world, _dt| {
                local = local.wrapping_add(1);
                divan::black_box(local);
            },
        ))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("warm plain app");
    app
}

#[divan::bench]
fn system_local_paired_control(bencher: Bencher) {
    let mode =
        std::env::var("MOIRAI_SYSTEM_LOCAL_CONTROL").unwrap_or_else(|_| String::from("local"));
    bencher
        .counter(ItemsCount::new(1_usize))
        .with_inputs(|| {
            if mode == "plain" {
                plain_closure_state_app()
            } else {
                local_state_app()
            }
        })
        .bench_local_refs(|app| app.update(1.0 / 60.0).expect("update"));
}

fn cadenced_app(period: u64) -> moirai::App {
    let mut builder = AppBuilder::new();
    builder
        .add_system(
            System::new("cadenced", stage::UPDATE, |_world, _dt| {
                divan::black_box(());
            })
            .run_if(Condition::fixed_step_mod(period, 0).expect("cadence")),
        )
        .expect("system");
    builder.build().expect("app")
}

#[divan::bench(args = [1_u64, 8, 64])]
fn cadence_condition_outside_fixed_update(bencher: Bencher, period: u64) {
    bencher
        .counter(ItemsCount::new(1_usize))
        .with_inputs(|| cadenced_app(period))
        .bench_local_refs(|app| app.update(1.0 / 60.0).expect("update"));
}

fn fixed_cadenced_app(period: u64) -> moirai::App {
    let mut builder = AppBuilder::new();
    builder
        .schedule_builder()
        .fixed(FixedConfig::new(Duration::from_millis(1)).expect("fixed"));
    builder
        .add_system(
            System::new("fixed-cadenced", stage::FIXED_UPDATE, |_world, _dt| {
                divan::black_box(());
            })
            .run_if(Condition::fixed_step_mod(period, 0).expect("cadence")),
        )
        .expect("system");
    builder.build().expect("app")
}

#[divan::bench(args = [1_u64, 8, 64])]
fn fixed_cadence_condition(bencher: Bencher, period: u64) {
    bencher
        .counter(ItemsCount::new(1_usize))
        .with_inputs(|| fixed_cadenced_app(period))
        .bench_local_refs(|app| app.update(0.001).expect("fixed update"));
}
