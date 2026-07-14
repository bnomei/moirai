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
fn membership_policies_are_reusable_and_track_relevant_topology() {
    for policy in [QueryPolicy::Membership, QueryPolicy::DeltaMembership] {
        let mut world = world();
        let first = world.spawn().expect("spawn first");
        world.insert(first, Position(1)).expect("insert first");
        let mut query = world
            .prepare_query1::<Position>(QuerySpec::new(), policy)
            .expect("prepare");

        assert_eq!(position_values(&mut query, &mut world), vec![1]);
        assert_eq!(position_values(&mut query, &mut world), vec![1]);

        let second = world.spawn().expect("spawn second");
        world.insert(second, Position(2)).expect("insert second");
        assert_eq!(position_values(&mut query, &mut world), vec![1, 2]);

        world.remove::<Position>(first).expect("remove first");
        assert_eq!(position_values(&mut query, &mut world), vec![2]);
    }
}

#[test]
fn membership_policies_observe_values_without_structural_changes() {
    for policy in [QueryPolicy::Membership, QueryPolicy::DeltaMembership] {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");
        let mut query = world
            .prepare_query1::<Position>(QuerySpec::new(), policy)
            .expect("prepare");

        world
            .get_mut::<Position>(entity)
            .expect("get")
            .expect("present")
            .0 = 9;
        assert_eq!(position_values(&mut query, &mut world), vec![9]);

        let unrelated = world.spawn().expect("spawn unrelated");
        world.insert(unrelated, Marker).expect("insert unrelated");
        assert_eq!(position_values(&mut query, &mut world), vec![9]);
    }
}

#[test]
fn membership_policies_are_owner_scoped() {
    for policy in [QueryPolicy::Membership, QueryPolicy::DeltaMembership] {
        let mut source = world();
        let mut other = world();
        let mut query = source
            .prepare_query1::<Position>(QuerySpec::new(), policy)
            .expect("prepare");

        assert!(matches!(
            query.iter(&mut other, QueryWindow::All),
            Err(QueryError::WrongOwner)
        ));
    }
}

#[test]
fn materialized_membership_rejects_exact_order_but_validates_exact_ids_first() {
    for policy in [QueryPolicy::Membership, QueryPolicy::DeltaMembership] {
        let mut other = world();
        let foreign = other.spawn().expect("foreign");
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");
        let stale = world.spawn().expect("stale");
        world.despawn(stale).expect("despawn stale");

        assert!(matches!(
            world.prepare_query1::<Position>(
                QuerySpec::new().exact_ids(
                    vec![entity, entity],
                    ExactIdPolicy::SkipUnavailable,
                ),
                policy,
            ),
            Err(QueryError::DuplicateExactId { entity: duplicate }) if duplicate == entity
        ));
        assert!(matches!(
            world.prepare_query1::<Position>(
                QuerySpec::new().exact_ids(vec![foreign], ExactIdPolicy::SkipUnavailable),
                policy,
            ),
            Err(QueryError::WrongOwner)
        ));
        assert!(matches!(
            world.prepare_query1::<Position>(
                QuerySpec::new().exact_ids(vec![stale], ExactIdPolicy::ErrorOnUnavailable),
                policy,
            ),
            Err(QueryError::MissingExactId { entity: missing }) if missing == stale
        ));
        assert!(matches!(
            world.prepare_query1::<Position>(
                QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::SkipUnavailable),
                policy,
            ),
            Err(QueryError::UnsupportedCachePolicy { .. })
        ));
    }
}

#[test]
fn membership_applies_since_and_cursor_windows_at_execution() {
    for policy in [QueryPolicy::Membership, QueryPolicy::DeltaMembership] {
        let mut world = world();
        let old = world.spawn().expect("spawn old");
        world.insert(old, Position(1)).expect("insert old");
        let since_after_old = world.change_tick();
        let new = world.spawn().expect("spawn new");
        world.insert(new, Position(2)).expect("insert new");
        let spec = QuerySpec::new().added::<Position>();
        let mut query = world
            .prepare_query1::<Position>(spec.clone(), policy)
            .expect("prepare");

        let narrow: Vec<_> = query
            .iter(&mut world, QueryWindow::Since(since_after_old))
            .expect("since")
            .map(|(_, position)| position.0)
            .collect();
        assert_eq!(narrow, vec![2]);

        let mut cursor =
            QueryCursor::from_spec_start::<Position>(&mut world, &spec).expect("cursor");
        let before = cursor.since();
        let wide: Vec<_> = query
            .iter(&mut world, QueryWindow::Cursor(&mut cursor))
            .expect("cursor iteration")
            .map(|(_, position)| position.0)
            .collect();
        assert_eq!(wide, vec![1, 2]);
        assert!(cursor.since() > before);
    }
}

