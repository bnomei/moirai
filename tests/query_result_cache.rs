use moirai::component::ComponentOptions;
use moirai::query::{ExactIdPolicy, QueryError, QueryParams, QuerySpec};
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
fn result_cache_paths_reject_duplicate_exact_ids_contextually() {
    let mut other = world();
    let foreign = other.spawn().expect("foreign");
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("position");
    world.insert(entity, Marker).expect("marker");
    let stale = world.spawn().expect("stale");
    world.despawn(stale).expect("despawn stale");
    let exact = QuerySpec::new().exact_ids(vec![entity, entity], ExactIdPolicy::SkipUnavailable);
    let cache = world
        .build_query_result_cache::<Position>(QuerySpec::new())
        .expect("cache");

    assert!(matches!(
        world.query::<Position>(&exact, QueryParams::new().result_cache(&cache)),
        Err(QueryError::DuplicateExactId { entity: duplicate }) if duplicate == entity
    ));
    assert!(matches!(
        world.build_entity_query_result_cache(exact.clone()),
        Err(QueryError::DuplicateExactId { entity: duplicate }) if duplicate == entity
    ));
    assert!(matches!(
        world.build_query_result_cache::<Position>(exact.clone()),
        Err(QueryError::DuplicateExactId { entity: duplicate }) if duplicate == entity
    ));
    assert!(matches!(
        world.build_query2_result_cache::<Position, Marker>(exact),
        Err(QueryError::DuplicateExactId { entity: duplicate }) if duplicate == entity
    ));

    let mixed_foreign = QuerySpec::new().exact_ids(
        vec![foreign, foreign, entity, entity],
        ExactIdPolicy::SkipUnavailable,
    );
    assert!(matches!(
        world.build_entity_query_result_cache(mixed_foreign.clone()),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        world.build_query_result_cache::<Position>(mixed_foreign.clone()),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        world.build_query2_result_cache::<Position, Marker>(mixed_foreign),
        Err(QueryError::WrongOwner)
    ));

    let mixed_stale = QuerySpec::new().exact_ids(
        vec![stale, stale, entity, entity],
        ExactIdPolicy::ErrorOnUnavailable,
    );
    assert!(matches!(
        world.build_entity_query_result_cache(mixed_stale.clone()),
        Err(QueryError::MissingExactId { entity: missing }) if missing == stale
    ));
    assert!(matches!(
        world.build_query_result_cache::<Position>(mixed_stale.clone()),
        Err(QueryError::MissingExactId { entity: missing }) if missing == stale
    ));
    assert!(matches!(
        world.build_query2_result_cache::<Position, Marker>(mixed_stale),
        Err(QueryError::MissingExactId { entity: missing }) if missing == stale
    ));
}

#[test]
fn entity_result_cache_refreshes_after_deferred_empty_spawn_and_despawn() {
    let mut world = world();
    let initial = world.spawn().expect("initial");
    let spec = QuerySpec::new();
    let cache = world
        .build_entity_query_result_cache(spec.clone())
        .expect("cache");

    let added = world.commands().expect("commands").spawn().expect("spawn");
    world.flush().expect("spawn flush");
    assert_eq!(
        world
            .query_ids(&spec, QueryParams::new().result_cache(&cache))
            .expect("after spawn")
            .collect::<Vec<_>>(),
        vec![initial, added]
    );

    world
        .commands()
        .expect("commands")
        .despawn(initial)
        .expect("despawn");
    world.flush().expect("despawn flush");
    assert_eq!(
        world
            .query_ids(&spec, QueryParams::new().result_cache(&cache))
            .expect("after despawn")
            .collect::<Vec<_>>(),
        vec![added]
    );
}

#[test]
fn entity_result_cache_rejects_moving_exact_and_foreign_uses() {
    let mut a = world();
    let mut b = world();
    assert!(matches!(
        a.build_entity_query_result_cache(QuerySpec::new().changed::<Position>()),
        Err(QueryError::MovingChangeWindow)
    ));
    let entity = a.spawn().expect("entity");
    assert!(matches!(
        a.build_entity_query_result_cache(
            QuerySpec::new().exact_ids(vec![entity], moirai::query::ExactIdPolicy::SkipUnavailable)
        ),
        Err(QueryError::ExactIdOrderConflict)
    ));
    let cache = a
        .build_entity_query_result_cache(QuerySpec::new())
        .expect("cache");
    assert!(matches!(
        b.query_ids(&QuerySpec::new(), QueryParams::new().result_cache(&cache)),
        Err(QueryError::WrongOwner)
    ));
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
