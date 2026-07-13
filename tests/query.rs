use moirai::component::ComponentOptions;
use moirai::query::{ExactIdPolicy, QueryCursor, QueryError, QueryParams, QuerySpec};
use moirai::world::{World, WorldBuilder};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Velocity(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct TablePos(i32);

#[derive(Clone, Copy)]
struct Player;

#[derive(Clone, Copy)]
struct Enemy;

fn sparse_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register position");
    builder
        .register_component::<Velocity>(ComponentOptions::sparse())
        .expect("register velocity");
    builder
        .register_component::<Enemy>(ComponentOptions::sparse())
        .expect("register enemy");
    builder.build().expect("build")
}

fn mixed_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register position");
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register table pos");
    builder.build().expect("build")
}

fn tag_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register position");
    builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("register player");
    builder.build().expect("build")
}

#[test]
fn query1_returns_all_entities_with_component() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    let c = world.spawn().expect("spawn c");
    world.insert(a, Position(1)).expect("insert a");
    world.insert(b, Position(2)).expect("insert b");
    world.insert(c, Velocity(9)).expect("insert c");

    let spec = QuerySpec::new();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1, 2]);
}

#[test]
fn query_respects_without_list() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    world.insert(a, Position(1)).expect("insert a");
    world.insert(a, Enemy).expect("mark enemy a");
    world.insert(b, Position(2)).expect("insert b");

    let spec = QuerySpec::new().without::<Enemy>();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query2_returns_intersection() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    let c = world.spawn().expect("spawn c");
    world.insert(a, Position(1)).expect("insert a");
    world.insert(a, Velocity(10)).expect("insert vel a");
    world.insert(b, Position(2)).expect("insert b");
    world.insert(c, Velocity(3)).expect("insert vel c");

    let spec = QuerySpec::new();
    let matches: Vec<_> = world
        .query2::<Position, Velocity>(&spec, QueryParams::new())
        .expect("query2")
        .map(|(_, p, v)| (p.0, v.0))
        .collect();
    assert_eq!(matches, vec![(1, 10)]);
}

#[test]
fn query2_mixed_table_and_sparse_components() {
    let mut world = mixed_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    world.insert(a, Position(1)).expect("insert sparse a");
    world.insert(a, TablePos(10)).expect("insert table a");
    world.insert(b, TablePos(20)).expect("insert table b");

    let spec = QuerySpec::new();
    let matches: Vec<_> = world
        .query2::<Position, TablePos>(&spec, QueryParams::new())
        .expect("query2")
        .map(|(_, p, t)| (p.0, t.0))
        .collect();
    assert_eq!(matches, vec![(1, 10)]);
}

#[test]
fn table_component_insert_get_query() {
    let mut world = mixed_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(42)).expect("insert");

    let spec = QuerySpec::new();
    let matches: Vec<_> = world
        .query::<TablePos>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, t)| t.0)
        .collect();
    assert_eq!(matches, vec![42]);
}

#[test]
fn query_with_tag_filter() {
    let mut world = tag_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    world.insert(a, Position(1)).expect("insert a");
    world.insert(b, Position(2)).expect("insert b");
    world.insert(a, Player).expect("tag player");

    let spec = QuerySpec::new().with_tag::<Player>();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1]);
}

#[test]
fn query_skips_despawned_entities() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    world.insert(a, Position(1)).expect("insert a");
    world.insert(b, Position(2)).expect("insert b");
    world.despawn(a).expect("despawn a");

    let spec = QuerySpec::new();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query_added_filters_by_tick() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    world.insert(a, Position(1)).expect("insert a");
    let since_after_a = world.change_tick();

    let b = world.spawn().expect("spawn b");
    world.insert(b, Position(2)).expect("insert b");

    let spec = QuerySpec::new().added::<Position>();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().since(since_after_a))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query_changed_filters_by_tick() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    world.insert(a, Position(1)).expect("insert a");
    let since_after_insert = world.change_tick();

    world
        .get_mut::<Position>(a)
        .expect("get mut")
        .expect("present")
        .0 = 5;

    let spec = QuerySpec::new().changed::<Position>();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().since(since_after_insert))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![5]);
}

#[test]
fn query_unregistered_component_returns_error() {
    let mut world = sparse_world();
    let spec = QuerySpec::new();
    assert!(matches!(
        world.query::<TablePos>(&spec, QueryParams::new()),
        Err(QueryError::UnregisteredComponent { .. })
    ));
}

