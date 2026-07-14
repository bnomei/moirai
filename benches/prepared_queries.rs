use divan::{counter::ItemsCount, Bencher};
use moirai::bench_internals::{adhoc_query1_count, adhoc_query2_count};
use moirai::component::ComponentOptions;
use moirai::query::{QueryCursor, QueryPolicy, QuerySpec, QueryWindow};
use moirai::world::WorldBuilder;

fn main() {
    divan::main();
}

#[derive(Clone, Copy)]
struct Position(i32);

#[derive(Clone, Copy)]
struct Velocity(i32);

#[derive(Clone, Copy, Debug)]
enum Layout {
    SparseSparse,
    SparseTable,
    TableSparse,
    TableTable,
}

impl Layout {
    const fn options(self) -> (ComponentOptions, ComponentOptions) {
        match self {
            Self::SparseSparse => (ComponentOptions::sparse(), ComponentOptions::sparse()),
            Self::SparseTable => (ComponentOptions::sparse(), ComponentOptions::table()),
            Self::TableSparse => (ComponentOptions::table(), ComponentOptions::sparse()),
            Self::TableTable => (ComponentOptions::table(), ComponentOptions::table()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Scale {
    Tiny,
    Typical,
}

impl Scale {
    const fn count(self) -> usize {
        match self {
            Self::Tiny => 64,
            Self::Typical => 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Skew {
    FirstDense,
    SecondDense,
}

impl Skew {
    const fn strides(self) -> (usize, usize) {
        match self {
            Self::FirstDense => (1, 8),
            Self::SecondDense => (8, 1),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Policy {
    Prepared,
    Membership,
    DeltaMembership,
    Result,
}

impl Policy {
    const fn query_policy(self) -> QueryPolicy {
        match self {
            Self::Prepared => QueryPolicy::Prepared,
            Self::Membership => QueryPolicy::Membership,
            Self::DeltaMembership => QueryPolicy::DeltaMembership,
            Self::Result => QueryPolicy::Result,
        }
    }
}

const LAYOUTS: [Layout; 4] = [
    Layout::SparseSparse,
    Layout::SparseTable,
    Layout::TableSparse,
    Layout::TableTable,
];
const SCALES: [Scale; 2] = [Scale::Tiny, Scale::Typical];
const SKEWS: [Skew; 2] = [Skew::FirstDense, Skew::SecondDense];
const POLICIES: [Policy; 4] = [
    Policy::Prepared,
    Policy::Membership,
    Policy::DeltaMembership,
    Policy::Result,
];
const TEMPORAL_POLICIES: [Policy; 3] = [
    Policy::Prepared,
    Policy::Membership,
    Policy::DeltaMembership,
];

fn query1_cases() -> impl Iterator<Item = (Layout, Scale, Policy)> {
    LAYOUTS.into_iter().flat_map(|layout| {
        SCALES.into_iter().flat_map(move |scale| {
            POLICIES
                .into_iter()
                .map(move |policy| (layout, scale, policy))
        })
    })
}

fn query2_cases() -> impl Iterator<Item = (Layout, Scale, Skew, Policy)> {
    LAYOUTS.into_iter().flat_map(|layout| {
        SCALES.into_iter().flat_map(move |scale| {
            SKEWS.into_iter().flat_map(move |skew| {
                POLICIES
                    .into_iter()
                    .map(move |policy| (layout, scale, skew, policy))
            })
        })
    })
}

fn temporal_cases() -> impl Iterator<Item = (Layout, Scale, Skew, Policy)> {
    LAYOUTS.into_iter().flat_map(|layout| {
        SCALES.into_iter().flat_map(move |scale| {
            SKEWS.into_iter().flat_map(move |skew| {
                TEMPORAL_POLICIES
                    .into_iter()
                    .map(move |policy| (layout, scale, skew, policy))
            })
        })
    })
}

fn query_world(
    layout: Layout,
    count: usize,
    position_stride: usize,
    velocity_stride: usize,
) -> (moirai::World, Vec<moirai::EntityId>) {
    let (position_options, velocity_options) = layout.options();
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(position_options)
        .expect("position");
    builder
        .register_component::<Velocity>(velocity_options)
        .expect("velocity");
    let mut world = builder.build().expect("world");
    let mut entities = Vec::with_capacity(count);
    for index in 0..count {
        let entity = world.spawn().expect("spawn");
        if index % position_stride == 0 {
            world
                .insert(entity, Position(index as i32))
                .expect("position");
        }
        if index % velocity_stride == 0 {
            world.insert(entity, Velocity(1)).expect("velocity");
        }
        entities.push(entity);
    }
    (world, entities)
}

#[divan::bench(args = query1_cases())]
fn query1_read_all(bencher: Bencher, case: (Layout, Scale, Policy)) {
    let (layout, scale, policy) = case;
    let count = scale.count();
    bencher
        .counter(ItemsCount::new(count))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, 1, 8);
            let query = world
                .prepare_query1::<Position>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            (world, query)
        })
        .bench_local_refs(|(world, query)| {
            let sum = query
                .iter(world, QueryWindow::All)
                .expect("query")
                .map(|(_, position)| position.0 as i64)
                .sum::<i64>();
            divan::black_box(sum);
        });
}

#[divan::bench(args = query2_cases())]
fn query2_read_all_population_matrix(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    let matches = count.div_ceil(position_stride.max(velocity_stride));
    bencher
        .counter(ItemsCount::new(matches))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, position_stride, velocity_stride);
            let query = world
                .prepare_query2::<Position, Velocity>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            (world, query)
        })
        .bench_local_refs(|(world, query)| {
            let sum = query
                .iter(world, QueryWindow::All)
                .expect("query")
                .map(|(_, position, velocity)| (position.0 + velocity.0) as i64)
                .sum::<i64>();
            divan::black_box(sum);
        });
}

#[divan::bench(args = temporal_cases())]
fn query2_since_one_changed(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    bencher
        .counter(ItemsCount::new(1_usize))
        .with_inputs(|| {
            let (mut world, entities) =
                query_world(layout, count, position_stride, velocity_stride);
            let spec = QuerySpec::new().changed::<Position>();
            let query = world
                .prepare_query2::<Position, Velocity>(spec, policy.query_policy())
                .expect("prepare");
            let since = world.change_tick();
            let entity = entities
                .into_iter()
                .find(|entity| {
                    world.get::<Position>(*entity).expect("position").is_some()
                        && world.get::<Velocity>(*entity).expect("velocity").is_some()
                })
                .expect("matching entity");
            world
                .get_mut::<Position>(entity)
                .expect("position")
                .expect("present")
                .0 += 1;
            (world, query, since)
        })
        .bench_local_refs(|(world, query, since)| {
            let count = query
                .iter(world, QueryWindow::Since(*since))
                .expect("query")
                .count();
            assert_eq!(count, 1);
            divan::black_box(count);
        });
}

#[divan::bench(args = temporal_cases())]
fn query2_cursor_empty_window(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    bencher
        .counter(ItemsCount::new(0_usize))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, position_stride, velocity_stride);
            let spec = QuerySpec::new().changed::<Position>();
            let query = world
                .prepare_query2::<Position, Velocity>(spec.clone(), policy.query_policy())
                .expect("prepare");
            let cursor = QueryCursor::from_spec2_now::<Position, Velocity>(&mut world, &spec)
                .expect("cursor");
            (world, query, cursor)
        })
        .bench_local_refs(|(world, query, cursor)| {
            let count = query
                .iter(world, QueryWindow::Cursor(cursor))
                .expect("query")
                .count();
            assert_eq!(count, 0);
            divan::black_box(count);
        });
}

#[divan::bench(args = query1_cases())]
fn query1_mut_all(bencher: Bencher, case: (Layout, Scale, Policy)) {
    let (layout, scale, policy) = case;
    let count = scale.count();
    bencher
        .counter(ItemsCount::new(count))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, 1, 1);
            let mut query = world
                .prepare_query1::<Position>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            query
                .for_each_mut(&mut world, QueryWindow::All, |_, position| {
                    position.0 = position.0.wrapping_add(1);
                    Ok(())
                })
                .expect("warm mutation scratch");
            (world, query)
        })
        .bench_local_refs(|(world, query)| {
            query
                .for_each_mut(world, QueryWindow::All, |_, position| {
                    position.0 = position.0.wrapping_add(1);
                    Ok(())
                })
                .expect("mutate");
        });
}

