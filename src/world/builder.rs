use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::component::{
    ComponentId, ComponentOptions, ComponentRegistry, RegistrationError, StorageKind,
};
use crate::storage::{table_column_factory, ArchetypeStorage, SparseStore, TableColumnFactory};
use crate::world::{World, WorldError, WorldOwner};

/// Checked world schema construction.
pub struct WorldBuilder {
    owner: WorldOwner,
    registry: ComponentRegistry,
    sparse_factories: Vec<Option<Box<dyn FnOnce() -> SparseStore>>>,
    table_factories: Vec<Option<TableColumnFactory>>,
}

impl WorldBuilder {
    pub fn new() -> Self {
        Self {
            owner: WorldOwner::new(),
            registry: ComponentRegistry::new(),
            sparse_factories: Vec::new(),
            table_factories: Vec::new(),
        }
    }

    pub fn register_component<T: Clone + 'static>(
        &mut self,
        options: ComponentOptions,
    ) -> Result<ComponentId, RegistrationError> {
        self.register_component_named::<T>(None, options)
    }

    pub fn register_component_named<T: Clone + 'static>(
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
        Ok(id)
    }

    pub fn register_tag(&mut self, name: &str) -> Result<ComponentId, RegistrationError> {
        let id = self
            .registry
            .register_untyped(&self.owner, name, ComponentOptions::tag())?;
        self.ensure_factory_slots(id.index());
        self.sparse_factories[id.index()] = Some(Box::new(SparseStore::new_tag));
        Ok(id)
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
        Ok(World::from_parts(
            self.owner,
            self.registry,
            sparse_stores,
            archetypes,
        ))
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
