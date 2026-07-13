use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::component::{
    ComponentId, ComponentOptions, ComponentRegistry, RegistrationError, StorageKind,
};
use crate::event::{
    ComponentLifecycleRegistry, EventId, EventOptions, EventRegistrationError, EventRegistry,
    EventStorage,
};
use crate::resource::ResourceStore;
use crate::storage::{table_column_factory, ArchetypeStorage, SparseStore, TableColumnFactory};
use crate::world::{World, WorldError, WorldEvents, WorldOwner};

type ResourceRegistrar = Box<dyn FnOnce(&mut ResourceStore)>;

fn map_lifecycle_registration_error(error: EventRegistrationError) -> RegistrationError {
    match error {
        EventRegistrationError::TypeConflict {
            name,
            existing,
            requested,
        } => RegistrationError::NameConflict {
            name,
            existing,
            requested,
        },
        EventRegistrationError::InvalidCapacity => RegistrationError::UnsupportedStorage {
            name: alloc::string::String::from("component lifecycle event"),
            detail: alloc::string::String::from("invalid lifecycle event capacity"),
        },
    }
}

/// Checked world schema construction.
pub struct WorldBuilder {
    owner: WorldOwner,
    registry: ComponentRegistry,
    sparse_factories: Vec<Option<Box<dyn FnOnce() -> SparseStore>>>,
    table_factories: Vec<Option<TableColumnFactory>>,
    resource_registrars: Vec<ResourceRegistrar>,
    event_registry: EventRegistry,
    lifecycle_registry: ComponentLifecycleRegistry,
}

impl WorldBuilder {
    pub fn new() -> Self {
        Self {
            owner: WorldOwner::new(),
            registry: ComponentRegistry::new(),
            sparse_factories: Vec::new(),
            table_factories: Vec::new(),
            resource_registrars: Vec::new(),
            event_registry: EventRegistry::new(),
            lifecycle_registry: ComponentLifecycleRegistry::new(),
        }
    }

    pub fn register_component<T: 'static>(
        &mut self,
        options: ComponentOptions,
    ) -> Result<ComponentId, RegistrationError> {
        self.register_component_named::<T>(None, options)
    }

    pub fn register_component_named<T: 'static>(
        &mut self,
        name: Option<&str>,
        options: ComponentOptions,
    ) -> Result<ComponentId, RegistrationError> {
        let id = self
            .registry
            .register_typed::<T>(&self.owner, name, options)?;
        self.ensure_factory_slots(id.index());
        if options.storage() == StorageKind::Sparse {
            if options.is_tag() {
                self.sparse_factories[id.index()] = Some(Box::new(SparseStore::new_tag));
            } else {
                self.sparse_factories[id.index()] =
                    Some(Box::new(|| SparseStore::new_typed::<T>()));
            }
        } else if options.storage() == StorageKind::Table {
            self.table_factories[id.index()] = Some(table_column_factory::<T>());
        }
        self.register_component_lifecycle(id.index())?;
        Ok(id)
    }

    pub fn register_tag(&mut self, name: &str) -> Result<ComponentId, RegistrationError> {
        let id = self
            .registry
            .register_untyped(&self.owner, name, ComponentOptions::tag())?;
        self.ensure_factory_slots(id.index());
        self.sparse_factories[id.index()] = Some(Box::new(SparseStore::new_tag));
        self.register_component_lifecycle(id.index())?;
        Ok(id)
    }

    pub fn register_resource<R: 'static>(&mut self) {
        self.resource_registrars.push(Box::new(|store| {
            store.register::<R>();
        }));
    }

    pub fn register_state<S: Eq + 'static>(&mut self) {
        self.resource_registrars.push(Box::new(|store| {
            store.register_state::<S>();
        }));
    }

    pub fn add_event<E: 'static>(
        &mut self,
        options: EventOptions,
    ) -> Result<EventId, EventRegistrationError> {
        self.event_registry.register::<E>(&self.owner, options)
    }

    pub fn build(mut self) -> Result<World, WorldError> {
        let mut sparse_stores = Vec::with_capacity(self.registry.len());
        for index in 0..self.registry.len() {
            let store = self.sparse_factories[index]
                .take()
                .map(|factory| factory())
                .unwrap_or_else(SparseStore::new_empty);
            sparse_stores.push(store);
        }
        let archetypes = ArchetypeStorage::new(self.table_factories);
        let mut resources = ResourceStore::new();
        for registrar in self.resource_registrars {
            registrar(&mut resources);
        }
        let registry_len = self.event_registry.len();
        let mut events = WorldEvents {
            registry: self.event_registry,
            storage: EventStorage::new(registry_len),
            lifecycle: self.lifecycle_registry,
        };
        for index in 0..registry_len {
            let event_id = EventId::new(self.owner.clone(), index as u32);
            if let Some(options) = events.registry.options(&event_id) {
                events.storage.ensure_channel(index, options.retention());
            }
        }
        events.lifecycle.ensure_storage_channels(
            &mut events.storage,
            &events.registry,
            &self.owner,
        );
        Ok(World::from_parts(
            self.owner,
            self.registry,
            sparse_stores,
            archetypes,
            resources,
            events,
        ))
    }

    fn register_component_lifecycle(
        &mut self,
        component_index: usize,
    ) -> Result<(), RegistrationError> {
        self.lifecycle_registry
            .register_component(&mut self.event_registry, &self.owner, component_index)
            .map_err(map_lifecycle_registration_error)
    }

    fn ensure_factory_slots(&mut self, index: usize) {
        while self.sparse_factories.len() <= index {
            self.sparse_factories.push(None);
            self.table_factories.push(None);
        }
    }
}

impl Default for WorldBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl WorldBuilder {
    pub(crate) fn owner_for_test(&self) -> WorldOwner {
        self.owner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_constructs() {
        let _ = WorldBuilder::default();
    }

    #[test]
    fn lifecycle_registration_error_mapping() {
        assert!(matches!(
            map_lifecycle_registration_error(EventRegistrationError::TypeConflict {
                name: alloc::string::String::from("Added"),
                existing: alloc::string::String::from("a"),
                requested: alloc::string::String::from("b"),
            }),
            RegistrationError::NameConflict { .. }
        ));
        assert!(matches!(
            map_lifecycle_registration_error(
                EventRegistrationError::InvalidCapacity
            ),
            RegistrationError::UnsupportedStorage { name, detail }
                if name == "component lifecycle event"
                    && detail == "invalid lifecycle event capacity"
        ));
    }
}
