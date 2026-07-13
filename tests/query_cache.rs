use moirai::component::ComponentOptions;
use moirai::query::{QueryCursor, QueryError, QueryParams, QuerySpec};
use moirai::world::{World, WorldBuilder};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy)]
struct Marker;

#[derive(Clone, Copy, Debug, PartialEq)]
struct TablePos(i32);

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
        .query::<Position>(&spec, params)
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    let second: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(first, vec![1]);
    assert_eq!(second, vec![1]);
}

#[test]
fn entity_membership_cache_refreshes_after_deferred_empty_spawn() {
    let mut world = world();
    let spec = QuerySpec::new();
    let cache = world
        .build_entity_query_cache(spec.clone())
        .expect("entity cache");
    let entity = world.commands().expect("commands").spawn().expect("spawn");
    world.flush().expect("flush");
    assert_eq!(
        world
            .query_ids(&spec, QueryParams::new().membership_cache(&cache))
            .expect("refreshed")
            .collect::<Vec<_>>(),
        vec![entity]
    );
}

#[test]
fn entity_membership_cache_is_owner_and_spec_scoped() {
    let mut a = world();
    let mut b = world();
    let cache = a.build_entity_query_cache(QuerySpec::new()).expect("cache");
    assert!(matches!(
        b.query_ids(
            &QuerySpec::new(),
            QueryParams::new().membership_cache(&cache)
        ),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        a.query_ids(
            &QuerySpec::new().with::<Position>(),
            QueryParams::new().membership_cache(&cache)
        ),
        Err(QueryError::WrongQuery { .. })
    ));
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
        world_b.query::<Position>(&QuerySpec::new(), params),
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
        .query::<Position>(&spec, params)
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
        .query::<Position>(&spec, params)
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
            &spec,
            QueryParams::new()
                .membership_cache(&cache)
                .since(since_after_a),
        )
        .expect("narrow")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(narrow, vec![2]);

    let mut cursor = QueryCursor::from_spec_start::<Position>(&mut world, &spec).expect("cursor");
    let wide: Vec<_> = world
        .query::<Position>(
            &spec,
            QueryParams::new()
                .membership_cache(&cache)
                .cursor(&mut cursor),
        )
        .expect("wide")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(wide, vec![1, 2]);
}

fn table_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register");
    builder.build().expect("build")
}

#[test]
fn table_membership_cache_collects_archetype_entities() {
    let mut world = table_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    world.insert(a, TablePos(1)).expect("insert a");
    world.insert(b, TablePos(2)).expect("insert b");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<TablePos>(spec.clone())
        .expect("cache");
    let matches: Vec<_> = world
        .query::<TablePos>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1, 2]);
}

#[test]
fn table_added_filter_uses_structural_membership_cache() {
    let mut world = table_world();
    let a = world.spawn().expect("spawn a");
    world.insert(a, TablePos(1)).expect("insert a");
    let since_after_a = world.change_tick();

    let b = world.spawn().expect("spawn b");
    world.insert(b, TablePos(2)).expect("insert b");

    let spec = QuerySpec::new().added::<TablePos>();
    let cache = world
        .build_query_cache::<TablePos>(spec.clone())
        .expect("cache");
    let matches: Vec<_> = world
        .query::<TablePos>(
            &spec,
            QueryParams::new()
                .membership_cache(&cache)
                .since(since_after_a),
        )
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn exact_id_query_iterator_skips_unmatched_entities() {
    let mut world = world();
    let matched = world.spawn().expect("matched");
    let skipped = world.spawn().expect("skipped");
    world.insert(matched, Position(7)).expect("matched");
    world.insert(skipped, Marker).expect("marker only");

    let spec = QuerySpec::new().exact_ids(
        vec![matched, skipped],
        moirai::query::ExactIdPolicy::SkipUnavailable,
    );
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![7]);
}

#[test]
fn cached_for_each_mut_matches_uncached_added_window() {
    let mut world = world();
    let old = world.spawn().expect("old");
    world.insert(old, Position(1)).expect("old");
    let since_after_old = world.change_tick();

    let new = world.spawn().expect("new");
    world.insert(new, Position(2)).expect("new");

    let spec = QuerySpec::new().added::<Position>();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let mut uncached = Vec::new();
    world
        .for_each_mut::<Position>(
            &spec,
            QueryParams::new().since(since_after_old),
            |_, pos| {
                uncached.push(pos.0);
                pos.0 += 100;
                Ok(())
            },
        )
        .expect("uncached");

    world.insert(old, Position(1)).expect("reset old");
    world.insert(new, Position(2)).expect("reset new");

    let mut cached = Vec::new();
    world
        .for_each_mut::<Position>(
            &spec,
            QueryParams::new()
                .membership_cache(&cache)
                .since(since_after_old),
            |_, pos| {
                cached.push(pos.0);
                pos.0 += 100;
                Ok(())
            },
        )
        .expect("cached");

    assert_eq!(uncached, cached);
    assert_eq!(uncached, vec![2]);
    assert_eq!(
        world.get::<Position>(old).expect("get").expect("present").0,
        1,
        "older structural member must not be mutated"
    );
}

#[test]
fn cached_query_iterator_refreshes_after_topology_change() {
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let _first: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("warm")
        .collect();

    let newcomer = world.spawn().expect("spawn");
    world.insert(newcomer, Position(2)).expect("insert");
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("refresh")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches.len(), 2);
}
