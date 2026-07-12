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
        .query::<Position>(spec, QueryParams::new())
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
        .query::<Position>(spec, QueryParams::new())
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
        .query2::<Position, Velocity>(spec, QueryParams::new())
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
        .query2::<Position, TablePos>(spec, QueryParams::new())
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
        .query::<TablePos>(spec, QueryParams::new())
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
        .query::<Position>(spec, QueryParams::new())
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
        .query::<Position>(spec, QueryParams::new())
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
        .query::<Position>(spec, QueryParams::new().since(since_after_a))
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
        .query::<Position>(spec, QueryParams::new().since(since_after_insert))
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
        world.query::<TablePos>(spec, QueryParams::new()),
        Err(QueryError::UnregisteredComponent { .. })
    ));
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
        .query::<Position>(spec, QueryParams::new())
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
fn for_each_mut_updates_sparse_components() {
    let mut world = sparse_world();
    let a = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("insert");

    world
        .for_each_mut::<Position>(QuerySpec::new(), QueryParams::new(), |_, pos| {
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
        .for_each2_mut::<Position, Velocity>(QuerySpec::new(), QueryParams::new(), |_, pos, vel| {
            pos.0 += vel.0;
            Ok(())
        })
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
            QuerySpec::new(),
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
        world.query::<Position>(spec, QueryParams::new()),
        Err(QueryError::MissingExactId { .. })
    ));
}

#[test]
fn query_rejects_non_tag_with_tag_filter() {
    let mut world = sparse_world();
    let spec = QuerySpec::new().with_tag::<Position>();
    assert!(matches!(
        world.query::<Position>(spec, QueryParams::new()),
        Err(QueryError::WrongStorageKind { .. })
    ));
}

#[test]
fn query_cursor_from_spec_without_testkit() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let spec = QuerySpec::new().added::<Position>();
    let mut cursor = QueryCursor::from_spec_start::<Position>(&world, &spec).expect("cursor");
    let params = QueryParams::new().cursor(&mut cursor);
    let mut query = world.query::<Position>(spec, params).expect("query");
    assert!(query.next().is_some());
}

#[test]
fn query_cursor_rejects_an_exact_id_spec_with_a_different_policy() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let skip_spec = QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::SkipUnavailable);
    let mut cursor = QueryCursor::from_spec_start::<Position>(&world, &skip_spec).expect("cursor");
    let error_spec = QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::ErrorOnUnavailable);

    assert!(matches!(
        world.query::<Position>(error_spec, QueryParams::new().cursor(&mut cursor)),
        Err(QueryError::WrongQuery { .. })
    ));
}

#[test]
fn query_effects_rejects_commands_while_idle() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let result = world.for_each_mut_with_effects::<Position>(
        QuerySpec::new(),
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
        .query::<Position>(spec, QueryParams::new())
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}
