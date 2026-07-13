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
    let mut world_b = builder_b.build().expect("build b");

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
fn unregistered_event_send_is_rejected() {
    let mut world = WorldBuilder::new().build().expect("build");
    assert!(matches!(
        world.send(Damage { amount: 1 }),
        Err(moirai::world::WorldError::UnregisteredEvent { .. })
    ));
}

#[test]
fn event_reader_fork_creates_second_reader() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 1 }).expect("one");
    let mut parent = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("parent");
    let _fork = parent.fork(&mut world).expect("fork");
    world.send(Damage { amount: 2 }).expect("two");

    assert_eq!(
        world
            .read_event(&mut parent)
            .expect("parent")
            .map(|d| d.amount),
        Some(1)
    );
    assert_eq!(
        world
            .read_event(&mut parent)
            .expect("parent2")
            .map(|d| d.amount),
        Some(2)
    );
}

#[test]
fn event_payload_pool_reuses_recycled_boxes() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");

    for amount in 1..=4 {
        world.send(Damage { amount }).expect("send");
        assert_eq!(
            world
                .read_event(&mut reader)
                .expect("read")
                .map(|d| d.amount),
            Some(amount)
        );
    }
}

#[test]
fn dropping_all_readers_compacts_frame_channel() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::frame(moirai::StageOperation::Update))
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 1 }).expect("send");
    {
        let _reader = world
            .event_reader::<Damage>(EventReaderStart::OldestRetained)
            .expect("reader");
    }
    world
        .send(Damage { amount: 2 })
        .expect("send after compact");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("late reader");
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("read")
            .map(|d| d.amount),
        Some(2)
    );
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

#[test]
fn two_independent_readers_observe_same_manual_event() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 7 }).expect("send");

    let mut reader_a = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader a");
    let mut reader_b = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader b");

    assert_eq!(
        world
            .read_event(&mut reader_a)
            .expect("read a")
            .map(|d| d.amount),
        Some(7)
    );
    assert_eq!(
        world
            .read_event(&mut reader_b)
            .expect("read b")
            .map(|d| d.amount),
        Some(7)
    );
}

#[test]
fn forked_reader_replays_independently_of_parent() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 1 }).expect("one");
    world.send(Damage { amount: 2 }).expect("two");

    let mut parent = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("parent");
    let mut fork = parent.fork(&mut world).expect("fork");
    assert_eq!(
        world
            .read_event(&mut parent)
            .expect("parent first")
            .map(|d| d.amount),
        Some(1)
    );

    assert_eq!(
        world
            .read_event(&mut fork)
            .expect("fork first")
            .map(|d| d.amount),
        Some(1)
    );
    assert_eq!(
        world
            .read_event(&mut fork)
            .expect("fork second")
            .map(|d| d.amount),
        Some(2)
    );
    assert_eq!(
        world
            .read_event(&mut parent)
            .expect("parent second")
            .map(|d| d.amount),
        Some(2)
    );
}

#[test]
fn late_oldest_retained_reader_receives_all_manual_events_in_order() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 10 }).expect("one");
    world.send(Damage { amount: 20 }).expect("two");

    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("late");
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("first")
            .map(|d| d.amount),
        Some(10)
    );
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("second")
            .map(|d| d.amount),
        Some(20)
    );
}

#[test]
fn manual_history_survives_reader_progress_for_late_reader() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::manual())
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 10 }).expect("one");
    world.send(Damage { amount: 20 }).expect("two");

    let mut early = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("early");
    assert_eq!(
        world
            .read_event(&mut early)
            .expect("read")
            .map(|d| d.amount),
        Some(10)
    );
    drop(early);

    let mut late = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("late");
    assert_eq!(
        world
            .read_event(&mut late)
            .expect("first")
            .map(|d| d.amount),
        Some(10)
    );
    assert_eq!(
        world
            .read_event(&mut late)
            .expect("second")
            .map(|d| d.amount),
        Some(20)
    );
}

#[test]
fn bounded_retention_without_reader_keeps_latest_events() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::bounded(2).expect("bounded"))
        .expect("register");
    let mut world = builder.build().expect("build");
    world.send(Damage { amount: 1 }).expect("one");
    world.send(Damage { amount: 2 }).expect("two");
    world.send(Damage { amount: 3 }).expect("three");

    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("late");
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("second retained")
            .map(|d| d.amount),
        Some(2)
    );
    assert_eq!(
        world
            .read_event(&mut reader)
            .expect("third retained")
            .map(|d| d.amount),
        Some(3)
    );
    assert!(world.read_event(&mut reader).expect("drain").is_none());
}