#[test]
fn query2_skips_entities_missing_second_component() {
    let mut world = sparse_world();
    let matched = world.spawn().expect("matched");
    let partial = world.spawn().expect("partial");
    world.insert(matched, Position(1)).expect("pos matched");
    world.insert(matched, Velocity(2)).expect("vel matched");
    world.insert(partial, Position(3)).expect("pos partial");

    let matches: Vec<_> = world
        .query2::<Position, Velocity>(&QuerySpec::new(), QueryParams::new())
        .expect("query")
        .map(|(_, pos, vel)| (pos.0, vel.0))
        .collect();
    assert_eq!(matches, vec![(1, 2)]);
}

#[test]
fn query_exact_ids_preserves_order() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    let c = world.spawn().expect("spawn c");
    world.insert(a, Position(1)).expect("insert a");
    world.insert(b, Position(2)).expect("insert b");
    world.insert(c, Position(3)).expect("insert c");

    let spec = QuerySpec::new().exact_ids(vec![c, a], ExactIdPolicy::SkipUnavailable);
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(entity, p)| (entity, p.0))
        .collect();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].1, 3);
    assert_eq!(matches[1].1, 1);
    assert_eq!(matches[0].0, c);
    assert_eq!(matches[1].0, a);
}

#[test]
fn for_each_mut_updates_table_components() {
    let mut world = mixed_world();
    let a = world.spawn().expect("spawn");
    world.insert(a, TablePos(5)).expect("insert");

    world
        .for_each_mut::<TablePos>(&QuerySpec::new(), QueryParams::new(), |_, pos| {
            pos.0 *= 2;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world.get::<TablePos>(a).expect("get").expect("present").0,
        10
    );
}

#[test]
fn for_each2_mut_updates_sparse_and_table_components() {
    let mut world = mixed_world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert sparse");
    world.insert(a, TablePos(10)).expect("insert table");

    world
        .for_each2_mut::<Position, TablePos>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, pos, table| {
                pos.0 += table.0;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world.get::<Position>(a).expect("get").expect("present").0,
        11
    );
}

#[test]
fn for_each_mut_updates_sparse_components() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    world
        .for_each_mut::<Position>(&QuerySpec::new(), QueryParams::new(), |_, pos| {
            pos.0 += 10;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world.get::<Position>(a).expect("get").expect("present").0,
        11
    );
}

#[test]
fn for_each2_mut_updates_both_components() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");
    world.insert(a, Velocity(2)).expect("insert");

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
        world.get::<Position>(a).expect("get").expect("present").0,
        3
    );
}

#[test]
fn duplicate_mutable_component_is_rejected() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    assert!(matches!(
        world.for_each2_mut::<Position, Position>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, _, _| { Ok(()) }
        ),
        Err(QueryError::DuplicateMutableComponent { .. })
    ));
}

#[test]
fn query_exact_ids_error_on_unavailable() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn a");
    let missing = world.spawn().expect("spawn missing");
    world.insert(a, Position(1)).expect("insert a");
    world.despawn(missing).expect("despawn");

    let spec = QuerySpec::new().exact_ids(vec![a, missing], ExactIdPolicy::ErrorOnUnavailable);
    assert!(matches!(
        world.query::<Position>(&spec, QueryParams::new()),
        Err(QueryError::MissingExactId { .. })
    ));
}

#[test]
fn query_rejects_non_tag_with_tag_filter() {
    let mut world = sparse_world();
    let spec = QuerySpec::new().with_tag::<Position>();
    assert!(matches!(
        world.query::<Position>(&spec, QueryParams::new()),
        Err(QueryError::WrongStorageKind { .. })
    ));
}

#[test]
fn query_cursor_from_spec_without_testkit() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let spec = QuerySpec::new().added::<Position>();
    let mut cursor = QueryCursor::from_spec_start::<Position>(&mut world, &spec).expect("cursor");
    let params = QueryParams::new().cursor(&mut cursor);
    let mut query = world.query::<Position>(&spec, params).expect("query");
    assert!(query.next().is_some());
}

#[test]
fn query_cursor_rejects_an_exact_id_spec_with_a_different_policy() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let skip_spec = QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::SkipUnavailable);
    let mut cursor =
        QueryCursor::from_spec_start::<Position>(&mut world, &skip_spec).expect("cursor");
    let error_spec = QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::ErrorOnUnavailable);

    assert!(matches!(
        world.query::<Position>(&error_spec, QueryParams::new().cursor(&mut cursor)),
        Err(QueryError::WrongQuery { .. })
    ));
}

#[test]
fn query_effects_rejects_commands_while_idle() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let result = world.for_each_mut_with_effects::<Position>(
        &QuerySpec::new(),
        QueryParams::new(),
        |_, _, effects| {
            let _ = effects.commands()?;
            Ok(())
        },
    );
    assert!(matches!(result, Err(QueryError::BorrowConflict { .. })));
}

