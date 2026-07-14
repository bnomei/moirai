use core::fmt;
use core::time::Duration;

use divan::{counter::ItemsCount, Bencher};
use moirai::component::ComponentOptions;
use moirai::query::{PreparedQuery2, QueryPolicy, QuerySpec, QueryWindow};
use moirai::schedule::{stage, Condition, System};
use moirai::world::WorldBuilder;
use moirai::{AppBuilder, DenseEntityScratch, EntityId, FixedConfig, Revision, RevisionKey, World};

fn main() {
    divan::main();
}

#[derive(Clone, Copy)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct CollisionCandidate {
    cell: u32,
    speed_squared: f32,
}

#[derive(Clone, Copy, Debug)]
enum PairLayout {
    SparseSparse,
    TableSparse,
    TableTable,
}

impl PairLayout {
    const fn first(self) -> ComponentOptions {
        match self {
            Self::SparseSparse => ComponentOptions::sparse(),
            Self::TableSparse | Self::TableTable => ComponentOptions::table(),
        }
    }

    const fn second(self) -> ComponentOptions {
        match self {
            Self::SparseSparse | Self::TableSparse => ComponentOptions::sparse(),
            Self::TableTable => ComponentOptions::table(),
        }
    }
}

impl fmt::Display for PairLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SparseSparse => f.write_str("sparse_sparse"),
            Self::TableSparse => f.write_str("table_sparse"),
            Self::TableTable => f.write_str("table_table"),
        }
    }
}

#[derive(Clone, Copy)]
struct AsteroidCase {
    entities: usize,
    velocity_stride: usize,
    layout: PairLayout,
    churn: usize,
}

impl AsteroidCase {
    const fn active(self) -> usize {
        self.entities.div_ceil(self.velocity_stride)
    }
}

impl fmt::Display for AsteroidCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}_entities_1_in_{}_active_{}_{}_churn",
            self.entities, self.velocity_stride, self.layout, self.churn
        )
    }
}

const ASTEROID_STEADY_CASES: [AsteroidCase; 6] = [
    AsteroidCase {
        entities: 1_024,
        velocity_stride: 1,
        layout: PairLayout::SparseSparse,
        churn: 0,
    },
    AsteroidCase {
        entities: 4_096,
        velocity_stride: 4,
        layout: PairLayout::SparseSparse,
        churn: 0,
    },
    AsteroidCase {
        entities: 1_024,
        velocity_stride: 1,
        layout: PairLayout::TableSparse,
        churn: 0,
    },
    AsteroidCase {
        entities: 4_096,
        velocity_stride: 4,
        layout: PairLayout::TableSparse,
        churn: 0,
    },
    AsteroidCase {
        entities: 1_024,
        velocity_stride: 1,
        layout: PairLayout::TableTable,
        churn: 0,
    },
    AsteroidCase {
        entities: 4_096,
        velocity_stride: 4,
        layout: PairLayout::TableTable,
        churn: 0,
    },
];

const ASTEROID_CHURN_CASES: [AsteroidCase; 3] = [
    AsteroidCase {
        entities: 4_096,
        velocity_stride: 2,
        layout: PairLayout::SparseSparse,
        churn: 64,
    },
    AsteroidCase {
        entities: 4_096,
        velocity_stride: 2,
        layout: PairLayout::TableSparse,
        churn: 64,
    },
    AsteroidCase {
        entities: 4_096,
        velocity_stride: 2,
        layout: PairLayout::TableTable,
        churn: 64,
    },
];

struct AsteroidInput {
    world: World,
    movement: PreparedQuery2<Position, Velocity>,
    collision: DenseEntityScratch<CollisionCandidate>,
    candidates: Vec<(EntityId, CollisionCandidate)>,
    churn_entities: Vec<(EntityId, Velocity)>,
}

