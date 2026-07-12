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
            self.channels.push(EventChannel::new(EventRetention::Manual));
        }
        self.channels[index].retention = retention;
    }

    pub fn send<E: Clone + 'static>(
        &mut self,
        event_id: &EventId,
        event: E,
    ) -> Result<(), WorldError> {
        let channel = self
            .channels
            .get_mut(event_id.index())
            .ok_or(WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", event_id.index()),
            })?;
        if channel.closed {
            return Err(WorldError::EventChannelClosed);
        }
        let sequence = channel
            .next_sequence
            .checked_add(1)
            .ok_or(WorldError::EventChannelClosed)?;
        channel.next_sequence = sequence;
        channel.payloads.push(Box::new(event));
        channel.sequences.push(sequence);
        channel.enforce_retention();
        channel.compact();
        Ok(())
    }

    pub fn read_next<E: 'static>(
        &self,
        reader: &mut EventReader<E>,
    ) -> Result<Option<&E>, EventReadError> {
        let channel = self
            .channels
            .get(reader.event_id.index())
            .ok_or(EventReadError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?;
        if channel.closed && reader.cursor.get() >= channel.next_sequence {
            return Err(EventReadError::ChannelClosed);
        }
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
        let channel = self
            .channels
            .get_mut(event_id.index())
            .ok_or(WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", event_id.index()),
            })?;
        let cursor_value = match start {
            EventReaderStart::OldestRetained => channel.oldest_retained,
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
        reader: &EventReader<E>,
    ) -> Result<EventReader<E>, WorldError> {
        let channel = self
            .channels
            .get_mut(reader.event_id.index())
            .ok_or(WorldError::UnregisteredEvent {
                name: alloc::format!("event {}", reader.event_id.index()),
            })?;
        let cursor = Rc::new(Cell::new(reader.cursor.get()));
        channel.cursors.push(Rc::downgrade(&cursor));
        Ok(EventReader {
            owner: reader.owner.clone(),
            event_id: reader.event_id.clone(),
            cursor,
            _marker: PhantomData,
        })
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
                    let removed = self.sequences.remove(0);
                    self.oldest_retained = removed;
                }
            }
            EventRetention::Frame(_) | EventRetention::Manual => {}
        }
    }

    fn compact(&mut self) {
        self.cursors.retain(|weak| weak.strong_count() > 0);
        if self.cursors.is_empty() {
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
            if first >= min_cursor {
                break;
            }
            self.sequences.remove(0);
            self.payloads.remove(0);
            self.oldest_retained = first;
        }
    }
}

impl<E: 'static> EventReader<E> {
    pub fn fork(&mut self, world: &mut crate::world::World) -> Result<Self, WorldError> {
        world.fork_event_reader(self)
    }
}