use moirai::component::ComponentOptions;
use moirai::query::{ExactIdPolicy, QueryCursor, QueryError, QueryPolicy, QuerySpec, QueryWindow};
use moirai::world::{World, WorldBuilder};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Position(i32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Marker;

#[derive(Clone, Copy, Debug, PartialEq)]
struct TablePos(i32);

fn world() -> World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register Position");
    builder
        .register_component::<Marker>(ComponentOptions::sparse())
        .expect("register Marker");
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register TablePos");
    builder.build().expect("build")
}

fn position_values(
    query: &mut moirai::query::PreparedQuery1<Position>,
    world: &mut World,
) -> Vec<i32> {
    query
        .iter(world, QueryWindow::All)
        .expect("iterate")
        .map(|(_, position)| position.0)
        .collect()
}

#[test]
fn result_policy_is_reusable_and_refreshes_relevant_topology() {
    let mut world = world();
    let first = world.spawn().expect("first");
    world.insert(first, Position(1)).expect("insert first");
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare");

    assert_eq!(position_values(&mut query, &mut world), vec![1]);
    assert_eq!(position_values(&mut query, &mut world), vec![1]);

    let second = world.spawn().expect("second");
    world.insert(second, Position(2)).expect("insert second");
    assert_eq!(position_values(&mut query, &mut world), vec![1, 2]);

    world.remove::<Position>(first).expect("remove first");
    assert_eq!(position_values(&mut query, &mut world), vec![2]);
}

#[test]
fn result_policy_reads_current_values_and_ignores_unrelated_topology() {
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare");

    world
        .get_mut::<Position>(entity)
        .expect("get")
        .expect("present")
        .0 = 7;
    let unrelated = world.spawn().expect("unrelated");
    world.insert(unrelated, Marker).expect("marker");

    assert_eq!(position_values(&mut query, &mut world), vec![7]);
}

#[test]
fn result_policy_is_owner_scoped() {
    let mut source = world();
    let mut other = world();
    let mut query = source
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare");

    assert!(matches!(
        query.iter(&mut other, QueryWindow::All),
        Err(QueryError::WrongOwner)
    ));
}

#[test]
fn result_policy_rejects_moving_windows() {
    let mut world = world();

    assert!(matches!(
        world
            .prepare_query1::<Position>(QuerySpec::new().added::<Position>(), QueryPolicy::Result,),
        Err(QueryError::MovingChangeWindow)
    ));
    assert!(matches!(
        world.prepare_query1::<Position>(
            QuerySpec::new().changed::<Position>(),
            QueryPolicy::Result,
        ),
        Err(QueryError::MovingChangeWindow)
    ));
}

#[test]
fn result_policy_rejects_exact_order_but_validates_exact_ids_first() {
    let mut other = world();
    let foreign = other.spawn().expect("foreign");
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    let stale = world.spawn().expect("stale");
    world.despawn(stale).expect("despawn stale");

    assert!(matches!(
        world.prepare_query1::<Position>(
            QuerySpec::new().exact_ids(vec![entity, entity], ExactIdPolicy::SkipUnavailable),
            QueryPolicy::Result,
        ),
        Err(QueryError::DuplicateExactId { entity: duplicate }) if duplicate == entity
    ));
    assert!(matches!(
        world.prepare_query1::<Position>(
            QuerySpec::new().exact_ids(vec![foreign], ExactIdPolicy::SkipUnavailable),
            QueryPolicy::Result,
        ),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        world.prepare_query1::<Position>(
            QuerySpec::new().exact_ids(vec![stale], ExactIdPolicy::ErrorOnUnavailable),
            QueryPolicy::Result,
        ),
        Err(QueryError::MissingExactId { entity: missing }) if missing == stale
    ));
    assert!(matches!(
        world.prepare_query1::<Position>(
            QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::SkipUnavailable),
            QueryPolicy::Result,
        ),
        Err(QueryError::UnsupportedCachePolicy { .. })
    ));
}