impl AsteroidInput {
    fn new(case: AsteroidCase, policy: QueryPolicy) -> Self {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(case.layout.first())
            .expect("register position");
        builder
            .register_component::<Velocity>(case.layout.second())
            .expect("register velocity");
        let mut world = builder.build().expect("build asteroid world");
        let mut churn_entities = Vec::with_capacity(case.churn);

        for index in 0..case.entities {
            let entity = world.spawn().expect("spawn asteroid");
            let velocity = velocity_for_index(index);
            world
                .insert(
                    entity,
                    Position {
                        x: index as f32 * 0.125,
                        y: (index & 255) as f32 * 0.25,
                    },
                )
                .expect("insert position");
            if index % case.velocity_stride == 0 {
                world.insert(entity, velocity).expect("insert velocity");
            }
            if churn_entities.len() < case.churn {
                churn_entities.push((entity, velocity));
            }
        }

        let movement = world
            .prepare_query2::<Position, Velocity>(QuerySpec::new(), policy)
            .expect("prepare movement query");
        let collision = DenseEntityScratch::with_capacity(&world, case.active());
        let mut input = Self {
            world,
            movement,
            collision,
            candidates: Vec::with_capacity(case.active()),
            churn_entities,
        };
        input.simulate();
        if matches!(policy, QueryPolicy::DeltaMembership) {
            input.toggle_velocity_membership();
            input.simulate();
            input.toggle_velocity_membership();
            input.simulate();
        }
        input
    }

    fn toggle_velocity_membership(&mut self) {
        for &(entity, velocity) in &self.churn_entities {
            if self
                .world
                .get::<Velocity>(entity)
                .expect("inspect velocity")
                .is_some()
            {
                self.world
                    .remove::<Velocity>(entity)
                    .expect("remove velocity");
            } else {
                self.world
                    .insert(entity, velocity)
                    .expect("restore velocity");
            }
        }
    }

    fn simulate(&mut self) {
        const DT: f32 = 1.0 / 64.0;

        self.candidates.clear();
        self.collision.clear();
        self.movement
            .for_each_mut_read(
                &mut self.world,
                QueryWindow::All,
                |entity, position, velocity| {
                    position.x += velocity.x * DT;
                    position.y += velocity.y * DT;
                    let candidate = CollisionCandidate {
                        cell: collision_cell(*position),
                        speed_squared: velocity.x * velocity.x + velocity.y * velocity.y,
                    };
                    self.candidates.push((entity, candidate));
                    Ok(())
                },
            )
            .expect("run movement query");

        let mut checksum = 0.0_f32;
        for &(entity, candidate) in &self.candidates {
            self.collision
                .insert(&self.world, entity, candidate)
                .expect("stage collision candidate");
            let staged = self
                .collision
                .get(&self.world, entity)
                .expect("read collision candidate")
                .expect("candidate present");
            checksum += staged.speed_squared + staged.cell as f32;
        }
        divan::black_box(checksum);
    }
}

fn velocity_for_index(index: usize) -> Velocity {
    Velocity {
        x: ((index & 15) as f32 - 7.0) * 0.03125,
        y: (((index >> 4) & 15) as f32 - 7.0) * 0.03125,
    }
}

fn collision_cell(position: Position) -> u32 {
    let x = (position.x as i32 as u32) & 0xffff;
    let y = (position.y as i32 as u32) & 0xffff;
    x | (y << 16)
}

#[divan::bench(args = ASTEROID_STEADY_CASES)]
fn asteroid_movement_collision_steady(bencher: Bencher<'_, '_>, case: AsteroidCase) {
    bencher
        .counter(ItemsCount::new(case.active()))
        .with_inputs(|| AsteroidInput::new(case, QueryPolicy::Prepared))
        .bench_local_refs(AsteroidInput::simulate);
}

#[divan::bench(args = ASTEROID_CHURN_CASES)]
fn asteroid_movement_collision_membership_churn(bencher: Bencher<'_, '_>, case: AsteroidCase) {
    bencher
        .counter(ItemsCount::new(case.active()))
        .with_inputs(|| AsteroidInput::new(case, QueryPolicy::DeltaMembership))
        .bench_local_refs(|input| {
            input.toggle_velocity_membership();
            input.simulate();
        });
}

