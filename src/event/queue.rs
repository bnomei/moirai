use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;
use core::marker::PhantomData;

use crate::event::registry::{EventId, EventRetention};
use crate::operation::StageOperation;
use crate::world::{EventReadError, WorldError, WorldOwner};

#[allow(dead_code)]
pub(crate) struct EventStorage {
    channels: Vec<EventChannel>,
}

struct EventChannel {
    payloads: Vec<Box<dyn Any>>,
    free_payloads: Vec<Box<dyn Any>>,
    sequences: Vec<u64>,
    next_sequence: u64,
    oldest_retained: u64,
    retention: EventRetention,
    cursors: Vec<Weak<Cell<u64>>>,
    closed: bool,
}

/// Explicit reader start policy.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EventReaderStart {
    OldestRetained,
    FromNow,
}

/// Independent typed event reader whose reads own cloned payloads.
pub struct EventReader<E> {
    owner: WorldOwner,
    pub(crate) event_id: EventId,
    cursor: Rc<Cell<u64>>,
    last_payload: Option<Box<dyn Any>>,
    _marker: PhantomData<E>,
}

impl EventStorage {
    pub fn new(capacity: usize) -> Self {
        Self {
            channels: Vec::with_capacity(capacity),
        }
    }

    pub fn ensure_channel(&mut self, index: usize, retention: EventRetention) {
        while self.channels.len() <= index {
            self.channels
                .push(EventChannel::new(EventRetention::Manual));
        }
        self.channels[index].retention = retention;
    }

    pub fn send<E: Clone + 'static>(
        &mut self,
        event_id: &EventId,
        event: E,
    ) -> Result<(), WorldError> {
        let channel = self.channels.get_mut(event_id.index()).ok_or_else(|| {
            WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", event_id.index()),
            }
        })?;
        if channel.closed {
            return Err(WorldError::EventChannelClosed);
        }
        channel.prune_readers();
        let sequence = match channel.next_sequence.checked_add(1) {
            Some(sequence) => sequence,
            None => {
                channel.closed = true;
                return Err(WorldError::EventChannelClosed);
            }
        };
        channel.next_sequence = sequence;
        let payload = channel.take_or_create_payload(event);
        channel.payloads.push(payload);
        channel.sequences.push(sequence);
        channel.enforce_retention();
        Ok(())
    }

    pub fn read_next<'a, E: Clone + 'static>(
        &mut self,
        owner: &WorldOwner,
        reader: &'a mut EventReader<E>,
    ) -> Result<Option<&'a E>, EventReadError> {
        reader.validate_owner(owner)?;
        reader
            .event_id
            .validate_owner(owner)
            .map_err(|_| EventReadError::OwnerMismatch {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?;
        let channel = self
            .channels
            .get_mut(reader.event_id.index())
            .ok_or_else(|| EventReadError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?;
        let cursor = reader.cursor.get();
        if cursor < channel.oldest_retained {
            let dropped = channel.oldest_retained - cursor;
            reader.cursor.set(channel.oldest_retained);
            return Err(EventReadError::Lagged { dropped });
        }
        let position = channel
            .sequences
            .iter()
            .position(|sequence| *sequence > cursor);
        let Some(position) = position else {
            if channel.closed {
                return Err(EventReadError::ChannelClosed);
            }
            return Ok(None);
        };
        let sequence = channel.sequences[position];
        let event = channel.payloads[position]
            .downcast_ref::<E>()
            .ok_or_else(|| EventReadError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?
            .clone();
        reader.last_payload = Some(match reader.last_payload.take() {
            Some(mut payload) => match payload.downcast_mut::<E>() {
                Some(slot) => {
                    *slot = event;
                    payload
                }
                None => {
                    channel.recycle_payload(payload);
                    Box::new(event)
                }
            },
            None => Box::new(event),
        });
        reader.cursor.set(sequence);
        channel.prune_readers();
        Ok(reader
            .last_payload
            .as_ref()
            .and_then(|payload| payload.downcast_ref::<E>()))
    }

    pub fn create_reader<E: Clone + 'static>(
        &mut self,
        owner: WorldOwner,
        event_id: EventId,
        start: EventReaderStart,
    ) -> Result<EventReader<E>, WorldError> {
        event_id
            .validate_owner(&owner)
            .map_err(map_registration_owner_error)?;
        let channel = self.channels.get_mut(event_id.index()).ok_or_else(|| {
            WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", event_id.index()),
            }
        })?;
        let cursor_value = match start {
            EventReaderStart::OldestRetained => channel
                .sequences
                .first()
                .map(|sequence| sequence.saturating_sub(1))
                .unwrap_or(channel.oldest_retained),
            EventReaderStart::FromNow => channel.next_sequence,
        };
        let cursor = Rc::new(Cell::new(cursor_value));
        channel.cursors.push(Rc::downgrade(&cursor));
        channel.prune_readers();
        Ok(EventReader {
            owner,
            event_id,
            cursor,
            last_payload: None,
            _marker: PhantomData,
        })
    }

    pub fn fork_reader<E: Clone + 'static>(
        &mut self,
        owner: &WorldOwner,
        reader: &EventReader<E>,
    ) -> Result<EventReader<E>, WorldError> {
        if let Err(error) = reader.validate_owner(owner) {
            return Err(map_read_owner_error(error));
        }
        reader
            .event_id
            .validate_owner(owner)
            .map_err(map_registration_owner_error)?;
        let channel = self
            .channels
            .get_mut(reader.event_id.index())
            .ok_or_else(|| WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?;
        let cursor = Rc::new(Cell::new(reader.cursor.get()));
        channel.cursors.push(Rc::downgrade(&cursor));
        Ok(EventReader {
            owner: reader.owner.clone(),
            event_id: reader.event_id.clone(),
            cursor,
            last_payload: None,
            _marker: PhantomData,
        })
    }

    #[cfg(test)]
    pub(crate) fn clear_channels_for_test(&mut self) {
        self.channels.clear();
    }

    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn set_channel_state_for_test(
        &mut self,
        index: usize,
        next_sequence: u64,
        closed: bool,
    ) {
        if let Some(channel) = self.channels.get_mut(index) {
            channel.next_sequence = next_sequence;
            channel.closed = closed;
        }
    }

    pub fn clear_frame(&mut self, operation: StageOperation) {
        for channel in &mut self.channels {
            if matches!(channel.retention, EventRetention::Frame(owner) if owner == operation) {
                channel.recycle_payloads();
                channel.sequences.clear();
                channel.oldest_retained = channel.next_sequence;
                channel.prune_readers();
            }
        }
    }
}

