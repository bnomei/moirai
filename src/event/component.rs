use alloc::vec::Vec;

use crate::component::ComponentId;
use crate::entity::EntityId;
use crate::event::queue::EventStorage;
use crate::event::registry::{EventId, EventOptions, EventRegistrationError, EventRegistry};
use crate::operation::StageOperation;
use crate::world::{WorldError, WorldOwner};

/// Payload for a committed component addition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentAdded {
    pub entity: EntityId,
    pub component: ComponentId,
}

/// Payload for a committed component removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentRemoved {
    pub entity: EntityId,
    pub component: ComponentId,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleKind {
    Added,
    Removed,
}

pub(crate) struct ComponentLifecycleRegistry {
    added_event_indices: Vec<Option<u32>>,
    removed_event_indices: Vec<Option<u32>>,
}

impl ComponentLifecycleRegistry {
    pub fn new() -> Self {
        Self {
            added_event_indices: Vec::new(),
            removed_event_indices: Vec::new(),
        }
    }

    pub fn register_component(
        &mut self,
        registry: &mut EventRegistry,
        owner: &WorldOwner,
        component_index: usize,
    ) -> Result<(), EventRegistrationError> {
        self.ensure_slots(component_index);
        if self.added_event_indices[component_index].is_none() {
            let id = registry
                .register_lifecycle::<ComponentAdded>(
                    owner,
                    component_index,
                    LifecycleKind::Added,
                    EventOptions::frame(StageOperation::Update),
                )
                .expect("lifecycle added registration is infallible");
            self.added_event_indices[component_index] = Some(id.index() as u32);
        }
        if self.removed_event_indices[component_index].is_none() {
            let id = registry
                .register_lifecycle::<ComponentRemoved>(
                    owner,
                    component_index,
                    LifecycleKind::Removed,
                    EventOptions::frame(StageOperation::Update),
                )
                .expect("lifecycle removed registration is infallible");
            self.removed_event_indices[component_index] = Some(id.index() as u32);
        }
        Ok(())
    }

    pub fn ensure_storage_channels(
        &self,
        storage: &mut EventStorage,
        registry: &EventRegistry,
        owner: &WorldOwner,
    ) {
        for index in 0..self.added_event_indices.len() {
            if let Some(event_index) = self.added_event_indices[index] {
                let event_id = EventId::new(owner.clone(), event_index);
                if let Some(options) = registry.options(&event_id) {
                    storage.ensure_channel(event_index as usize, options.retention())
                }
            }
            if let Some(event_index) = self.removed_event_indices[index] {
                let event_id = EventId::new(owner.clone(), event_index);
                if let Some(options) = registry.options(&event_id) {
                    storage.ensure_channel(event_index as usize, options.retention())
                }
            }
        }
    }

    pub fn emit_added(
        &self,
        storage: &mut EventStorage,
        owner: &WorldOwner,
        entity: EntityId,
        component_index: usize,
    ) -> Result<(), WorldError> {
        let Some(event_index) = self
            .added_event_indices
            .get(component_index)
            .and_then(|id| *id)
        else {
            return Ok(());
        };
        let event_id = EventId::new(owner.clone(), event_index);
        let component = ComponentId::new(owner.clone(), component_index as u32);
        storage.send(&event_id, ComponentAdded { entity, component })
    }

    pub fn emit_removed(
        &self,
        storage: &mut EventStorage,
        owner: &WorldOwner,
        entity: EntityId,
        component_index: usize,
    ) -> Result<(), WorldError> {
        let Some(event_index) = self
            .removed_event_indices
            .get(component_index)
            .and_then(|id| *id)
        else {
            return Ok(());
        };
        let event_id = EventId::new(owner.clone(), event_index);
        let component = ComponentId::new(owner.clone(), component_index as u32);
        storage.send(&event_id, ComponentRemoved { entity, component })
    }

    pub fn added_event_id(&self, owner: &WorldOwner, component_index: usize) -> Option<EventId> {
        self.added_event_indices
            .get(component_index)
            .and_then(|index| *index)
            .map(|index| EventId::new(owner.clone(), index))
    }

