use moirai::component::ComponentOptions;
use moirai::event::EventReaderStart;
use moirai::world::WorldBuilder;

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Health(i32);

#[test]
fn component_added_emitted_after_commit() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .on_add_reader::<Health>(EventReaderStart::OldestRetained)
        .expect("reader");

    let entity = world.spawn().expect("spawn");
    world.insert(entity, Health(3)).expect("insert");

    let event = world.read_event(&mut reader).expect("read").expect("event");
    assert_eq!(event.entity, entity);
}

#[test]
fn deferred_insert_emits_after_flush() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .on_add_reader::<Health>(EventReaderStart::OldestRetained)
        .expect("reader");

    let entity = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    world
        .commands()
        .expect("commands")
        .insert(entity, Health(9))
        .expect("queue");
    world.flush().expect("flush");

    let event = world.read_event(&mut reader).expect("read").expect("event");
    assert_eq!(event.entity, entity);
    assert!(world.is_alive(entity));
}

#[test]
fn replacement_does_not_emit_second_add() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .on_add_reader::<Health>(EventReaderStart::OldestRetained)
        .expect("reader");

    let entity = world.spawn().expect("spawn");
    world.insert(entity, Health(1)).expect("first");
    world.insert(entity, Health(2)).expect("replace");

    assert!(world.read_event(&mut reader).expect("one").is_some());
    assert!(world.read_event(&mut reader).expect("two").is_none());
}

#[test]
fn deferred_remove_of_absent_sparse_component_emits_nothing() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut remove_reader = world
        .on_remove_reader::<Health>(EventReaderStart::OldestRetained)
        .expect("reader");

    let entity = world
        .commands()
        .expect("commands")
        .spawn()
        .expect("reserve");
    world
        .commands()
        .expect("commands")
        .remove::<Health>(entity)
        .expect("queue");
    world.flush().expect("flush");

    assert!(world
        .read_event(&mut remove_reader)
        .expect("read")
        .is_none());
}

#[test]
fn late_lifecycle_reader_observes_all_additions_in_order() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");

    let first = world.spawn().expect("first spawn");
    world.insert(first, Health(1)).expect("first insert");
    let second = world.spawn().expect("second spawn");
    world.insert(second, Health(2)).expect("second insert");

    let mut reader = world
        .on_add_reader::<Health>(EventReaderStart::OldestRetained)
        .expect("late reader");
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("first read")
            .expect("first event")
            .entity,
        first
    );
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("second read")
            .expect("second event")
            .entity,
        second
    );
}
