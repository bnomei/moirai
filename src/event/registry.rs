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

    pub fn bounded(capacity: usize) -> Result<Self, EventRegistrationError> {
        if capacity == 0 {
            return Err(EventRegistrationError::InvalidCapacity);
        }
        Ok(Self {
            retention: EventRetention::Bounded(capacity),
            external_source: false,
        })
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

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventRegistrationError {
    TypeConflict {
        name: String,
        existing: String,
        requested: String,
    },
    InvalidCapacity,
}

struct EventEntry {
    name: String,
    type_id: TypeId,
    options: EventOptions,
    #[allow(dead_code)]
    lifecycle_component_index: Option<usize>,
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

    pub fn register<E: Clone + 'static>(
        &mut self,
        owner: &WorldOwner,
        options: EventOptions,
    ) -> Result<EventId, EventRegistrationError> {
        let name = type_name::<E>().to_string();
        let type_id = TypeId::of::<E>();
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.type_id == type_id && entry.lifecycle_component_index.is_none())
        {
            let entry = &self.entries[index];
            if entry.options == options {
                return Ok(EventId::new(owner.clone(), index as u32));
            }
            return Err(EventRegistrationError::TypeConflict {
                name: name.clone(),
                existing: entry.name.clone(),
                requested: name,
            });
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
            lifecycle_component_index: None,
        });
        Ok(EventId::new(owner.clone(), index))
    }

    pub(crate) fn register_lifecycle<E: Clone + 'static>(
        &mut self,
        owner: &WorldOwner,
        component_index: usize,
        kind: crate::event::component::LifecycleKind,
        options: EventOptions,
    ) -> Result<EventId, EventRegistrationError> {
        let name = alloc::format!("__lifecycle_{kind:?}_{component_index}");
        let type_id = TypeId::of::<E>();
        let index = self.entries.len() as u32;
        self.entries.push(EventEntry {
            name,
            type_id,
            options,
            lifecycle_component_index: Some(component_index),
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

    pub fn id_of<E: Clone + 'static>(&self, owner: &WorldOwner) -> Option<EventId> {
        let type_id = TypeId::of::<E>();
        self.entries
            .iter()
            .position(|entry| entry.type_id == type_id && entry.lifecycle_component_index.is_none())
            .map(|index| EventId::new(owner.clone(), index as u32))
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for EventRegistrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TypeConflict {
                name,
                existing,
                requested,
            } => write!(
                f,
                "event registration conflict for {name}: existing={existing}, requested={requested}"
            ),
            Self::InvalidCapacity => f.write_str("event retention capacity must be nonzero"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EventRegistrationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Damage(#[allow(dead_code)] u32);

    #[derive(Clone, Copy)]
    struct Heal(#[allow(dead_code)] u32);

    #[test]
    fn external_source_flag_round_trip() {
        let options = EventOptions::manual().external_source();
        assert!(options.is_external_source());
        assert_eq!(options.retention(), EventRetention::Manual);
    }

    #[test]
    fn default_registry_is_empty() {
        let registry = EventRegistry::default();
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn duplicate_registration_is_idempotent() {
        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        let options = EventOptions::manual();
        let first = registry.register::<Damage>(&owner, options).expect("first");
        let second = registry
            .register::<Damage>(&owner, options)
            .expect("repeat");
        assert_eq!(first, second);
    }

    #[test]
    fn same_name_different_type_is_rejected() {
        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        let name = type_name::<Damage>().to_string();
        registry.entries.push(EventEntry {
            name: name.clone(),
            type_id: TypeId::of::<Heal>(),
            options: EventOptions::manual(),
            lifecycle_component_index: None,
        });
        let err = registry
            .register::<Damage>(&owner, EventOptions::manual())
            .expect_err("conflict");
        assert!(matches!(err, EventRegistrationError::TypeConflict { .. }));
    }

    #[test]
    fn event_id_validate_owner_rejects_foreign_world() {
        let owner_a = WorldOwner::new();
        let owner_b = WorldOwner::new();
        let id = EventId::new(owner_a, 0);
        assert!(registry_owner_check(&id, &owner_b).is_err());
    }

    #[test]
    fn type_id_accessor_returns_registered_type() {
        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        let id = registry
            .register::<Damage>(&owner, EventOptions::manual())
            .expect("register");
        assert_eq!(registry.type_id(&id), Some(TypeId::of::<Damage>()));
    }

    fn registry_owner_check(
        id: &EventId,
        owner: &WorldOwner,
    ) -> Result<(), EventRegistrationError> {
        id.validate_owner(owner)
    }
}