impl EventChannel {
    fn new(retention: EventRetention) -> Self {
        Self {
            payloads: Vec::with_capacity(16),
            free_payloads: Vec::with_capacity(16),
            sequences: Vec::with_capacity(16),
            next_sequence: 0,
            oldest_retained: 0,
            retention,
            cursors: Vec::new(),
            closed: false,
        }
    }

    fn enforce_retention(&mut self) {
        match self.retention {
            EventRetention::Bounded(capacity) => {
                while self.payloads.len() > capacity {
                    let payload = self.payloads.remove(0);
                    self.recycle_payload(payload);
                    let _ = self.sequences.remove(0);
                }
                self.refresh_oldest_retained();
            }
            EventRetention::Frame(_) | EventRetention::Manual => {}
        }
    }

    fn take_or_create_payload<E: 'static>(&mut self, event: E) -> Box<dyn Any> {
        if let Some(mut payload) = self.free_payloads.pop() {
            if let Some(slot) = payload.downcast_mut::<E>() {
                *slot = event;
                return payload;
            }
            self.free_payloads.push(payload);
        }
        Box::new(event)
    }

    fn recycle_payload(&mut self, payload: Box<dyn Any>) {
        self.free_payloads.push(payload);
    }

    fn recycle_payloads(&mut self) {
        self.free_payloads.append(&mut self.payloads);
    }

    fn prune_readers(&mut self) {
        let mut index = 0;
        while index < self.cursors.len() {
            if self.cursors[index].strong_count() == 0 {
                self.cursors.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn refresh_oldest_retained(&mut self) {
        self.oldest_retained = self
            .sequences
            .first()
            .map(|sequence| sequence.saturating_sub(1))
            .unwrap_or(self.next_sequence);
    }
}

impl<E: Clone + 'static> EventReader<E> {
    pub fn fork(&mut self, world: &mut crate::world::World) -> Result<Self, WorldError> {
        world.fork_event_reader(self)
    }

    pub(crate) fn validate_owner(&self, owner: &WorldOwner) -> Result<(), EventReadError> {
        if self.owner.same(owner) {
            Ok(())
        } else {
            Err(EventReadError::OwnerMismatch {
                name: alloc::string::String::from("event reader"),
            })
        }
    }
}

fn map_read_owner_error(error: EventReadError) -> WorldError {
    match error {
        EventReadError::OwnerMismatch { name } => WorldError::UnregisteredEvent { name },
        EventReadError::UnregisteredEvent { name } => WorldError::UnregisteredEvent { name },
        EventReadError::Lagged { .. } | EventReadError::ChannelClosed => {
            WorldError::UnregisteredEvent {
                name: alloc::string::String::from("invalid reader state"),
            }
        }
    }
}

fn map_registration_owner_error(
    error: crate::event::registry::EventRegistrationError,
) -> WorldError {
    match error {
        crate::event::registry::EventRegistrationError::TypeConflict { name, .. } => {
            WorldError::UnregisteredEvent { name }
        }
        crate::event::registry::EventRegistrationError::InvalidCapacity => {
            WorldError::UnregisteredEvent {
                name: alloc::string::String::from("invalid event capacity"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;
    use crate::event::EventOptions;
    use crate::world::WorldBuilder;

    #[derive(Clone, Debug, PartialEq)]
    struct Damage(u32);

    #[derive(Clone, Debug, PartialEq)]
    struct Other(u32);

    #[test]
    fn storage_send_read_fork_and_map_errors() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);

        assert!(matches!(
            storage.send(&EventId::new(owner.clone(), 99), Damage(1)),
            Err(WorldError::UnregisteredEvent { .. })
        ));

        storage.send(&event_id, Damage(1)).expect("one");

        let mut wrong = storage
            .create_reader::<Other>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("wrong reader");
        assert!(matches!(
            storage.read_next(&owner, &mut wrong),
            Err(EventReadError::UnregisteredEvent { .. })
        ));

        let mut reader = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("reader");
        assert!(storage
            .read_next(&owner, &mut reader)
            .expect("read")
            .is_some());

        let other_owner = WorldOwner::new();
        assert!(matches!(
            storage.fork_reader(&other_owner, &reader),
            Err(WorldError::UnregisteredEvent { .. })
        ));
        assert!(matches!(
            map_read_owner_error(EventReadError::OwnerMismatch {
                name: String::from("reader")
            }),
            WorldError::UnregisteredEvent { .. }
        ));
        assert!(matches!(
            map_registration_owner_error(
                crate::event::registry::EventRegistrationError::TypeConflict {
                    name: String::from("Damage"),
                    existing: String::from("a"),
                    requested: String::from("b"),
                }
            ),
            WorldError::UnregisteredEvent { .. }
        ));
        assert!(matches!(
            map_registration_owner_error(
                crate::event::registry::EventRegistrationError::InvalidCapacity
            ),
            WorldError::UnregisteredEvent { .. }
        ));
    }

    #[test]
    fn dropping_last_reader_does_not_clear_frame_payloads() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(
            event_id.index(),
            EventRetention::Frame(StageOperation::Update),
        );
        storage.send(&event_id, Damage(1)).expect("send");
        {
            let _reader = storage
                .create_reader::<Damage>(
                    owner.clone(),
                    event_id.clone(),
                    EventReaderStart::OldestRetained,
                )
                .expect("reader");
        }
        storage
            .send(&event_id, Damage(2))
            .expect("send after reader drop");
        let mut reader = storage
            .create_reader::<Damage>(owner.clone(), event_id, EventReaderStart::OldestRetained)
            .expect("late");
        for expected in [1, 2] {
            assert_eq!(
                storage
                    .read_next(&owner, &mut reader)
                    .expect("read")
                    .map(|d| d.0),
                Some(expected)
            );
        }
    }

    #[test]
    fn send_rejects_closed_channel_and_recycles_wrong_payload_type() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        storage.send(&event_id, Other(9)).expect("warm pool");
        storage.send(&event_id, Damage(1)).expect("typed reuse");
        storage.channels[event_id.index()].closed = true;
        assert!(matches!(
            storage.send(&event_id, Damage(2)),
            Err(WorldError::EventChannelClosed)
        ));
    }

    #[test]
    fn create_reader_and_read_next_reject_unregistered_channel() {
        let owner = WorldOwner::new();
        let mut storage = EventStorage::new(0);
        let bogus = EventId::new(owner.clone(), 0);
        assert!(matches!(
            storage.create_reader::<Damage>(
                owner.clone(),
                bogus.clone(),
                EventReaderStart::OldestRetained
            ),
            Err(WorldError::UnregisteredEvent { .. })
        ));
        let mut reader = EventReader::<Damage> {
            owner: owner.clone(),
            event_id: bogus,
            cursor: Rc::new(Cell::new(0)),
            last_payload: None,
            _marker: PhantomData,
        };
        assert!(matches!(
            storage.read_next(&owner, &mut reader),
            Err(EventReadError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn map_read_owner_error_covers_lagged_and_closed() {
        assert!(matches!(
            map_read_owner_error(EventReadError::Lagged { dropped: 2 }),
            WorldError::UnregisteredEvent { .. }
        ));
        assert!(matches!(
            map_read_owner_error(EventReadError::ChannelClosed),
            WorldError::UnregisteredEvent { .. }
        ));
    }

    #[test]
    fn read_next_rejects_event_id_owner_mismatch() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        storage.send(&event_id, Damage(1)).expect("send");

        let mut reader = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("reader");
        reader.event_id = EventId::new(WorldOwner::new(), event_id.index() as u32);
        assert!(matches!(
            storage.read_next(&owner, &mut reader),
            Err(EventReadError::OwnerMismatch { name })
                if name == alloc::format!("event {}", event_id.index())
        ));
    }

    #[test]
    fn fork_reader_rejects_unregistered_channel() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        let reader = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("reader");
        storage.clear_channels_for_test();
        assert!(matches!(
            storage.fork_reader(&owner, &reader),
            Err(WorldError::UnregisteredEvent { name })
                if name == alloc::format!("event {}", event_id.index())
        ));
    }

    #[test]
    fn reader_progress_does_not_clear_frame_payloads() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(
            event_id.index(),
            EventRetention::Frame(StageOperation::Update),
        );
        storage.send(&event_id, Damage(1)).expect("one");
        storage.send(&event_id, Damage(2)).expect("two");
        storage.send(&event_id, Damage(3)).expect("three");
        let reader = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("reader");
        reader.cursor.set(3);
        storage.send(&event_id, Damage(4)).expect("four");
        let mut reader = storage
            .create_reader::<Damage>(owner.clone(), event_id, EventReaderStart::OldestRetained)
            .expect("fresh");
        for expected in [1, 2, 3, 4] {
            assert_eq!(
                storage
                    .read_next(&owner, &mut reader)
                    .expect("read")
                    .map(|d| d.0),
                Some(expected)
            );
        }
    }

    #[test]
    fn map_read_owner_error_covers_unregistered_event() {
        assert!(matches!(
            map_read_owner_error(EventReadError::UnregisteredEvent {
                name: String::from("Damage")
            }),
            WorldError::UnregisteredEvent { name }
                if name == "Damage"
        ));
    }

    #[test]
    fn two_readers_both_observe_same_manual_event() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        storage.send(&event_id, Damage(42)).expect("send");

        let mut reader_a = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("reader a");
        let mut reader_b = storage
            .create_reader::<Damage>(owner.clone(), event_id, EventReaderStart::OldestRetained)
            .expect("reader b");

        assert_eq!(
            storage
                .read_next(&owner, &mut reader_a)
                .expect("read a")
                .map(|d| d.0),
            Some(42)
        );
        assert_eq!(
            storage
                .read_next(&owner, &mut reader_b)
                .expect("read b")
                .map(|d| d.0),
            Some(42)
        );
    }

    #[test]
    fn forked_reader_replays_independently_of_parent() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        storage.send(&event_id, Damage(1)).expect("one");
        storage.send(&event_id, Damage(2)).expect("two");

        let mut parent = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("parent");
        let mut fork = storage.fork_reader(&owner, &parent).expect("fork");
        let _ = storage
            .read_next(&owner, &mut parent)
            .expect("parent consumes one");

        assert_eq!(
            storage
                .read_next(&owner, &mut fork)
                .expect("fork first")
                .map(|d| d.0),
            Some(1)
        );
        assert_eq!(
            storage
                .read_next(&owner, &mut fork)
                .expect("fork second")
                .map(|d| d.0),
            Some(2)
        );
        assert_eq!(
            storage
                .read_next(&owner, &mut parent)
                .expect("parent second")
                .map(|d| d.0),
            Some(2)
        );
    }

    #[test]
    fn late_oldest_retained_reader_receives_all_manual_events_in_order() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        storage.send(&event_id, Damage(10)).expect("one");
        storage.send(&event_id, Damage(20)).expect("two");

        let mut late = storage
            .create_reader::<Damage>(owner.clone(), event_id, EventReaderStart::OldestRetained)
            .expect("late");
        assert_eq!(
            storage
                .read_next(&owner, &mut late)
                .expect("first")
                .map(|d| d.0),
            Some(10)
        );
        assert_eq!(
            storage
                .read_next(&owner, &mut late)
                .expect("second")
                .map(|d| d.0),
            Some(20)
        );
    }

    #[test]
    fn bounded_retention_without_reader_keeps_latest_events() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::bounded(2).expect("bounded"))
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Bounded(2));
        storage.send(&event_id, Damage(1)).expect("one");
        storage.send(&event_id, Damage(2)).expect("two");
        storage.send(&event_id, Damage(3)).expect("three");

        let mut late = storage
            .create_reader::<Damage>(owner.clone(), event_id, EventReaderStart::OldestRetained)
            .expect("late");
        assert_eq!(
            storage
                .read_next(&owner, &mut late)
                .expect("second retained")
                .map(|d| d.0),
            Some(2)
        );
        assert_eq!(
            storage
                .read_next(&owner, &mut late)
                .expect("third retained")
                .map(|d| d.0),
            Some(3)
        );
        assert!(storage
            .read_next(&owner, &mut late)
            .expect("drain")
            .is_none());
    }

    #[test]
    fn bounded_retention_reports_lag_for_slow_reader() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::bounded(1).expect("bounded"))
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Bounded(1));
        let mut reader = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("reader");
        storage.send(&event_id, Damage(1)).expect("one");
        storage.send(&event_id, Damage(2)).expect("two");
        assert!(matches!(
            storage.read_next(&owner, &mut reader),
            Err(EventReadError::Lagged { dropped: 1 })
        ));
    }

    #[test]
    fn frame_clear_reports_lag_and_resets_oldest_reader_start() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::frame(StageOperation::Update))
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(
            event_id.index(),
            EventRetention::Frame(StageOperation::Update),
        );
        let mut existing = storage
            .create_reader::<Damage>(owner.clone(), event_id.clone(), EventReaderStart::FromNow)
            .expect("existing");
        storage.send(&event_id, Damage(1)).expect("one");
        storage.send(&event_id, Damage(2)).expect("two");
        storage.clear_frame(StageOperation::Update);

        assert!(matches!(
            storage.read_next(&owner, &mut existing),
            Err(EventReadError::Lagged { dropped: 2 })
        ));
        assert!(storage
            .read_next(&owner, &mut existing)
            .expect("caught up")
            .is_none());

        let mut late = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("late");
        assert!(storage
            .read_next(&owner, &mut late)
            .expect("starts at boundary")
            .is_none());
        storage.send(&event_id, Damage(3)).expect("three");
        assert_eq!(
            storage
                .read_next(&owner, &mut late)
                .expect("new frame")
                .map(|event| event.0),
            Some(3)
        );
    }

    #[test]
    fn from_now_reader_skips_manual_history() {
        let mut builder = WorldBuilder::new();
        let event_id = builder
            .add_event::<Damage>(EventOptions::manual())
            .expect("register");
        let owner = builder.owner_for_test();
        let mut storage = EventStorage::new(1);
        storage.ensure_channel(event_id.index(), EventRetention::Manual);
        storage.send(&event_id, Damage(1)).expect("one");
        storage.send(&event_id, Damage(2)).expect("two");
        let mut slow = storage
            .create_reader::<Damage>(
                owner.clone(),
                event_id.clone(),
                EventReaderStart::OldestRetained,
            )
            .expect("slow");
        let _ = storage.read_next(&owner, &mut slow).expect("consume one");
        storage.send(&event_id, Damage(3)).expect("three");
        let mut fast = storage
            .create_reader::<Damage>(owner.clone(), event_id.clone(), EventReaderStart::FromNow)
            .expect("fast");
        storage.send(&event_id, Damage(4)).expect("four");
        assert_eq!(
            storage
                .read_next(&owner, &mut fast)
                .expect("read")
                .map(|d| d.0),
            Some(4)
        );
    }
}
