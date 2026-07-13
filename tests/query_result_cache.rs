use moirai::component::ComponentOptions;
use moirai::query::{QueryError, QueryParams, QuerySpec};
use moirai::world::WorldBuilder;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy)]
struct Marker;

fn world() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<Marker>(ComponentOptions::sparse())
        .expect("register");
    builder.build().expect("build")
}

#[test]
fn result_cache_cold_and_hot_hit() {
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_result_cache::<Position>(spec.clone())
        .expect("cache");
    let params = QueryParams::new().result_cache(&cache);

    let first: Vec<_> = world
        .query::<Position>(&spec, params)
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    let second: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().result_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(first, vec![1]);
    assert_eq!(second, vec![1]);
}

#[test]
fn result_cache_updates_on_topology_change() {
    let mut world = world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_result_cache::<Position>(spec.clone())
        .expect("cache");
    let params = QueryParams::new().result_cache(&cache);

    let b = world.spawn().expect("spawn");
    world.insert(b, Position(2)).expect("insert");

    let matches: Vec<_> = world
        .query::<Position>(&spec, params)
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1, 2]);
}

#[test]
#[cfg(feature = "testkit")]
fn result_cache_invalidate_and_rebuild() {
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_result_cache::<Position>(spec.clone())
        .expect("cache");
    world.invalidate_query_result_cache(&cache);

    let rebuilt = world
        .build_query_result_cache::<Position>(spec.clone())
        .expect("rebuild");
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().result_cache(&rebuilt))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1]);
}

#[test]
fn result_cache_rejects_wrong_owner() {
    let mut world_a = world();
    let mut world_b = world();
    let entity = world_a.spawn().expect("spawn");
    world_a.insert(entity, Position(1)).expect("insert");

    let cache = world_a
        .build_query_result_cache::<Position>(QuerySpec::new())
        .expect("cache");
    assert!(matches!(
        world_b.query::<Position>(&QuerySpec::new(), QueryParams::new().result_cache(&cache)),
        Err(QueryError::WrongOwner)
    ));
}

#[test]
fn result_cache_rejects_exact_ids() {
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    let spec =
        QuerySpec::new().exact_ids(vec![entity], moirai::query::ExactIdPolicy::SkipUnavailable);
    assert!(matches!(
        world.build_query_result_cache::<Position>(spec),
        Err(QueryError::ExactIdOrderConflict)
    ));
}