    pub fn removed_event_id(&self, owner: &WorldOwner, component_index: usize) -> Option<EventId> {
        self.removed_event_indices
            .get(component_index)
            .and_then(|index| *index)
            .map(|index| EventId::new(owner.clone(), index))
    }

    fn ensure_slots(&mut self, component_index: usize) {
        while self.added_event_indices.len() <= component_index {
            self.added_event_indices.push(None);
            self.removed_event_indices.push(None);
        }
    }

    #[cfg(test)]
    pub(crate) fn clear_added_event_for_test(&mut self, component_index: usize) {
        if let Some(slot) = self.added_event_indices.get_mut(component_index) {
            *slot = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn clear_removed_event_for_test(&mut self, component_index: usize) {
        if let Some(slot) = self.removed_event_indices.get_mut(component_index) {
            *slot = None;
        }
    }
}

impl Default for ComponentLifecycleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentId;

    #[test]
    fn default_registry_has_no_slots() {
        let registry = ComponentLifecycleRegistry::default();
        assert!(registry.added_event_id(&WorldOwner::new(), 0).is_none());
    }

    #[test]
    fn register_component_is_idempotent_for_same_index() {
        let owner = WorldOwner::new();
        let mut events = EventRegistry::new();
        let mut lifecycle = ComponentLifecycleRegistry::new();
        lifecycle
            .register_component(&mut events, &owner, 0)
            .expect("first");
        lifecycle
            .register_component(&mut events, &owner, 0)
            .expect("repeat");
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn emit_without_lifecycle_channel_is_noop() {
        let owner = WorldOwner::new();
        let lifecycle = ComponentLifecycleRegistry::new();
        let mut storage = EventStorage::new(0);
        let entity = EntityId::from_parts(0, 1);
        lifecycle
            .emit_added(&mut storage, &owner, entity, 0)
            .expect("noop added");
        lifecycle
            .emit_removed(&mut storage, &owner, entity, 0)
            .expect("noop removed");
    }

    #[test]
    fn register_component_opens_sparse_lifecycle_slots() {
        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        let mut lifecycle = ComponentLifecycleRegistry::new();
        lifecycle
            .register_component(&mut registry, &owner, 4)
            .expect("register");
        let mut storage = EventStorage::new(0);
        lifecycle.ensure_storage_channels(&mut storage, &registry, &owner);
        let entity = EntityId::from_parts(0, 1);
        let component = ComponentId::new(owner.clone(), 4);
        let added = lifecycle.added_event_id(&owner, 4).expect("added");
        storage
            .send(
                &added,
                ComponentAdded {
                    entity,
                    component: component.clone(),
                },
            )
            .expect("send added");
    }

    #[test]
    fn ensure_storage_channels_opens_registered_lifecycle_events() {
        let owner = WorldOwner::new();
        let mut registry = EventRegistry::new();
        let mut lifecycle = ComponentLifecycleRegistry::new();
        lifecycle
            .register_component(&mut registry, &owner, 0)
            .expect("register");
        let mut storage = EventStorage::new(2);
        lifecycle.ensure_storage_channels(&mut storage, &registry, &owner);
        let entity = EntityId::from_parts(0, 1);
        let component = ComponentId::new(owner.clone(), 0);
        let added = lifecycle.added_event_id(&owner, 0).expect("added");
        storage
            .send(
                &added,
                ComponentAdded {
                    entity,
                    component: component.clone(),
                },
            )
            .expect("send added");
        let removed = lifecycle.removed_event_id(&owner, 0).expect("removed");
        storage
            .send(&removed, ComponentRemoved { entity, component })
            .expect("send removed");

        let empty_registry = EventRegistry::new();
        lifecycle.ensure_storage_channels(&mut storage, &empty_registry, &owner);

        lifecycle.clear_added_event_for_test(0);
        lifecycle.clear_removed_event_for_test(0);
        assert!(lifecycle.added_event_id(&owner, 0).is_none());
        assert!(lifecycle.removed_event_id(&owner, 0).is_none());
        lifecycle.clear_added_event_for_test(99);
        lifecycle.clear_removed_event_for_test(99);
    }
}
