mod access;
mod builder;
mod bundle;
mod error;
mod events;
mod flush;
mod owner;

pub(crate) use owner::WorldOwner;
mod query;
mod resources;
mod spawn;

pub use builder::WorldBuilder;
pub use error::{WorldAllocatorError, WorldError};

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;

use crate::component::{ComponentId, ComponentRegistry, StorageKind};
use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::storage::{SparseStore, TypedSparseStorage};
use crate::time::{ChangeTick, ChangeTickError};


/// ECS world with checked sparse-component lifecycle.
pub struct World {
    owner: WorldOwner,
    allocator: EntityAllocator,
    registry: ComponentRegistry,
    sparse_stores: Vec<SparseStore>,
    change_tick: ChangeTick,
}

impl World {
    pub(crate) fn from_parts(
        owner: WorldOwner,
        registry: ComponentRegistry,
        sparse_stores: Vec<SparseStore>,
    ) -> Self {
        Self {
            owner,
            allocator: EntityAllocator::new(),
            registry,
            sparse_stores,
            change_tick: ChangeTick::ZERO,
        }
    }

    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.allocator.is_alive(entity)
    }

    pub fn spawn(&mut self) -> EntityId {
        self.allocator.alloc()
    }

    pub fn despawn(&mut self, entity: EntityId) -> Result<(), WorldError> {
        self.ensure_alive(entity)?;
        for store in &mut self.sparse_stores {
            store.remove_entity(entity);
        }
        self.allocator
            .free(entity)
            .map_err(|error| self.map_allocator_error(entity, error))
    }

    pub fn insert<T: 'static>(
        &mut self,
        entity: EntityId,
        value: T,
    ) -> Result<Option<T>, WorldError> {
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        let tick = self.issue_change_tick()?;
        let store = self.sparse_store_mut::<T>(component_id)?;
        Ok(store.insert_with_tick(entity, value, tick))
    }

    pub fn get<T: 'static>(&self, entity: EntityId) -> Result<Option<&T>, WorldError> {
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        Ok(self.sparse_store::<T>(component_id)?.get(entity))
    }

    pub fn get_mut<T: 'static>(
        &mut self,
        entity: EntityId,
    ) -> Result<Option<&mut T>, WorldError> {
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        let tick = self.issue_change_tick()?;
        Ok(self
            .sparse_store_mut::<T>(component_id)?
            .get_mut_with_tick(entity, tick))
    }

    pub fn remove<T: 'static>(&mut self, entity: EntityId) -> Result<Option<T>, WorldError> {
        self.ensure_alive(entity)?;
        let component_id = self.component_id::<T>()?;
        component_id.validate_owner(&self.owner)?;
        Ok(self.sparse_store_mut::<T>(component_id)?.remove(entity))
    }

    pub fn len_sparse<T: 'static>(&self) -> Result<usize, WorldError> {
        let component_id = self.component_id::<T>()?;
        Ok(self.sparse_store::<T>(component_id)?.len())
    }

    fn component_id<T: 'static>(&self) -> Result<ComponentId, WorldError> {
        self.registry
            .id_of::<T>(&self.owner)
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: String::from(type_name::<T>()),
            })
    }

    fn sparse_store<T: 'static>(
        &self,
        component_id: ComponentId,
    ) -> Result<&TypedSparseStorage<T>, WorldError> {
        self.ensure_sparse_kind(&component_id)?;
        self.sparse_stores
            .get(component_id.index())
            .and_then(|store| store.typed::<T>())
            .ok_or_else(|| WorldError::WrongStorageKind {
                name: String::from(type_name::<T>()),
            })
    }

    fn sparse_store_mut<T: 'static>(
        &mut self,
        component_id: ComponentId,
    ) -> Result<&mut TypedSparseStorage<T>, WorldError> {
        self.ensure_sparse_kind(&component_id)?;
        self.sparse_stores
            .get_mut(component_id.index())
            .and_then(|store| store.typed_mut::<T>())
            .ok_or_else(|| WorldError::WrongStorageKind {
                name: String::from(type_name::<T>()),
            })
    }

    fn ensure_sparse_kind(&self, component_id: &ComponentId) -> Result<(), WorldError> {
        match self.registry.storage_kind(component_id) {
            Some(StorageKind::Sparse) => Ok(()),
            Some(StorageKind::Table) => Err(WorldError::WrongStorageKind {
                name: format!("component {}", component_id.index()),
            }),
            None => Err(WorldError::UnregisteredComponent {
                name: format!("component {}", component_id.index()),
            }),
        }
    }

    fn ensure_alive(&self, entity: EntityId) -> Result<(), WorldError> {
        if self.allocator.is_alive(entity) {
            Ok(())
        } else {
            Err(WorldError::StaleEntity { entity })
        }
    }

    fn issue_change_tick(&mut self) -> Result<ChangeTick, WorldError> {
        self.change_tick
            .issue()
            .map_err(|_: ChangeTickError| WorldError::ChangeTickExhausted)
    }

    fn map_allocator_error(&self, entity: EntityId, error: AllocatorError) -> WorldError {
        match error {
            AllocatorError::GenerationOverflow => {
                WorldError::Allocator(WorldAllocatorError::GenerationOverflow)
            }
            AllocatorError::SlotRetired => WorldError::Allocator(WorldAllocatorError::SlotRetired),
            AllocatorError::StaleEntity | AllocatorError::DoubleFree | AllocatorError::NotLive => {
                WorldError::StaleEntity { entity }
            }
        }
    }
}