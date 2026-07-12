use moirai::event::{EventOptions, EventReaderStart};
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
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");

    world.send(Damage { amount: 1 }).expect("send");
    world.send(Damage { amount: 2 }).expect("send");

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
        .add_event::<Damage>(EventOptions::bounded(1).expect("bounded"))
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");

    world.send(Damage { amount: 1 }).expect("one");
    world.send(Damage { amount: 2 }).expect("two");
    assert!(matches!(
        world.read_event(&mut reader),
        Err(EventReadError::Lagged { dropped: 1 })
    ));
}

#[test]
fn bounded_zero_capacity_is_rejected_at_registration() {
    assert!(EventOptions::bounded(0).is_err());
}

#[test]
fn duplicate_event_registration_requires_matching_options() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("initial registration");

    assert!(builder
        .add_event::<Damage>(EventOptions::frame(moirai::StageOperation::Update))
        .is_err());
}

#[test]
fn event_reader_rejects_cross_world_reads() {
    let mut builder_a = WorldBuilder::new();
    builder_a
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world_a = builder_a.build().expect("build a");

    let mut builder_b = WorldBuilder::new();
    builder_b
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let world_b = builder_b.build().expect("build b");

    let mut reader = world_a
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    world_a.send(Damage { amount: 1 }).expect("send");
    assert!(matches!(
        world_b.read_event(&mut reader),
        Err(EventReadError::OwnerMismatch { .. })
    ));
}

#[test]
#[cfg(feature = "testkit")]
fn sequence_exhaustion_closes_channel_and_reads_report_closed() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::FromNow)
        .expect("reader");

    world.set_event_sequence_for_test(0, u64::MAX, false);
    assert!(matches!(
        world.send(Damage { amount: 1 }),
        Err(moirai::world::WorldError::EventChannelClosed)
    ));
    assert!(matches!(
        world.read_event(&mut reader),
        Err(EventReadError::ChannelClosed)
    ));
}

#[test]
fn manual_events_remain_readable_after_late_reader_creation() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 1 }).expect("send");

    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    assert_eq!(
        world.read_event(&mut reader).expect("read").cloned(),
        Some(Damage { amount: 1 })
    );
}