#[test]
fn explicit_without_excludes_domain_marker_without_magic_name() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn");
    let b = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");
    world.insert(a, Enemy).expect("enemy");
    world.insert(b, Position(2)).expect("insert");

    let spec = QuerySpec::new().without::<Enemy>();
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

fn reverse_sparse_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Velocity>(ComponentOptions::sparse())
        .expect("register velocity");
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register position");
    builder.build().expect("build")
}

fn table_only_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register");
    builder.build().expect("build")
}

fn table_pair_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    builder.build().expect("build")
}

#[test]
fn for_each2_mut_table_primary_sparse_second() {
    let mut world = mixed_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(10)).expect("table");
    world.insert(entity, Position(1)).expect("sparse");

    world
        .for_each2_mut::<TablePos, Position>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, table, pos| {
                pos.0 += table.0;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        11
    );
}

#[test]
fn for_each2_mut_sparse_pair_when_second_registered_first() {
    let mut world = reverse_sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(3)).expect("pos");
    world.insert(entity, Velocity(4)).expect("vel");

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
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        7
    );
}

#[test]
fn for_each_mut_uses_membership_cache_for_table_primary() {
    let mut world = table_only_world();
    let a = world.spawn().expect("a");
    let b = world.spawn().expect("b");
    world.insert(a, TablePos(1)).expect("a");
    world.insert(b, TablePos(2)).expect("b");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<TablePos>(spec.clone())
        .expect("cache");

    world
        .for_each_mut::<TablePos>(
            &spec,
            QueryParams::new().membership_cache(&cache),
            |_, pos| {
                pos.0 *= 10;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world.get::<TablePos>(a).expect("get").expect("present").0,
        10
    );
    assert_eq!(
        world.get::<TablePos>(b).expect("get").expect("present").0,
        20
    );
}

#[test]
fn for_each_mut_uses_result_cache() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(5)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_result_cache::<Position>(spec.clone())
        .expect("cache");

    world
        .for_each_mut::<Position>(&spec, QueryParams::new().result_cache(&cache), |_, pos| {
            pos.0 += 1;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        6
    );
}

#[test]
fn for_each2_mut_uses_membership_cache() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("pos");
    world.insert(entity, Velocity(2)).expect("vel");

    let spec = QuerySpec::new();
    let cache = world
        .build_query2_cache::<Position, Velocity>(spec.clone())
        .expect("cache");

    world
        .for_each2_mut::<Position, Velocity>(
            &spec,
            QueryParams::new().membership_cache(&cache),
            |_, pos, vel| {
                pos.0 += vel.0;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        3
    );
}

#[test]
fn for_each2_mut_query2_cache_skips_entities_missing_second() {
    let mut world = sparse_world();
    let pos_only = world.spawn().expect("a");
    let both = world.spawn().expect("b");
    world.insert(pos_only, Position(1)).expect("a");
    world.insert(both, Position(2)).expect("b");
    world.insert(both, Velocity(9)).expect("b");

    let cache = world
        .build_query2_cache::<Position, Velocity>(QuerySpec::new())
        .expect("cache");

    world
        .for_each2_mut::<Position, Velocity>(
            &QuerySpec::new(),
            QueryParams::new().membership_cache(&cache),
            |_, pos, _| {
                pos.0 += 100;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(pos_only)
            .expect("get")
            .expect("present")
            .0,
        1
    );
    assert_eq!(
        world
            .get::<Position>(both)
            .expect("get")
            .expect("present")
            .0,
        102
    );
}

#[test]
fn for_each2_mut_table_table_pair_updates_both() {
    let mut world = table_pair_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("pos");
    world.insert(entity, Velocity(2)).expect("vel");

    world
        .for_each2_mut::<Position, Velocity>(
            &QuerySpec::new(),
            QueryParams::new(),
            |_, pos, vel| {
                pos.0 *= 10;
                vel.0 += 5;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        10
    );
    assert_eq!(
        world
            .get::<Velocity>(entity)
            .expect("get")
            .expect("present")
            .0,
        7
    );
}

fn reverse_table_pair_world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Velocity>(ComponentOptions::table())
        .expect("velocity");
    builder
        .register_component::<Position>(ComponentOptions::table())
        .expect("position");
    builder.build().expect("build")
}

#[test]
fn for_each2_mut_table_table_reverse_column_order() {
    let mut world = reverse_table_pair_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(2)).expect("pos");
    world.insert(entity, Velocity(3)).expect("vel");

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
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        5
    );
}