#[derive(Clone, Copy)]
struct GrassBlade {
    bend: f32,
    tip: f32,
}

#[derive(Clone, Copy)]
struct WindLayer {
    low: f32,
    high: f32,
}

#[derive(Clone, Copy)]
struct GrassEpoch {
    terrain: Revision,
    wind: Revision,
}

impl GrassEpoch {
    fn key(self) -> RevisionKey<2> {
        RevisionKey::new([self.terrain, self.wind])
    }
}

#[derive(Clone, Copy)]
struct BladeCache {
    key: RevisionKey<2>,
    coefficient: f32,
}

#[derive(Clone, Copy)]
struct GrassCase {
    entities: usize,
    visible_stride: usize,
    period: usize,
    phase: usize,
    layout: PairLayout,
}

impl GrassCase {
    const fn visible(self) -> usize {
        self.entities.div_ceil(self.visible_stride)
    }
}

impl fmt::Display for GrassCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}_entities_1_in_{}_visible_period_{}_phase_{}_{}",
            self.entities, self.visible_stride, self.period, self.phase, self.layout
        )
    }
}

const GRASS_CASES: [GrassCase; 6] = [
    GrassCase {
        entities: 1_024,
        visible_stride: 1,
        period: 1,
        phase: 0,
        layout: PairLayout::SparseSparse,
    },
    GrassCase {
        entities: 4_096,
        visible_stride: 4,
        period: 4,
        phase: 0,
        layout: PairLayout::SparseSparse,
    },
    GrassCase {
        entities: 1_024,
        visible_stride: 1,
        period: 2,
        phase: 1,
        layout: PairLayout::TableSparse,
    },
    GrassCase {
        entities: 4_096,
        visible_stride: 8,
        period: 8,
        phase: 3,
        layout: PairLayout::TableSparse,
    },
    GrassCase {
        entities: 1_024,
        visible_stride: 2,
        period: 4,
        phase: 2,
        layout: PairLayout::TableTable,
    },
    GrassCase {
        entities: 4_096,
        visible_stride: 16,
        period: 16,
        phase: 7,
        layout: PairLayout::TableTable,
    },
];

struct GrassInput {
    world: World,
    update: PreparedQuery2<GrassBlade, WindLayer>,
    visible: Vec<EntityId>,
    cache: DenseEntityScratch<BladeCache>,
    coefficients: Vec<f32>,
    period: usize,
    phase: usize,
}

impl GrassInput {
    fn new(case: GrassCase) -> Self {
        assert!(case.period.is_power_of_two());
        assert!(case.phase < case.period);

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<GrassBlade>(case.layout.first())
            .expect("register blade");
        builder
            .register_component::<WindLayer>(case.layout.second())
            .expect("register wind layer");
        builder.insert_resource(GrassEpoch {
            terrain: Revision::ZERO,
            wind: Revision::ZERO,
        });
        let mut world = builder.build().expect("build grass world");
        let mut visible = Vec::with_capacity(case.visible());

        for index in 0..case.entities {
            let entity = world.spawn().expect("spawn blade");
            world
                .insert(
                    entity,
                    GrassBlade {
                        bend: (index & 31) as f32 * 0.015625,
                        tip: 0.0,
                    },
                )
                .expect("insert blade");
            if index % case.visible_stride == 0 {
                world
                    .insert(
                        entity,
                        WindLayer {
                            low: ((index & 7) as f32 + 1.0) * 0.0078125,
                            high: (((index >> 3) & 7) as f32 + 1.0) * 0.00390625,
                        },
                    )
                    .expect("insert wind layer");
                visible.push(entity);
            }
        }

        let update = world
            .prepare_query2::<GrassBlade, WindLayer>(QuerySpec::new(), QueryPolicy::Membership)
            .expect("prepare grass query");
        let mut cache = DenseEntityScratch::with_capacity(&world, visible.len());
        let key = RevisionKey::new([Revision::ZERO, Revision::ZERO]);
        let mut coefficients = vec![0.0; visible.len()];
        for (index, &entity) in visible.iter().enumerate() {
            let coefficient = coefficient_for_index(index);
            cache
                .insert(&world, entity, BladeCache { key, coefficient })
                .expect("prime blade cache");
            coefficients[index] = coefficient;
        }

        let mut input = Self {
            world,
            update,
            visible,
            cache,
            coefficients,
            period: case.period,
            phase: case.phase,
        };
        input.run_cadence_batch(false);
        input.run_cadence_batch(true);
        input
    }

