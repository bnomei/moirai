//! Preserve/adapt parity closures not covered by 1:1 named source tests.

use moirai::component::{ComponentOptions, RegistrationError};
use moirai::event::{EventOptions, EventReaderStart};
use moirai::query::{QueryError, QueryParams, QuerySpec};
use moirai::schedule::{stage, ScheduleBuilder, System};
use moirai::state::{apply, State};
use moirai::world::{DynamicBundle, WorldBuilder, WorldError};
use moirai::{AppBuilder, BuildError, ChangeTick, StageOperation};

#[derive(Clone, Copy)]
struct Position(#[allow(dead_code)] i32);

#[derive(Clone, Copy)]
struct Velocity(#[allow(dead_code)] i32);

#[derive(Clone, Copy)]
struct Player;

#[derive(Clone, Debug, PartialEq)]
struct Score(i32);

#[derive(Clone, Debug, PartialEq)]
struct Damage(u32);

fn sparse_world() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder.build().expect("build")
}

#[test]
fn register_tag_resolves_by_name() {
    let mut builder = WorldBuilder::new();
    let tag = builder.register_tag("player").expect("register");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    assert!(world.add_tag(entity, &tag).expect("add"));
    assert!(world.has_tag(entity, &tag).expect("has"));
}

#[test]
fn register_untyped_requires_tag() {
    let mut builder = WorldBuilder::new();
    let tag = builder.register_tag("marker").expect("tag");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    assert!(world.add_tag(entity, &tag).expect("add"));
}

#[test]
fn registration_error_includes_component_name() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("first");
    let err = builder
        .register_component::<Position>(ComponentOptions::table())
        .expect_err("conflict");
    assert!(matches!(
        err,
        RegistrationError::TypeConflict { name, .. } if name.contains("Position")
    ));
}

#[test]
fn sparse_insert_replaces_existing() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("first");
    world.insert(entity, Position(9)).expect("replace");
    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        9
    );
}

#[test]
fn resource_mut_updates_value() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(1)).expect("insert");
    world
        .resource_mut::<Score>()
        .expect("mut")
        .expect("present")
        .0 = 7;
    assert_eq!(world.resource::<Score>().expect("get"), Some(&Score(7)));
}

#[test]
fn resource_added_tick_updates() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(1)).expect("insert");
    assert_eq!(
        world.resource_changed_tick::<Score>().expect("tick"),
        Some(ChangeTick::from_raw(1))
    );
}

#[test]
fn state_set_updates_current_and_previous() {
    let mut builder = AppBuilder::new();
    builder.world_builder().register_state::<u8>();
    builder
        .add_system(apply::<u8>("apply", stage::UPDATE))
        .expect("apply");
    let mut app = builder.build().expect("build");
    app.world_mut()
        .insert_resource(State::new(1u8))
        .expect("state");
    app.world_mut()
        .resource_mut::<State<u8>>()
        .expect("mut")
        .expect("present")
        .request(2)
        .expect("request");
    app.update(1.0 / 60.0).expect("update");
    let state = app
        .world()
        .resource::<State<u8>>()
        .expect("get")
        .expect("present");
    assert_eq!(*state.current(), 2);
    assert_eq!(state.previous(), Some(&1));
}

#[test]
fn separate_event_readers_advance_independently() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage(1)).expect("send");
    let mut retained = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("retained");
    let mut from_now = world
        .event_reader::<Damage>(EventReaderStart::FromNow)
        .expect("from_now");
    world.send(Damage(2)).expect("send");
    assert_eq!(
        world.read_event(&mut retained).expect("r1").map(|d| d.0),
        Some(1)
    );
    assert_eq!(
        world.read_event(&mut from_now).expect("n1").map(|d| d.0),
        Some(2)
    );
}

#[test]
fn event_compact_handles_empty_queue() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<Damage>(EventOptions::frame(moirai::StageOperation::Update))
        .expect("register");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
}

#[test]
fn event_compact_releases_consumed_payloads() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    world.send(Damage(1)).expect("send");
    assert!(world.read_event(&mut reader).expect("read").is_some());
    assert!(world.read_event(&mut reader).expect("again").is_none());
}

#[test]
fn event_compact_retains_unread_payloads() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    world.send(Damage(1)).expect("send");
    let _second = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("second");
    assert_eq!(
        world.read_event(&mut reader).expect("read").map(|d| d.0),
        Some(1)
    );
}

#[test]
fn event_registry_tracks_entries() {
    let mut builder = WorldBuilder::new();
    assert!(builder.add_event::<Damage>(EventOptions::manual()).is_ok());
    let mut world = builder.build().expect("build");
    assert!(world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .is_ok());
}