#[divan::bench(args = query2_cases())]
fn query2_mut_read_all(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    let matches = count.div_ceil(position_stride.max(velocity_stride));
    bencher
        .counter(ItemsCount::new(matches))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, position_stride, velocity_stride);
            let mut query = world
                .prepare_query2::<Position, Velocity>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            query
                .for_each_mut_read(&mut world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    Ok(())
                })
                .expect("warm mutation scratch");
            (world, query)
        })
        .bench_local_refs(|(world, query)| {
            query
                .for_each_mut_read(world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    Ok(())
                })
                .expect("mutate");
        });
}

#[divan::bench(args = query2_cases())]
fn query2_mut_mut_all(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    let matches = count.div_ceil(position_stride.max(velocity_stride));
    bencher
        .counter(ItemsCount::new(matches))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, position_stride, velocity_stride);
            let mut query = world
                .prepare_query2::<Position, Velocity>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            query
                .for_each_mut_mut(&mut world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    velocity.0 = velocity.0.wrapping_neg();
                    Ok(())
                })
                .expect("warm mutation scratch");
            (world, query)
        })
        .bench_local_refs(|(world, query)| {
            query
                .for_each_mut_mut(world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    velocity.0 = velocity.0.wrapping_neg();
                    Ok(())
                })
                .expect("mutate");
        });
}