    fn run_cadence_batch(&mut self, invalidate_cache: bool) {
        for frame in 0..self.period {
            if frame & (self.period - 1) != self.phase {
                continue;
            }

            if invalidate_cache {
                self.world
                    .resource_scope_mut::<GrassEpoch, _>(|epoch, _| {
                        epoch
                            .expect("grass epoch resource")
                            .wind
                            .advance()
                            .expect("wind revision available");
                    })
                    .expect("advance grass epoch");
            }
            let key = self
                .world
                .resource_scope_ref::<GrassEpoch, _>(|epoch, _| {
                    epoch.expect("grass epoch resource").key()
                })
                .expect("read grass epoch");

            for (index, &entity) in self.visible.iter().enumerate() {
                let cached = self
                    .cache
                    .get_or_insert_with(&self.world, entity, || BladeCache {
                        key,
                        coefficient: coefficient_for_index(index),
                    })
                    .expect("access blade cache");
                if cached.key != key {
                    cached.key = key;
                    cached.coefficient = coefficient_for_index(index) * 1.015625;
                }
                self.coefficients[index] = cached.coefficient;
            }

            let coefficients = &self.coefficients;
            let mut checksum = 0.0_f32;
            let mut visible_index = 0;
            self.update
                .for_each_mut_read(&mut self.world, QueryWindow::All, |_entity, blade, wind| {
                    let coefficient = coefficients[visible_index];
                    visible_index += 1;
                    let low_layer = blade.bend * 0.9375 + wind.low * coefficient;
                    let high_layer = wind.high * (1.0 - coefficient) * 0.25;
                    blade.bend = low_layer + high_layer;
                    blade.tip = blade.tip * 0.875 + blade.bend * 0.125;
                    checksum += blade.tip;
                    Ok(())
                })
                .expect("run grass update");
            divan::black_box(checksum);
        }
    }
}

fn coefficient_for_index(index: usize) -> f32 {
    0.5 + (index & 31) as f32 * 0.0078125
}

#[divan::bench(args = GRASS_CASES)]
fn grass_layered_cadenced_cache_hit(bencher: Bencher<'_, '_>, case: GrassCase) {
    bencher
        .counter(ItemsCount::new(case.visible()))
        .with_inputs(|| GrassInput::new(case))
        .bench_local_refs(|input| input.run_cadence_batch(false));
}

#[divan::bench(args = GRASS_CASES)]
fn grass_layered_cadenced_cache_invalidated(bencher: Bencher<'_, '_>, case: GrassCase) {
    bencher
        .counter(ItemsCount::new(case.visible()))
        .with_inputs(|| GrassInput::new(case))
        .bench_local_refs(|input| input.run_cadence_batch(true));
}

struct ScheduledGrassLocal {
    update: PreparedQuery2<GrassBlade, WindLayer>,
    visible: Vec<EntityId>,
    cache: Option<DenseEntityScratch<BladeCache>>,
    coefficients: Vec<f32>,
    capacity: usize,
}

