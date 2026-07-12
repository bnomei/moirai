extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use moirai::component::ComponentOptions;
use moirai::query::{QueryParams, QuerySpec};
use moirai::world::{World, WorldBuilder};
use moirai::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Comp {
    Pos,
    Vel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    Spawn,
    Despawn(usize),
    InsertPos(usize, i32),
    InsertVel(usize, i32),
    RemovePos(usize),
}

#[derive(Clone, Copy)]
struct Position(#[allow(dead_code)] i32);

#[derive(Clone, Copy)]
struct Velocity(#[allow(dead_code)] i32);

struct Model {
    next_slot: usize,
    entities: BTreeMap<usize, BTreeSet<Comp>>,
}

impl Model {
    fn new() -> Self {
        Self {
            next_slot: 0,
            entities: BTreeMap::new(),
        }
    }

    fn spawn(&mut self) -> usize {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.entities.insert(slot, BTreeSet::new());
        slot
    }

    fn despawn(&mut self, slot: usize) {
        self.entities.remove(&slot);
    }

    fn insert(&mut self, slot: usize, comp: Comp) {
        if let Some(comps) = self.entities.get_mut(&slot) {
            comps.insert(comp);
        }
    }

    fn remove(&mut self, slot: usize, comp: Comp) {
        if let Some(comps) = self.entities.get_mut(&slot) {
            comps.remove(&comp);
        }
    }

    fn query_pos(&self) -> Vec<usize> {
        self.entities
            .iter()
            .filter_map(|(slot, comps)| comps.contains(&Comp::Pos).then_some(*slot))
            .collect()
    }

    fn query_pos_vel(&self) -> Vec<usize> {
        self.entities
            .iter()
            .filter_map(|(slot, comps)| {
                (comps.contains(&Comp::Pos) && comps.contains(&Comp::Vel)).then_some(*slot)
            })
            .collect()
    }
}

fn model_world() -> (World, BTreeMap<usize, EntityId>) {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<Velocity>(ComponentOptions::sparse())
        .expect("register");
    let world = builder.build().expect("build");
    (world, BTreeMap::new())
}

fn apply_op(world: &mut World, ids: &mut BTreeMap<usize, EntityId>, model: &mut Model, op: Op) {
    match op {
        Op::Spawn => {
            let slot = model.spawn();
            let entity = world.spawn().expect("spawn");
            ids.insert(slot, entity);
        }
        Op::Despawn(slot) => {
            if let Some(entity) = ids.get(&slot).copied() {
                if world.is_alive(entity) {
                    world.despawn(entity).expect("despawn");
                }
                model.despawn(slot);
            }
        }
        Op::InsertPos(slot, value) => {
            if let Some(entity) = ids.get(&slot).copied() {
                if world.is_alive(entity) {
                    world.insert(entity, Position(value)).expect("insert");
                    model.insert(slot, Comp::Pos);
                }
            }
        }
        Op::InsertVel(slot, value) => {
            if let Some(entity) = ids.get(&slot).copied() {
                if world.is_alive(entity) {
                    world.insert(entity, Velocity(value)).expect("insert");
                    model.insert(slot, Comp::Vel);
                }
            }
        }
        Op::RemovePos(slot) => {
            if let Some(entity) = ids.get(&slot).copied() {
                if world.is_alive(entity) {
                    let _ = world.remove::<Position>(entity);
                    model.remove(slot, Comp::Pos);
                }
            }
        }
    }
}

fn compare_query1(world: &mut World, model: &Model, ids: &BTreeMap<usize, EntityId>) {
    let spec = QuerySpec::new();
    let actual_slots: Vec<usize> = world
        .query::<Position>(spec.clone(), QueryParams::new())
        .expect("query")
        .filter_map(|(entity, _)| {
            ids.iter()
                .find_map(|(slot, id)| (*id == entity).then_some(*slot))
        })
        .collect();
    assert_eq!(actual_slots, model.query_pos());
}

fn compare_cached(
    world: &mut World,
    model: &Model,
    ids: &BTreeMap<usize, EntityId>,
    cache: &moirai::query::QueryCache,
) {
    let spec = QuerySpec::new();
    let params = QueryParams::new().membership_cache(cache);
    let actual_slots: Vec<usize> = world
        .query::<Position>(spec, params)
        .expect("cached")
        .filter_map(|(entity, _)| {
            ids.iter()
                .find_map(|(slot, id)| (*id == entity).then_some(*slot))
        })
        .collect();
    assert_eq!(actual_slots, model.query_pos());
}

#[test]
fn randomized_query_matches_reference_model() {
    let trace = [
        Op::Spawn,
        Op::InsertPos(0, 1),
        Op::Spawn,
        Op::InsertPos(1, 2),
        Op::InsertVel(0, 9),
        Op::Despawn(0),
        Op::InsertPos(1, 3),
        Op::RemovePos(1),
        Op::InsertPos(1, 4),
    ];

    let (mut world, mut ids) = model_world();
    let mut model = Model::new();
    let mut cache = None;

    for op in trace {
        apply_op(&mut world, &mut ids, &mut model, op);
        compare_query1(&mut world, &model, &ids);

        if cache.is_none() {
            cache = Some(
                world
                    .build_query_cache::<Position>(QuerySpec::new())
                    .expect("cache"),
            );
        }
        compare_cached(&mut world, &model, &ids, cache.as_ref().expect("cache"));
    }
}

#[test]
fn query2_randomized_matches_reference_model() {
    let trace = [
        Op::Spawn,
        Op::InsertPos(0, 1),
        Op::InsertVel(0, 2),
        Op::Spawn,
        Op::InsertPos(1, 3),
        Op::InsertVel(1, 4),
        Op::Despawn(0),
    ];

    let (mut world, mut ids) = model_world();
    let mut model = Model::new();

    for op in trace {
        apply_op(&mut world, &mut ids, &mut model, op);
        let expected = model.query_pos_vel();
        let actual: Vec<usize> = world
            .query2::<Position, Velocity>(QuerySpec::new(), QueryParams::new())
            .expect("query2")
            .filter_map(|(entity, _, _)| {
                ids.iter()
                    .find_map(|(slot, id)| (*id == entity).then_some(*slot))
            })
            .collect();
        assert_eq!(actual, expected);
    }
}