#[divan::bench(args = query2_cases())]
fn query2_mut_read_effects_all(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    let matches = count.div_ceil(position_stride.max(velocity_stride));
    bencher
        .counter(ItemsCount::new(matches))
        .with_inputs(|| {
            let (mut world, _) = query_world(layout, count, position_stride, velocity_stride);
            let mut query = world
                .prepare_query2::<Position, Velocity>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            query
                .for_each_mut_read_with_effects(
                    &mut world,
                    QueryWindow::All,
                    |_, position, velocity, effects| {
                        position.0 = position.0.wrapping_add(velocity.0);
                        divan::black_box(effects);
                        Ok(())
                    },
                )
                .expect("warm mutation scratch");
            (world, query)
        })
        .bench_local_refs(|(world, query)| {
            query
                .for_each_mut_read_with_effects(
                    world,
                    QueryWindow::All,
                    |_, position, velocity, effects| {
                        position.0 = position.0.wrapping_add(velocity.0);
                        divan::black_box(effects);
                        Ok(())
                    },
                )
                .expect("mutate with effects");
        });
}

#[divan::bench(args = [64_usize, 4_096])]
fn query1_retained_adhoc_control(bencher: Bencher, count: usize) {
    let (mut world, _) = query_world(Layout::SparseSparse, count, 1, 8);
    let spec = QuerySpec::new();
    bencher.counter(ItemsCount::new(count)).bench_local(|| {
        divan::black_box(adhoc_query1_count::<Position>(&mut world, &spec).expect("ad hoc"));
    });
}

#[divan::bench(args = [64_usize, 4_096])]
fn query2_retained_adhoc_control(bencher: Bencher, count: usize) {
    let (mut world, _) = query_world(Layout::SparseSparse, count, 1, 8);
    let spec = QuerySpec::new();
    bencher
        .counter(ItemsCount::new(count.div_ceil(8)))
        .bench_local(|| {
            divan::black_box(
                adhoc_query2_count::<Position, Velocity>(&mut world, &spec).expect("ad hoc"),
            );
        });
}

enum Query1PairedInput {
    Adhoc {
        world: moirai::World,
        spec: QuerySpec,
    },
    Prepared {
        world: moirai::World,
        query: moirai::PreparedQuery1<Position>,
    },
}

