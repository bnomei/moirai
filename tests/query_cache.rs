use moirai::component::ComponentOptions;
use moirai::query::{QueryCursor, QueryError, QueryParams, QuerySpec};
use moirai::world::{World, WorldBuilder};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy)]
struct Marker;

fn world() -> World {
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
fn query_cache_cold_and_hot_hit() {
    let mut world = world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let params = QueryParams::new().membership_cache(&cache);

    let first: Vec<_> = world
        .query::<Position>(spec.clone(), params)
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    let second: Vec<_> = world
        .query::<Position>(spec, QueryParams::new().membership_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(first, vec![1]);
    assert_eq!(second, vec![1]);
}

#[test]
fn query_result_cache_rejects_added_or_changed() {
    let mut world = world();
    let spec = QuerySpec::new().added::<Position>();
    assert!(matches!(
        world.build_query_result_cache::<Position>(spec),
        Err(QueryError::MovingChangeWindow)
    ));
}

#[test]
fn query_cache_is_owner_scoped() {
    let mut world_a = world();
    let mut world_b = world();
    let entity = world_a.spawn().expect("spawn");
    world_a.insert(entity, Position(1)).expect("insert");

    let cache = world_a
        .build_query_cache::<Position>(QuerySpec::new())
        .expect("cache");
    let params = QueryParams::new().membership_cache(&cache);
    assert!(matches!(
        world_b.query::<Position>(QuerySpec::new(), params),
        Err(QueryError::WrongOwner)
    ));
}

#[test]
fn query_cache_updates_on_spawn() {
    let mut world = world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let params = QueryParams::new().membership_cache(&cache);

    let b = world.spawn().expect("spawn");
    world.insert(b, Position(2)).expect("insert");

    let matches: Vec<_> = world
        .query::<Position>(spec, params)
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1, 2]);
}

#[test]
fn value_only_mutation_preserves_structural_cache() {
    let mut world = world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let params = QueryParams::new().membership_cache(&cache);

    world
        .get_mut::<Position>(a)
        .expect("get")
        .expect("present")
        .0 = 9;

    let matches: Vec<_> = world
        .query::<Position>(spec, params)
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![9]);
}

#[test]
fn membership_cache_stores_structural_members_for_added_queries() {
    let mut world = world();
    let a = world.spawn().expect("spawn a");
    world.insert(a, Position(1)).expect("insert a");
    let since_after_a = world.change_tick();

    let b = world.spawn().expect("spawn b");
    world.insert(b, Position(2)).expect("insert b");

    let spec = QuerySpec::new().added::<Position>();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");

    let narrow: Vec<_> = world
        .query::<Position>(
            spec.clone(),
            QueryParams::new()
                .membership_cache(&cache)
                .since(since_after_a),
        )
        .expect("narrow")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(narrow, vec![2]);

    let mut cursor = QueryCursor::from_spec_start::<Position>(&world, &spec).expect("cursor");
    let wide: Vec<_> = world
        .query::<Position>(
            spec,
            QueryParams::new()
                .membership_cache(&cache)
                .cursor(&mut cursor),
        )
        .expect("wide")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(wide, vec![1, 2]);
}