fn scheduled_grass_app(case: GrassCase) -> moirai::App {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .register_component::<GrassBlade>(case.layout.first())
        .expect("register blade");
    builder
        .world_builder()
        .register_component::<WindLayer>(case.layout.second())
        .expect("register wind");
    builder.insert_resource(GrassEpoch {
        terrain: Revision::ZERO,
        wind: Revision::ZERO,
    });
    builder.fixed(FixedConfig::new(Duration::from_millis(1)).expect("fixed"));

    builder
        .add_system(System::new(
            "seed-grass",
            stage::STARTUP,
            move |world, _dt| {
                let mut commands = world.commands().expect("startup commands");
                for index in 0..case.entities {
                    let entity = commands.spawn().expect("spawn blade");
                    commands
                        .insert(
                            entity,
                            GrassBlade {
                                bend: (index & 31) as f32 * 0.015625,
                                tip: 0.0,
                            },
                        )
                        .expect("insert blade");
                    if index % case.visible_stride == 0 {
                        commands
                            .insert(
                                entity,
                                WindLayer {
                                    low: ((index & 7) as f32 + 1.0) * 0.0078125,
                                    high: (((index >> 3) & 7) as f32 + 1.0) * 0.00390625,
                                },
                            )
                            .expect("insert wind");
                    }
                }
            },
        ))
        .expect("startup system");

    builder
        .add_system(
            System::with_local(
                "scheduled-grass",
                stage::FIXED_UPDATE,
                move |context| {
                    Ok(ScheduledGrassLocal {
                        update: context
                            .prepare_query2::<GrassBlade, WindLayer>(
                                QuerySpec::new(),
                                QueryPolicy::Membership,
                            )
                            .map_err(|error| format!("{error:?}"))?,
                        visible: Vec::with_capacity(case.visible()),
                        cache: None,
                        coefficients: Vec::with_capacity(case.visible()),
                        capacity: case.visible(),
                    })
                },
                |world, _dt, local| {
                    if local.cache.is_none() {
                        local.cache =
                            Some(DenseEntityScratch::with_capacity(world, local.capacity));
                    }
                    local.visible.clear();
                    local.visible.extend(
                        local
                            .update
                            .iter(world, QueryWindow::All)
                            .map_err(|error| format!("{error:?}"))?
                            .map(|(entity, _, _)| entity),
                    );
                    let key = world
                        .resource_scope_ref::<GrassEpoch, _>(|epoch, _| {
                            epoch.expect("grass epoch").key()
                        })
                        .map_err(|error| format!("{error:?}"))?;
                    local.coefficients.clear();
                    for (index, &entity) in local.visible.iter().enumerate() {
                        let cached = local
                            .cache
                            .as_mut()
                            .expect("cache initialized")
                            .get_or_insert_with(world, entity, || BladeCache {
                                key,
                                coefficient: coefficient_for_index(index),
                            })
                            .map_err(|error| format!("{error}"))?;
                        if cached.key != key {
                            cached.key = key;
                            cached.coefficient = coefficient_for_index(index);
                        }
                        local.coefficients.push(cached.coefficient);
                    }
                    let coefficients = &local.coefficients;
                    let mut visible_index = 0;
                    let mut checksum = 0.0_f32;
                    local
                        .update
                        .for_each_mut_read(world, QueryWindow::All, |_entity, blade, wind| {
                            let coefficient = coefficients[visible_index];
                            visible_index += 1;
                            blade.bend = blade.bend * 0.9375 + wind.low * coefficient;
                            blade.tip = blade.tip * 0.875 + (blade.bend + wind.high * 0.25) * 0.125;
                            checksum += blade.tip;
                            Ok(())
                        })
                        .map_err(|error| format!("{error:?}"))?;
                    divan::black_box(checksum);
                    Ok(())
                },
            )
            .run_if(
                Condition::fixed_step_mod(case.period as u64, case.phase as u64).expect("cadence"),
            ),
        )
        .expect("fixed grass system");

    let mut app = builder.build().expect("build scheduled grass app");
    app.update(case.period as f32 / 1_000.0)
        .expect("seed and warm scheduled grass");
    app
}

#[divan::bench(args = GRASS_CASES)]
fn grass_scheduled_fixed_cadence_with_local_cache(bencher: Bencher<'_, '_>, case: GrassCase) {
    bencher
        .counter(ItemsCount::new(case.visible()))
        .with_inputs(|| scheduled_grass_app(case))
        .bench_local_refs(|app| {
            app.update(case.period as f32 / 1_000.0)
                .expect("scheduled cadence batch")
        });
}
