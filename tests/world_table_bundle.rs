use moirai::component::ComponentOptions;
use moirai::query::{QueryParams, QuerySpec};
use moirai::world::{DynamicBundle, WorldBuilder};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Velocity(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mass(i32);

struct UnrelatedTable;

struct NonCloneTable(Box<i32>);

struct NonCloneSparse(Box<i32>);

struct IdentityTracked {
    token: Box<u64>,
    clones: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
}

impl Clone for IdentityTracked {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            token: Box::new(*self.token),
            clones: Rc::clone(&self.clones),
            drops: Rc::clone(&self.drops),
        }
    }
}

impl Drop for IdentityTracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn spawn_bundle_with_table_components() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    let mut world = builder.build().expect("build");

    let entity = world
        .spawn_bundle((Position(1), Velocity(2)))
        .expect("spawn bundle");
    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get position")
            .map(|p| p.0),
        Some(1)
    );
    assert_eq!(
        world
            .get::<Velocity>(entity)
            .expect("get velocity")
            .map(|v| v.0),
        Some(2)
    );
}

#[test]
fn tuple_and_dynamic_bundles_spawn_non_clone_values() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<NonCloneTable>(ComponentOptions::table())
        .expect("table");
    builder
        .register_component::<NonCloneSparse>(ComponentOptions::sparse())
        .expect("sparse");
    let mut world = builder.build().expect("build");

    let tuple_entity = world
        .spawn_bundle((NonCloneTable(Box::new(1)), NonCloneSparse(Box::new(2))))
        .expect("tuple bundle");
    assert_eq!(
        *world
            .get::<NonCloneTable>(tuple_entity)
            .expect("get")
            .expect("present")
            .0,
        1
    );

    let mut dynamic = DynamicBundle::new();
    dynamic
        .push(&world, NonCloneTable(Box::new(3)))
        .expect("dynamic table");
    dynamic
        .push(&world, NonCloneSparse(Box::new(4)))
        .expect("dynamic sparse");
    let dynamic_entity = world.spawn_bundle(dynamic).expect("dynamic bundle");
    assert_eq!(
        *world
            .get::<NonCloneSparse>(dynamic_entity)
            .expect("get")
            .expect("present")
            .0,
        4
    );
}

#[test]
fn table_query2_mutates_both_components() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    let mut world = builder.build().expect("build");
    let entity = world
        .spawn_bundle((Position(1), Velocity(2)))
        .expect("spawn");

    world
        .for_each2_mut::<Position, Velocity>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, pos, vel| {
                pos.0 += vel.0;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world.get::<Position>(entity).expect("get").map(|p| p.0),
        Some(3)
    );
}

#[test]
fn table_for_each2_mut_with_reversed_component_indices() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Mass>(ComponentOptions::table())
        .expect("mass");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    let mut world = builder.build().expect("build");

    let entity = world.spawn().expect("spawn");
    world.insert(entity, Mass(5)).expect("mass");
    world.insert(entity, Velocity(10)).expect("vel");

    world
        .for_each2_mut::<Velocity, Mass>(&QuerySpec::new(), QueryParams::new(), |_, vel, mass| {
            vel.0 += mass.0;
            mass.0 *= 2;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world.get::<Velocity>(entity).expect("vel").map(|v| v.0),
        Some(15)
    );
    assert_eq!(
        world.get::<Mass>(entity).expect("mass").map(|v| v.0),
        Some(10)
    );
}

#[test]
fn table_insert_replace_preserves_added_tick() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    world.insert(entity, Position(1)).expect("add");
    let since_after_add = world.change_tick();
    world.insert(entity, Position(2)).expect("replace");

    let added: Vec<_> = world
        .query::<Position>(
            &QuerySpec::new().added::<Position>(),
            QueryParams::new().since(since_after_add),
        )
        .expect("added")
        .map(|(_, p)| p.0)
        .collect();
    assert!(added.is_empty());

    let changed: Vec<_> = world
        .query::<Position>(
            &QuerySpec::new().changed::<Position>(),
            QueryParams::new().since(since_after_add),
        )
        .expect("changed")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(changed, vec![2]);
}

#[test]
fn table_migration_moves_identity_and_ticks_without_cloning() {
    let clones = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<IdentityTracked>(ComponentOptions::table())
        .expect("tracked");
    builder
        .register_component::<UnrelatedTable>(ComponentOptions::table())
        .expect("unrelated");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");

    world
        .insert(
            entity,
            IdentityTracked {
                token: Box::new(41),
                clones: Rc::clone(&clones),
                drops: Rc::clone(&drops),
            },
        )
        .expect("insert tracked");
    let identity = world
        .get::<IdentityTracked>(entity)
        .expect("get")
        .expect("present")
        .token
        .as_ref() as *const u64;
    let after_insert = world.change_tick();

    *world
        .get_mut::<IdentityTracked>(entity)
        .expect("get mut")
        .expect("present")
        .token = 42;
    let after_mutation = world.change_tick();

    world.insert(entity, UnrelatedTable).expect("add unrelated");
    assert_eq!(clones.get(), 0);
    assert_eq!(
        world
            .get::<IdentityTracked>(entity)
            .expect("get")
            .expect("present")
            .token
            .as_ref() as *const u64,
        identity
    );

    let added_after_original_insert = world
        .query::<IdentityTracked>(
            &QuerySpec::new().added::<IdentityTracked>(),
            QueryParams::new().since(after_insert),
        )
        .expect("added query")
        .count();
    assert_eq!(added_after_original_insert, 0);
    let changed_after_original_insert = world
        .query::<IdentityTracked>(
            &QuerySpec::new().changed::<IdentityTracked>(),
            QueryParams::new().since(after_insert),
        )
        .expect("changed query")
        .count();
    assert_eq!(changed_after_original_insert, 1);
    let changed_by_migration = world
        .query::<IdentityTracked>(
            &QuerySpec::new().changed::<IdentityTracked>(),
            QueryParams::new().since(after_mutation),
        )
        .expect("changed query")
        .count();
    assert_eq!(changed_by_migration, 0);

    assert!(world
        .remove::<UnrelatedTable>(entity)
        .expect("remove unrelated")
        .is_some());
    assert_eq!(clones.get(), 0);
    assert_eq!(
        world
            .get::<IdentityTracked>(entity)
            .expect("get")
            .expect("present")
            .token
            .as_ref() as *const u64,
        identity
    );

    let removed = world
        .remove::<IdentityTracked>(entity)
        .expect("remove tracked")
        .expect("present");
    assert_eq!(removed.token.as_ref() as *const u64, identity);
    assert_eq!(*removed.token, 42);
    assert_eq!(clones.get(), 0);
    assert_eq!(drops.get(), 0);
    drop(removed);
    assert_eq!(drops.get(), 1);
}