#[divan::bench(args = [64_usize, 4_096])]
fn query1_paired_control(bencher: Bencher, count: usize) {
    let mode = std::env::var("MOIRAI_QUERY_CONTROL").unwrap_or_else(|_| String::from("prepared"));
    bencher
        .counter(ItemsCount::new(count))
        .with_inputs(|| {
            let (mut world, _) = query_world(Layout::SparseSparse, count, 1, 8);
            if mode == "adhoc" {
                Query1PairedInput::Adhoc {
                    world,
                    spec: QuerySpec::new(),
                }
            } else {
                let query = world
                    .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Prepared)
                    .expect("prepare");
                Query1PairedInput::Prepared { world, query }
            }
        })
        .bench_local_refs(|input| match input {
            Query1PairedInput::Adhoc { world, spec } => {
                divan::black_box(adhoc_query1_count::<Position>(world, spec).expect("ad hoc"));
            }
            Query1PairedInput::Prepared { world, query } => {
                divan::black_box(query.iter(world, QueryWindow::All).expect("query").count());
            }
        });
}

#[divan::bench(args = query2_cases())]
fn query2_same_workload_membership_churn(bencher: Bencher, case: (Layout, Scale, Skew, Policy)) {
    let (layout, scale, skew, policy) = case;
    let count = scale.count();
    let (position_stride, velocity_stride) = skew.strides();
    bencher
        .counter(ItemsCount::new(
            count.div_ceil(position_stride.max(velocity_stride)),
        ))
        .with_inputs(|| {
            let (mut world, entities) =
                query_world(layout, count, position_stride, velocity_stride);
            let query = world
                .prepare_query2::<Position, Velocity>(QuerySpec::new(), policy.query_policy())
                .expect("prepare");
            let entity = entities
                .into_iter()
                .find(|entity| world.get::<Position>(*entity).expect("position").is_some())
                .expect("position entity");
            let present = world.get::<Velocity>(entity).expect("velocity").is_some();
            (world, query, entity, present)
        })
        .bench_local_refs(|(world, query, entity, present)| {
            if *present {
                world.remove::<Velocity>(*entity).expect("remove");
            } else {
                world.insert(*entity, Velocity(1)).expect("insert");
            }
            *present = !*present;
            divan::black_box(query.iter(world, QueryWindow::All).expect("query").count());
        });
}

#[derive(Clone, Copy, Debug)]
enum DeltaCursorPosition {
    Current,
    Lagging,
}

#[divan::bench(args = [DeltaCursorPosition::Current, DeltaCursorPosition::Lagging])]
fn query1_delta_current_vs_lagging(bencher: Bencher, position: DeltaCursorPosition) {
    const BACKLOG: usize = 4_096;
    let unseen = match position {
        DeltaCursorPosition::Current => 1,
        DeltaCursorPosition::Lagging => BACKLOG + 1,
    };
    bencher
        .counter(ItemsCount::new(unseen))
        .with_inputs(|| {
            let (mut world, entities) = query_world(Layout::SparseSparse, 1, 1, 1);
            let entity = entities[0];
            let mut current = world
                .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::DeltaMembership)
                .expect("current query");
            let lagging = world
                .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::DeltaMembership)
                .expect("lagging query");
            let mut present = true;
            for _ in 0..BACKLOG {
                toggle_position(&mut world, entity, &mut present);
            }
            // Advance only one query. The lagging peer deliberately pins the
            // retained prefix that the current query must skip in O(1).
            current
                .iter(&mut world, QueryWindow::All)
                .expect("advance current query")
                .for_each(|_| {});
            toggle_position(&mut world, entity, &mut present);

            let (target, peer) = match position {
                DeltaCursorPosition::Current => (current, lagging),
                DeltaCursorPosition::Lagging => (lagging, current),
            };
            (world, target, peer, present)
        })
        .bench_local_refs(|(world, target, peer, present)| {
            divan::black_box(peer);
            let count = target
                .iter(world, QueryWindow::All)
                .expect("refresh target")
                .count();
            assert_eq!(count, usize::from(*present));
            divan::black_box(count);
        });
}

fn toggle_position(world: &mut moirai::World, entity: moirai::EntityId, present: &mut bool) {
    if *present {
        world.remove::<Position>(entity).expect("remove position");
    } else {
        world.insert(entity, Position(1)).expect("insert position");
    }
    *present = !*present;
}