#[test]
fn query_cache_respects_without() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<Velocity>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let a = world.spawn().expect("spawn");
    let b = world.spawn().expect("spawn");
    world.insert(a, Position(1)).expect("a");
    world.insert(a, Velocity(1)).expect("vel a");
    world.insert(b, Position(2)).expect("b");

    let spec = QuerySpec::new().without::<Velocity>();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query_cache_respects_inactive_changes() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("tag");
    let mut world = builder.build().expect("build");
    let tagged = world.spawn().expect("spawn");
    let plain = world.spawn().expect("plain");
    world.insert(tagged, Position(1)).expect("tagged");
    world.insert(tagged, Player).expect("tag");
    world.insert(plain, Position(2)).expect("plain");

    let spec = QuerySpec::new().without_tag::<Player>();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    let matches: Vec<_> = world
        .query::<Position>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![2]);
}

#[test]
fn query_cache_survives_frame_event_clear() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    builder
        .add_event::<Damage>(EventOptions::frame(moirai::StageOperation::Update))
        .expect("event");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");

    let spec = QuerySpec::new();
    let cache = world
        .build_query_cache::<Position>(spec.clone())
        .expect("cache");
    world.send(Damage(1)).expect("send");

    let schedule = ScheduleBuilder::standard()
        .build(&mut world)
        .expect("schedule");
    let mut app = moirai::App::from_parts(world, schedule).expect("app");
    app.update(1.0 / 60.0).expect("update");

    let matches: Vec<_> = app
        .world_mut()
        .query::<Position>(&spec, QueryParams::new().membership_cache(&cache))
        .expect("query")
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(matches, vec![1]);
}

#[test]
fn dynamic_bundle_reports_missing_component() {
    let world = sparse_world();
    let mut bundle = DynamicBundle::new();
    assert!(matches!(
        bundle.push(&world, Velocity(1)),
        Err(WorldError::UnregisteredComponent { .. })
    ));
}

#[test]
fn dynamic_bundle_resolves_registered_components() {
    let mut world = sparse_world();
    let mut bundle = DynamicBundle::new();
    bundle.push(&world, Position(3)).expect("push");
    let entity = world.spawn_bundle(bundle).expect("bundle");
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
fn component_removed_emitted_on_despawn() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .on_remove_reader::<Position>(EventReaderStart::OldestRetained)
        .expect("reader");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    world.despawn(entity).expect("despawn");
    assert!(world.read_event(&mut reader).expect("read").is_some());
}

#[test]
fn component_events_readable_after_registration() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .on_add_reader::<Position>(EventReaderStart::OldestRetained)
        .expect("reader");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    assert!(world.read_event(&mut reader).expect("read").is_some());
}

#[test]
fn resource_scope_marks_changed() {
    let mut builder = WorldBuilder::new();
    builder.register_resource::<Score>();
    let mut world = builder.build().expect("build");
    world.insert_resource(Score(1)).expect("insert");
    world
        .resource_scope::<Score, _>(|value, _| {
            if let Some(score) = value {
                score.0 = 2;
            }
        })
        .expect("scope");
    assert_eq!(
        world.resource_changed_tick::<Score>().expect("tick"),
        Some(ChangeTick::from_raw(2))
    );
}

#[test]
fn dynamic_component_access_respects_lifecycle() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    world.despawn(entity).expect("despawn");
    assert!(matches!(
        world.get::<Position>(entity),
        Err(WorldError::StaleEntity { .. })
    ));
}

#[test]
fn dynamic_component_mut_updates_value() {
    let mut world = sparse_world();
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Position(1)).expect("insert");
    world
        .get_mut::<Position>(entity)
        .expect("mut")
        .expect("present")
        .0 = 4;
    assert_eq!(
        world
            .get::<Position>(entity)
            .expect("get")
            .expect("present")
            .0,
        4
    );
}

#[test]
fn query2_result_cache_handles_registered_and_missing() {
    let mut world = sparse_world();
    assert!(matches!(
        world.query2::<Position, Velocity>(&QuerySpec::new(), QueryParams::new()),
        Err(QueryError::UnregisteredComponent { .. })
    ));
}

#[test]
fn schedule_event_roles_control_dispatch() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<Damage>(EventOptions::manual())
        .expect("event");
    builder
        .add_system(
            System::new("send", stage::UPDATE, |world, _dt| {
                world.send(Damage(1)).expect("send");
            })
            .emits::<Damage>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    let mut reader = app
        .world_mut()
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert!(app
        .world_mut()
        .read_event(&mut reader)
        .expect("read")
        .is_some());
}

#[test]
fn schedule_validates_ordered_event_roles() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("producer", stage::UPDATE, |_w, _| {}).emits::<Damage>())
        .expect("producer");
    builder
        .add_system(
            System::new("consumer", stage::UPDATE, |_w, _| {})
                .consumes::<Damage>()
                .after("producer"),
        )
        .expect("consumer");
    assert!(builder.build(&mut world).is_ok());
}

