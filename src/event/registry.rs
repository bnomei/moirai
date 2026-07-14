//! Checked event registration table and retention policy for one [`crate::world::World`].
//!
//! Ordinary events are looked up by payload type. Lifecycle channels are registered alongside
//! component indices and are excluded from ordinary type lookup.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::{type_name, TypeId};

use crate::operation::StageOperation;
use crate::world::WorldOwner;

/// Dense registry-local event handle scoped to one world owner.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EventId {
    owner: WorldOwner,
    index: u32,
}

/// How long event payloads remain readable before pruning or frame cleanup.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EventRetention {
    /// Cleared at the end of one [`crate::operation::StageOperation`] pass.
    Frame(StageOperation),
    /// Retained until explicitly pruned by bounded policy or channel closure.
    Manual,
    /// Retains at most `capacity` newest payloads; slow readers may lag.
    Bounded(usize),
}

/// Registration-time event retention and source policy.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct EventOptions {
    retention: EventRetention,
    external_source: bool,
}

impl EventOptions {
    /// Frame-scoped retention cleared after the given host operation finishes.
    pub fn frame(operation: StageOperation) -> Self {
        Self {
            retention: EventRetention::Frame(operation),
            external_source: false,
        }
    }

    /// Manual retention until bounded pruning or channel closure.
    pub fn manual() -> Self {
        Self {
            retention: EventRetention::Manual,
            external_source: false,
        }
    }

    /// Bounded ring retention keeping the newest `capacity` payloads.
    pub fn bounded(capacity: usize) -> Result<Self, EventRegistrationError> {
        if capacity == 0 {
            return Err(EventRegistrationError::InvalidCapacity);
        }
        Ok(Self {
            retention: EventRetention::Bounded(capacity),
            external_source: false,
        })
    }

    /// Marks the event as originating outside in-world producers for schedule validation.
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

/// Event registration conflict or invalid retention input.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventRegistrationError {
    /// The same payload type or name was registered with incompatible options.
    TypeConflict {
        /// Registration name associated with the conflict.
        name: String,
        /// Existing entry name.
        existing: String,
        /// Requested entry name.
        requested: String,
    },
    /// Bounded retention capacity must be nonzero.
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
    ordinary_by_type: BTreeMap<TypeId, u32>,
    ordinary_count: usize,
}

const LINEAR_TYPE_LOOKUP_PREFIX: usize = 16;

impl EventId {
    pub(crate) fn new(owner: WorldOwner, index: u32) -> Self {
        Self { owner, index }
    }

    /// Dense registry index for diagnostics and channel lookup.
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
            ordinary_by_type: BTreeMap::new(),
            ordinary_count: 0,
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
        if let Some(index) = self.ordinary_index_of_type_id(type_id) {
            let index = index as usize;
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
        if self.ordinary_count >= LINEAR_TYPE_LOOKUP_PREFIX {
            self.ordinary_by_type.insert(type_id, index);
        }
        self.ordinary_count += 1;
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
        self.id_of_type_id(owner, TypeId::of::<E>())
    }

    pub(crate) fn id_of_type_id(&self, owner: &WorldOwner, type_id: TypeId) -> Option<EventId> {
        self.ordinary_index_of_type_id(type_id)
            .map(|index| EventId::new(owner.clone(), index))
    }

    fn ordinary_index_of_type_id(&self, type_id: TypeId) -> Option<u32> {
        let mut ordinary_entries = 0;
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.lifecycle_component_index.is_some() {
                continue;
            }
            ordinary_entries += 1;
            if entry.type_id == type_id {
                return Some(index as u32);
            }
            if ordinary_entries == LINEAR_TYPE_LOOKUP_PREFIX {
                break;
            }
        }
        self.ordinary_by_type.get(&type_id).copied()
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

    #[derive(Clone, Copy)]
    struct Indexed<const N: usize>;

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

    #[test]
    fn ordinary_type_index_ignores_lifecycle_entries_of_the_same_payload_type() {
        use crate::event::component::LifecycleKind;

        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        let lifecycle = registry
            .register_lifecycle::<Damage>(
                &owner,
                0,
                LifecycleKind::Added,
                EventOptions::frame(StageOperation::Update),
            )
            .expect("lifecycle");
        assert!(registry.id_of::<Damage>(&owner).is_none());

        let ordinary = registry
            .register::<Damage>(&owner, EventOptions::manual())
            .expect("ordinary");
        assert_ne!(ordinary, lifecycle);
        assert_eq!(registry.id_of::<Damage>(&owner), Some(ordinary));
    }

    #[test]
    fn adaptive_type_lookup_resolves_prefix_and_tree_entries() {
        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        macro_rules! register {
            ($index:literal) => {
                registry
                    .register::<Indexed<$index>>(&owner, EventOptions::manual())
                    .expect("register");
            };
        }
        register!(0);
        register!(1);
        register!(2);
        register!(3);
        register!(4);
        register!(5);
        register!(6);
        register!(7);
        register!(8);
        register!(9);
        register!(10);
        register!(11);
        register!(12);
        register!(13);
        register!(14);
        register!(15);
        register!(16);

        assert_eq!(registry.ordinary_count, 17);
        assert_eq!(registry.ordinary_by_type.len(), 1);

        assert_eq!(
            registry.id_of::<Indexed<0>>(&owner).map(|id| id.index()),
            Some(0)
        );
        assert_eq!(
            registry.id_of::<Indexed<16>>(&owner).map(|id| id.index()),
            Some(16)
        );
        assert_eq!(
            registry
                .register::<Indexed<16>>(&owner, EventOptions::manual())
                .expect("duplicate fallback entry")
                .index(),
            16
        );
    }

    fn registry_owner_check(
        id: &EventId,
        owner: &WorldOwner,
    ) -> Result<(), EventRegistrationError> {
        id.validate_owner(owner)
    }
}