#[test]
fn result_policy_supports_sparse_table_query2_and_stable_order() {
    let mut world = world();
    let first = world.spawn().expect("first");
    let second = world.spawn().expect("second");
    let sparse_only = world.spawn().expect("sparse only");
    for (entity, value) in [(first, 1), (second, 2)] {
        world.insert(entity, Position(value)).expect("Position");
        world
            .insert(entity, TablePos(value * 10))
            .expect("TablePos");
    }
    world
        .insert(sparse_only, Position(99))
        .expect("sparse only");
    let mut query = world
        .prepare_query2::<Position, TablePos>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare");

    let first_read: Vec<_> = query
        .iter(&mut world, QueryWindow::All)
        .expect("first read")
        .map(|(entity, position, table)| (entity, position.0, table.0))
        .collect();
    let second_read: Vec<_> = query
        .iter(&mut world, QueryWindow::All)
        .expect("second read")
        .map(|(entity, position, table)| (entity, position.0, table.0))
        .collect();
    assert_eq!(first_read, second_read);
    assert_eq!(first_read, vec![(first, 1, 10), (second, 2, 20)]);
}

#[test]
fn result_policy_accepts_execution_windows_and_matching_cursor() {
    let mut world = world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    let spec = QuerySpec::new();
    let mut query = world
        .prepare_query1::<Position>(spec.clone(), QueryPolicy::Result)
        .expect("prepare");
    let now = world.change_tick();

    assert_eq!(
        query
            .iter(&mut world, QueryWindow::Since(now))
            .expect("since")
            .count(),
        1
    );

    let mut cursor =
        QueryCursor::from_spec_start::<Position>(&mut world, &spec).expect("matching cursor");
    let before = cursor.since();
    assert_eq!(
        query
            .iter(&mut world, QueryWindow::Cursor(&mut cursor))
            .expect("cursor")
            .count(),
        1
    );
    assert!(cursor.since() > before);

    let mut wrong =
        QueryCursor::from_spec_start::<Position>(&mut world, &QuerySpec::new().without::<Marker>())
            .expect("wrong cursor");
    assert!(matches!(
        query.iter(&mut world, QueryWindow::Cursor(&mut wrong)),
        Err(QueryError::WrongQuery { .. })
    ));
}

#[test]
fn result_policy_stays_stable_through_slot_reuse() {
    let mut world = world();
    let first = world.spawn().expect("first");
    let second = world.spawn().expect("second");
    for (entity, value) in [(first, 1), (second, 2)] {
        world.insert(entity, Position(value)).expect("insert");
    }
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare");

    world.despawn(first).expect("despawn first");
    let replacement = world.spawn().expect("replacement");
    assert_ne!(replacement, first);
    world
        .insert(replacement, Position(3))
        .expect("insert replacement");

    let first_read: Vec<_> = query
        .iter(&mut world, QueryWindow::All)
        .expect("first read")
        .map(|(entity, position)| (entity, position.0))
        .collect();
    let second_read: Vec<_> = query
        .iter(&mut world, QueryWindow::All)
        .expect("second read")
        .map(|(entity, position)| (entity, position.0))
        .collect();
    assert_eq!(first_read, second_read);
    assert_eq!(first_read.len(), 2);
    assert!(first_read.contains(&(second, 2)));
    assert!(first_read.contains(&(replacement, 3)));
}

#[test]
fn result_policy_mutation_uses_cached_result_members() {
    let mut world = world();
    let first = world.spawn().expect("first");
    let second = world.spawn().expect("second");
    world.insert(first, Position(1)).expect("first position");
    world.insert(second, Position(2)).expect("second position");
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare");

    query
        .for_each_mut(&mut world, QueryWindow::All, |_, position| {
            position.0 *= 2;
            Ok(())
        })
        .expect("mutate");

    assert_eq!(position_values(&mut query, &mut world), vec![2, 4]);
}
