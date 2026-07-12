use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::{type_name, TypeId};

use crate::operation::StageOperation;
use crate::world::WorldOwner;

/// Dense registry-local event handle.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EventId {
    owner: WorldOwner,
    index: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EventRetention {
    Frame(StageOperation),
    Manual,
    Bounded(usize),
}

/// Registration-time event retention and policy.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct EventOptions {
    retention: EventRetention,
    external_source: bool,
}

impl EventOptions {
    pub fn frame(operation: StageOperation) -> Self {
        Self {
            retention: EventRetention::Frame(operation),
            external_source: false,
        }
    }

    pub fn manual() -> Self {
        Self {
            retention: EventRetention::Manual,
            external_source: false,
        }
    }

    pub fn bounded(capacity: usize) -> Self {
        Self {
            retention: EventRetention::Bounded(capacity),
            external_source: false,
        }
    }

    pub fn external_source(mut self) -> Self {
        self.external_source = true;
        self
    }

    pub(crate) fn retention(self) -> EventRetention {
        self.retention
    }

    #[allow(dead_code)]
    pub(crate) fn is_external_source(self) -> bool {
        self.external_source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventRegistrationError {
    TypeConflict {
        name: String,
        existing: String,
        requested: String,
    },
}

struct EventEntry {
    name: String,
    #[allow(dead_code)]
    type_id: TypeId,
    options: EventOptions,
}

pub(crate) struct EventRegistry {
    entries: Vec<EventEntry>,
}

impl EventId {
    pub(crate) fn new(owner: WorldOwner, index: u32) -> Self {
        Self { owner, index }
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub(crate) fn validate_owner(&self, owner: &WorldOwner) -> Result<(), EventRegistrationError> {
        if self.owner.same(owner) {
            Ok(())
        } else {
            Err(EventRegistrationError::TypeConflict {
                name: String::from("<event>"),
                existing: String::from("different world"),
                requested: String::from("different world"),
            })
        }
    }
}

impl EventRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn register<E: 'static>(
        &mut self,
        owner: &WorldOwner,
        options: EventOptions,
    ) -> Result<EventId, EventRegistrationError> {
        let name = type_name::<E>().to_string();
        let type_id = TypeId::of::<E>();
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.type_id == type_id)
        {
            return Ok(EventId::new(owner.clone(), index as u32));
        }
        if let Some((_index, entry)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.name == name)
        {
            return Err(EventRegistrationError::TypeConflict {
                name: name.clone(),
                existing: entry.name.clone(),
                requested: name,
            });
        }
        let index = self.entries.len() as u32;
        self.entries.push(EventEntry {
            name,
            type_id,
            options,
        });
        Ok(EventId::new(owner.clone(), index))
    }

    pub fn options(&self, id: &EventId) -> Option<EventOptions> {
        self.entries.get(id.index()).map(|entry| entry.options)
    }

    #[allow(dead_code)]
    pub fn type_id(&self, id: &EventId) -> Option<TypeId> {
        self.entries.get(id.index()).map(|entry| entry.type_id)
    }

    pub fn id_of<E: 'static>(&self, owner: &WorldOwner) -> Option<EventId> {
        let type_id = TypeId::of::<E>();
        self.entries
            .iter()
            .position(|entry| entry.type_id == type_id)
            .map(|index| EventId::new(owner.clone(), index as u32))
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}