#[test]
fn for_each2_mut_uses_result_cache() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("pos");
    world.insert(entity, Velocity(4)).expect("vel");

    let spec = QuerySpec::new();
    let cache = world
        .build_query2_result_cache::<Position, Velocity>(spec.clone())
        .expect("cache");

    world
        .for_each2_mut::<Position, Velocity>(
            &spec,
            QueryParams::new().result_cache(&cache),
            |_, pos, vel| {
                pos.0 += vel.0;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        5
    );
}

#[test]
fn query_changed_filter_returns_mutated_components() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    let since = world.change_tick();
    world
        .get_mut::<Position>(entity)
        .expect("mut")
        .expect("present")
        .0 = 9;

    let matches: Vec<_> = world
        .query::<Position>(
            &QuerySpec::new().changed::<Position>(),
            QueryParams::new().since(since),
        )
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![9]);
}

#[test]
fn for_each_mut_table_primary_updates_values() {
    let mut world = table_only_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(3)).expect("insert");

    world
        .for_each_mut::<TablePos>(&QuerySpec::new(), QueryParams::new(), |_, pos| {
            pos.0 *= 4;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world
            .get::<TablePos>(entity)
            .expect("get")
            .expect("present")
            .0,
        12
    );
}

#[test]
fn for_each2_mut_without_cache_skips_entities_missing_second() {
    let mut world = sparse_world();
    let pos_only = world.spawn().expect("a");
    let both = world.spawn().expect("b");
    world.insert(pos_only, Position(1)).expect("a");
    world.insert(both, Position(2)).expect("b");
    world.insert(both, Velocity(9)).expect("b");

    world
        .for_each2_mut::<Position, Velocity>(&QuerySpec::new(), QueryParams::new(), |_, pos, _| {
            pos.0 += 100;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(
        world
            .get::<Position>(pos_only)
            .expect("get")
            .expect("present")
            .0,
        1
    );
    assert_eq!(
        world
            .get::<Position>(both)
            .expect("get")
            .expect("present")
            .0,
        102
    );
}

#[test]
fn query2_table_primary_uses_table_traversal() {
    let mut world = table_only_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, TablePos(7)).expect("insert");

    let matches: Vec<_> = world
        .query2::<TablePos, TablePos>(&QuerySpec::new(), QueryParams::new())
        .expect("query2")
        .map(|(_, a, b)| (a.0, b.0))
        .collect();
    assert_eq!(matches, vec![(7, 7)]);
}

#[test]
fn query2_uses_membership_cache() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("pos");
    world.insert(entity, Velocity(2)).expect("vel");

    let spec = QuerySpec::new();
    let cache = world
        .build_query2_cache::<Position, Velocity>(spec.clone())
        .expect("cache");
    let matches: Vec<_> = world
        .query2::<Position, Velocity>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("query2")
        .map(|(_, _, vel)| vel.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query2_uses_result_cache() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(3)).expect("pos");
    world.insert(entity, Velocity(4)).expect("vel");

    let spec = QuerySpec::new();
    let cache = world
        .build_query2_result_cache::<Position, Velocity>(spec.clone())
        .expect("cache");
    let matches: Vec<_> = world
        .query2::<Position, Velocity>(&spec, QueryParams::new().result_cache(&cache))
        .expect("query2")
        .map(|(_, pos, _)| pos.0)
        .collect();
    assert_eq!(matches, vec![3]);
}

#[test]
fn query2_iterator_skips_entities_missing_second_component() {
    let mut world = sparse_world();
    let pos_only = world.spawn().expect("a");
    let both = world.spawn().expect("b");
    world.insert(pos_only, Position(1)).expect("a");
    world.insert(both, Position(2)).expect("b");
    world.insert(both, Velocity(9)).expect("b");

    let matches: Vec<_> = world
        .query2::<Position, Velocity>(&QuerySpec::new(), QueryParams::new())
        .expect("query2")
        .map(|(_, pos, _)| pos.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query_spec_rejects_overlapping_tag_filters() {
    let mut world = tag_world();
    let spec = QuerySpec::new()
        .with_tag::<Player>()
        .without_tag::<Player>();
    assert!(matches!(
        world.query::<Position>(&spec, QueryParams::new()),
        Err(QueryError::ConflictingFilters { .. })
    ));
}

#[test]
#[cfg(feature = "testkit")]
fn query1_rejects_stale_membership_cache_before_iteration() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    world.invalidate_query_cache(&cache);
    assert!(matches!(
        world.query::<Position>(&spec, QueryParams::new().membership_cache(&cache)),
        Err(QueryError::StaleCache)
    ));
}
