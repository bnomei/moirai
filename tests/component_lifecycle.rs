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
