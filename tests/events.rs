use moirai::{EventOptions, EventReaderStart};
use moirai::world::{EventReadError, WorldBuilder};

#[derive(Clone, Debug, PartialEq)]
struct Damage {
    amount: u32,
}

#[test]
fn events_send_and_read_in_order() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");

    world.send(Damage { amount: 1 }).expect("send");
    world.send(Damage { amount: 2 }).expect("send");

    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert_eq!(
        world.read_event(&mut reader).expect("read").cloned(),
        Some(Damage { amount: 1 })
    );
    assert_eq!(
        world.read_event(&mut reader).expect("read").cloned(),
        Some(Damage { amount: 2 })
    );
    assert!(world.read_event(&mut reader).expect("read").is_none());
}

#[test]
fn bounded_retention_reports_lag() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::bounded(1))
        .expect("register");
    let mut world = builder.build().expect("build");

    world.send(Damage { amount: 1 }).expect("one");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    world.send(Damage { amount: 2 }).expect("two");
    assert!(matches!(
        world.read_event(&mut reader),
        Err(EventReadError::Lagged { dropped: 1 })
    ));
}