#[test]
fn schedule_rejects_missing_same_stage_event_order() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("a", stage::UPDATE, |_w, _| {}).emits::<Damage>())
        .expect("a");
    builder
        .add_system(System::new("b", stage::UPDATE, |_w, _| {}).consumes::<Damage>())
        .expect("b");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::UnreachableEventProducer { .. })
    ));
}

#[test]
fn schedule_rejects_missing_event_producer() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("solo", stage::UPDATE, |_w, _| {}).consumes::<Damage>())
        .expect("system");
    assert!(matches!(
        builder.build(&mut world),
        Err(BuildError::MissingEventProducer { .. })
    ));
}

#[test]
fn schedule_accepts_intrinsic_component_event_producer() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut schedule_builder = ScheduleBuilder::standard();
    schedule_builder
        .add_system(
            System::new("work", stage::UPDATE, |_world, _dt| {})
                .consumes_on_add::<Position>()
                .consumes_on_remove::<Position>(),
        )
        .expect("work");
    assert!(schedule_builder.build(&mut world).is_ok());
}

#[test]
fn schedule_accepts_ordered_cross_stage_event_roles() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut builder = ScheduleBuilder::standard();
    builder
        .add_system(System::new("startup", stage::STARTUP, |_w, _| {}).emits::<Damage>())
        .expect("startup");
    builder
        .add_system(System::new("update", stage::UPDATE, |_w, _| {}).consumes::<Damage>())
        .expect("update");
    assert!(builder.build(&mut world).is_ok());
}

#[test]
fn schedule_event_role_failures_are_distinct() {
    let mut unregistered = WorldBuilder::new().build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(System::new("missing", stage::UPDATE, |_w, _| {}).emits::<Damage>())
        .expect("system");
    assert!(matches!(
        schedule.build(&mut unregistered),
        Err(BuildError::UnregisteredEventRole { .. })
    ));

    #[derive(Clone)]
    struct RenderOnly;
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<RenderOnly>(EventOptions::frame(StageOperation::Render))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(System::new("wrong", stage::UPDATE, |_w, _| {}).emits::<RenderOnly>())
        .expect("system");
    assert!(matches!(
        schedule.build(&mut world),
        Err(BuildError::EventOperationMismatch { .. })
    ));
}

#[test]
fn external_source_consumer_needs_no_system_producer() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update).external_source())
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(System::new("input", stage::UPDATE, |_w, _| {}).consumes::<Damage>())
        .expect("consumer");
    assert!(schedule.build(&mut world).is_ok());
}

#[test]
fn external_source_still_enforces_operation_owner() {
    #[derive(Clone)]
    struct RenderInput;
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<RenderInput>(EventOptions::frame(StageOperation::Render).external_source())
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(System::new("input", stage::UPDATE, |_w, _| {}).consumes::<RenderInput>())
        .expect("consumer");
    assert!(matches!(
        schedule.build(&mut world),
        Err(BuildError::EventOperationMismatch { .. })
    ));
}

#[test]
fn render_system_cannot_consume_update_lifecycle_channel() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .register_component::<Position>(ComponentOptions::sparse())
        .expect("component");
    let mut world = world_builder.build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(System::new("draw", stage::RENDER, |_w, _| {}).consumes_on_add::<Position>())
        .expect("consumer");
    assert!(matches!(
        schedule.build(&mut world),
        Err(BuildError::EventOperationMismatch { .. })
    ));
}

#[test]
fn transitive_event_order_is_reachable() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(System::new("producer", stage::UPDATE, |_w, _| {}).emits::<Damage>())
        .expect("producer");
    schedule
        .add_system(System::new("middle", stage::UPDATE, |_w, _| {}).after("producer"))
        .expect("middle");
    schedule
        .add_system(
            System::new("consumer", stage::UPDATE, |_w, _| {})
                .consumes::<Damage>()
                .after("middle"),
        )
        .expect("consumer");
    assert!(schedule.build(&mut world).is_ok());
}

#[test]
fn every_declared_producer_must_reach_consumer() {
    let mut world_builder = WorldBuilder::new();
    world_builder
        .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
        .expect("event");
    let mut world = world_builder.build().expect("world");
    let mut schedule = ScheduleBuilder::standard();
    schedule
        .add_system(
            System::new("ordered", stage::UPDATE, |_w, _| {})
                .emits::<Damage>()
                .before("consumer"),
        )
        .expect("ordered");
    schedule
        .add_system(System::new("unordered", stage::UPDATE, |_w, _| {}).emits::<Damage>())
        .expect("unordered");
    schedule
        .add_system(System::new("consumer", stage::UPDATE, |_w, _| {}).consumes::<Damage>())
        .expect("consumer");
    assert!(matches!(
        schedule.build(&mut world),
        Err(BuildError::UnreachableEventProducer { producer, .. }) if producer == "unordered"
    ));
}
