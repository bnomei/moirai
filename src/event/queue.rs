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

/// Independent typed event reader with explicit cursor ownership.
pub struct EventReader<E> {
    owner: WorldOwner,
    pub(crate) event_id: EventId,
    cursor: Rc<Cell<u64>>,
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
        let channel =
            self.channels
                .get_mut(event_id.index())
                .ok_or(WorldError::UnregisteredEvent {
                    name: alloc::format!("event {}", event_id.index()),
                })?;
        if channel.closed {
            return Err(WorldError::EventChannelClosed);
        }
        let sequence = match channel.next_sequence.checked_add(1) {
            Some(sequence) => sequence,
            None => {
                channel.closed = true;
                return Err(WorldError::EventChannelClosed);
            }
        };
        channel.next_sequence = sequence;
        channel.payloads.push(Box::new(event));
        channel.sequences.push(sequence);
        channel.enforce_retention();
        channel.compact();
        Ok(())
    }

    pub fn read_next<E: 'static>(
        &self,
        owner: &WorldOwner,
        reader: &mut EventReader<E>,
    ) -> Result<Option<&E>, EventReadError> {
        reader.validate_owner(owner)?;
        reader
            .event_id
            .validate_owner(owner)
            .map_err(|_| EventReadError::OwnerMismatch {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?;
        let channel = self.channels.get(reader.event_id.index()).ok_or(
            EventReadError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            },
        )?;
        let cursor = reader.cursor.get();
        if let Some(&first) = channel.sequences.first() {
            if cursor < first.saturating_sub(1) {
                let dropped = first - cursor - 1;
                reader.cursor.set(first.saturating_sub(1));
                return Err(EventReadError::Lagged { dropped });
            }
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
        reader.cursor.set(sequence);
        Ok(channel.payloads[position].downcast_ref::<E>())
    }

    pub fn create_reader<E: 'static>(
        &mut self,
        owner: WorldOwner,
        event_id: EventId,
        start: EventReaderStart,
    ) -> Result<EventReader<E>, WorldError> {
        event_id
            .validate_owner(&owner)
            .map_err(map_registration_owner_error)?;
        let channel =
            self.channels
                .get_mut(event_id.index())
                .ok_or(WorldError::UnregisteredEvent {
                    name: alloc::format!("event {}", event_id.index()),
                })?;
        let cursor_value = match start {
            EventReaderStart::OldestRetained => channel
                .sequences
                .first()
                .map(|sequence| sequence.saturating_sub(1))
                .unwrap_or(0),
            EventReaderStart::FromNow => channel.next_sequence,
        };
        let cursor = Rc::new(Cell::new(cursor_value));
        channel.cursors.push(Rc::downgrade(&cursor));
        channel.compact();
        Ok(EventReader {
            owner,
            event_id,
            cursor,
            _marker: PhantomData,
        })
    }

    pub fn fork_reader<E: 'static>(
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
        let channel = self.channels.get_mut(reader.event_id.index()).ok_or(
            WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            },
        )?;
        let cursor = Rc::new(Cell::new(reader.cursor.get()));
        channel.cursors.push(Rc::downgrade(&cursor));
        Ok(EventReader {
            owner: reader.owner.clone(),
            event_id: reader.event_id.clone(),
            cursor,
            _marker: PhantomData,
        })
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
                channel.payloads.clear();
                channel.sequences.clear();
                channel.oldest_retained = channel.next_sequence;
                channel.compact();
            }
        }
    }
}

impl EventChannel {
    fn new(retention: EventRetention) -> Self {
        Self {
            payloads: Vec::new(),
            sequences: Vec::new(),
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
                    let _ = self.payloads.remove(0);
                    let _ = self.sequences.remove(0);
                }
                self.refresh_oldest_retained();
            }
            EventRetention::Frame(_) | EventRetention::Manual => {}
        }
    }

    fn compact(&mut self) {
        self.cursors.retain(|weak| weak.strong_count() > 0);
        if self.cursors.is_empty() {
            if !self.payloads.is_empty() {
                self.oldest_retained = self.next_sequence;
                self.payloads.clear();
                self.sequences.clear();
            }
            return;
        }
        let min_cursor = self
            .cursors
            .iter()
            .filter_map(|weak| weak.upgrade())
            .map(|cursor| cursor.get())
            .min()
            .unwrap_or(self.next_sequence);
        while let Some(first) = self.sequences.first().copied() {
            if first > min_cursor {
                break;
            }
            self.sequences.remove(0);
            self.payloads.remove(0);
        }
        self.refresh_oldest_retained();
    }

    fn refresh_oldest_retained(&mut self) {
        self.oldest_retained = self
            .sequences
            .first()
            .map(|sequence| sequence.saturating_sub(1))
            .unwrap_or(self.next_sequence);
    }
}

impl<E: 'static> EventReader<E> {
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
