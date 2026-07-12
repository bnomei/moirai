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
            let id = registry.register_lifecycle::<ComponentAdded>(
                owner,
                component_index,
                LifecycleKind::Added,
                EventOptions::frame(StageOperation::Update),
            )?;
            self.added_event_indices[component_index] = Some(id.index() as u32);
        }
        if self.removed_event_indices[component_index].is_none() {
            let id = registry.register_lifecycle::<ComponentRemoved>(
                owner,
                component_index,
                LifecycleKind::Removed,
                EventOptions::frame(StageOperation::Update),
            )?;
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
                    storage.ensure_channel(event_index as usize, options.retention());
                }
            }
            if let Some(event_index) = self.removed_event_indices[index] {
                let event_id = EventId::new(owner.clone(), event_index);
                if let Some(options) = registry.options(&event_id) {
                    storage.ensure_channel(event_index as usize, options.retention());
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
}

impl Default for ComponentLifecycleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