#[test]
fn membership_cursor_must_match_the_prepared_query() {
    let mut world = world();
    let spec = QuerySpec::new().changed::<Position>();
    let mut query = world
        .prepare_query1::<Position>(spec, QueryPolicy::Membership)
        .expect("prepare");
    let mut wrong =
        QueryCursor::from_spec_start::<Position>(&mut world, &QuerySpec::new()).expect("cursor");

    assert!(matches!(
        query.iter(&mut world, QueryWindow::Cursor(&mut wrong)),
        Err(QueryError::WrongQuery { .. })
    ));
}

#[test]
fn membership_supports_sparse_table_query2_with_repeatable_results() {
    for policy in [QueryPolicy::Membership, QueryPolicy::DeltaMembership] {
        let mut world = world();
        let first = world.spawn().expect("first");
        let second = world.spawn().expect("second");
        let table_only = world.spawn().expect("table only");
        for (entity, value) in [(first, 1), (second, 2)] {
            world.insert(entity, Position(value)).expect("Position");
            world
                .insert(entity, TablePos(value * 10))
                .expect("TablePos");
        }
        world.insert(table_only, TablePos(99)).expect("table only");
        let mut query = world
            .prepare_query2::<Position, TablePos>(QuerySpec::new(), policy)
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
        let mut actual = first_read;
        actual.sort_unstable_by_key(|(_, position, _)| *position);
        assert_eq!(actual, vec![(first, 1, 10), (second, 2, 20)]);
    }
}

#[test]
fn delta_membership_preserves_the_set_and_repeatability_through_slot_reuse() {
    let mut world = world();
    let first = world.spawn().expect("first");
    let second = world.spawn().expect("second");
    let third = world.spawn().expect("third");
    for (entity, value) in [(first, 1), (second, 2), (third, 3)] {
        world.insert(entity, Position(value)).expect("insert");
    }
    let mut query = world
        .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::DeltaMembership)
        .expect("prepare");

    world.remove::<Position>(first).expect("remove first");
    world.despawn(second).expect("despawn second");
    let replacement = world.spawn().expect("replacement");
    assert_ne!(replacement, second);
    world
        .insert(replacement, Position(4))
        .expect("insert replacement");

    let first_read: Vec<_> = query
        .iter(&mut world, QueryWindow::All)
        .expect("iterate")
        .map(|(entity, position)| (entity, position.0))
        .collect();
    let second_read: Vec<_> = query
        .iter(&mut world, QueryWindow::All)
        .expect("iterate again")
        .map(|(entity, position)| (entity, position.0))
        .collect();
    assert_eq!(second_read, first_read);
    let mut actual = first_read;
    actual.sort_unstable_by_key(|(_, position)| *position);
    assert_eq!(actual, vec![(third, 3), (replacement, 4)]);
}

#[test]
fn membership_mutation_uses_the_selected_window() {
    let mut world = world();
    let old = world.spawn().expect("old");
    world.insert(old, Position(1)).expect("old position");
    let since_after_old = world.change_tick();
    let new = world.spawn().expect("new");
    world.insert(new, Position(2)).expect("new position");
    let mut query = world
        .prepare_query1::<Position>(
            QuerySpec::new().added::<Position>(),
            QueryPolicy::DeltaMembership,
        )
        .expect("prepare");

    query
        .for_each_mut(
            &mut world,
            QueryWindow::Since(since_after_old),
            |_, position| {
                position.0 += 10;
                Ok(())
            },
        )
        .expect("mutate");

    assert_eq!(world.get::<Position>(old).expect("get").expect("old").0, 1);
    assert_eq!(world.get::<Position>(new).expect("get").expect("new").0, 12);
}
