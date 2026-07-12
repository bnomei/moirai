use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;
use core::any::{Any, TypeId};

use crate::time::ChangeTick;
use crate::world::WorldError;

#[allow(dead_code)]
pub(crate) struct ResourceStore {
    registered: Vec<TypeId>,
    registered_names: Vec<String>,
    entries: Vec<Option<ResourceEntry>>,
    locked: Vec<TypeId>,
    scoped: Option<TypeId>,
    scoped_added: Option<ChangeTick>,
    #[allow(clippy::type_complexity)]
    state_transition_readers: BTreeMap<TypeId, fn(&dyn Any) -> Option<ChangeTick>>,
}

struct ResourceEntry {
    value: Box<dyn Any>,
    added: ChangeTick,
    changed: ChangeTick,
}

impl ResourceStore {
    pub fn new() -> Self {
        Self {
            registered: Vec::new(),
            entries: Vec::new(),
            locked: Vec::new(),
            scoped: None,
            scoped_added: None,
            state_transition_readers: BTreeMap::new(),
            registered_names: Vec::new(),
        }
    }

    pub fn register<R: 'static>(&mut self) -> usize {
        let type_id = TypeId::of::<R>();
        if let Some(index) = self.registered.iter().position(|id| *id == type_id) {
            return index;
        }
        let index = self.registered.len();
        self.registered.push(type_id);
        self.registered_names.push(String::from(type_name::<R>()));
        self.entries.push(None);
        index
    }

    pub fn register_state<S: Eq + 'static>(&mut self) {
        let type_id = TypeId::of::<crate::state::State<S>>();
        self.register::<crate::state::State<S>>();
        self.state_transition_readers
            .insert(type_id, |value: &dyn Any| {
                value
                    .downcast_ref::<crate::state::State<S>>()
                    .and_then(|state| state.transition_tick())
            });
    }

    pub fn contains<R: 'static>(&self) -> bool {
        self.contains_type(TypeId::of::<R>())
    }

    pub(crate) fn contains_type(&self, type_id: TypeId) -> bool {
        self.registered
            .iter()
            .position(|id| *id == type_id)
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.is_some())
            .unwrap_or(false)
    }

    pub fn prepare_insert<R: 'static>(&self) -> Result<(), WorldError> {
        self.ensure_accessible::<R>()?;
        self.require_registered::<R>()?;
        Ok(())
    }

    pub fn prepare_mut<R: 'static>(&self) -> Result<bool, WorldError> {
        self.ensure_accessible::<R>()?;
        let index = self.require_registered::<R>()?;
        Ok(self.entries[index].is_some())
    }

    pub fn prepare_scope<R: 'static>(&self) -> Result<(), WorldError> {
        self.ensure_accessible::<R>()?;
        self.require_registered::<R>()?;
        Ok(())
    }

    pub fn take_for_scope<R: 'static>(&mut self) -> Result<Option<R>, WorldError> {
        let type_id = TypeId::of::<R>();
        if self.scoped.is_some() {
            return Err(WorldError::ResourceScoped {
                name: String::from(type_name::<R>()),
            });
        }
        let index = self.require_registered::<R>()?;
        self.scoped = Some(type_id);
        self.scoped_added = self.entries[index].as_ref().map(|entry| entry.added);
        Ok(self.entries[index].take().map(|entry| {
            *entry
                .value
                .downcast::<R>()
                .expect("resource type matches registration")
        }))
    }

    pub fn restore_scope<R: 'static>(
        &mut self,
        value: R,
        tick: ChangeTick,
    ) -> Result<(), WorldError> {
        let type_id = TypeId::of::<R>();
        if self.scoped != Some(type_id) {
            return Err(WorldError::ResourceScoped {
                name: String::from(type_name::<R>()),
            });
        }
        let index = self.require_registered::<R>()?;
        let added = self.scoped_added.take().unwrap_or(tick);
        self.entries[index] = Some(ResourceEntry {
            value: Box::new(value),
            added,
            changed: tick,
        });
        self.scoped = None;
        Ok(())
    }

    pub fn insert<R: 'static>(
        &mut self,
        value: R,
        tick: ChangeTick,
    ) -> Result<Option<R>, WorldError> {
        self.ensure_accessible::<R>()?;
        let index = self.require_registered::<R>()?;
        let previous_added = self.entries[index].as_ref().map(|entry| entry.added);
        let previous = self.entries[index].take().map(|entry| {
            *entry
                .value
                .downcast::<R>()
                .expect("resource type matches registration")
        });
        self.entries[index] = Some(ResourceEntry {
            value: Box::new(value),
            added: previous_added.unwrap_or(tick),
            changed: tick,
        });
        Ok(previous)
    }

    pub fn remove<R: 'static>(&mut self) -> Result<Option<R>, WorldError> {
        self.ensure_accessible::<R>()?;
        if self.is_locked::<R>() {
            return Err(WorldError::ResourceInUse {
                name: String::from(type_name::<R>()),
            });
        }
        let index = self.require_registered::<R>()?;
        Ok(self.entries[index].take().map(|entry| {
            *entry
                .value
                .downcast::<R>()
                .expect("resource type matches registration")
        }))
    }

    pub fn get<R: 'static>(&self) -> Result<Option<&R>, WorldError> {
        self.ensure_accessible::<R>()?;
        let index = self.require_registered::<R>()?;
        Ok(self.entries[index].as_ref().map(|entry| {
            entry
                .value
                .downcast_ref::<R>()
                .expect("resource type match")
        }))
    }

    pub fn get_mut<R: 'static>(&mut self, tick: ChangeTick) -> Result<Option<&mut R>, WorldError> {
        self.ensure_accessible::<R>()?;
        let index = self.require_registered::<R>()?;
        if let Some(entry) = self.entries[index].as_mut() {
            entry.changed = tick;
            return Ok(Some(
                entry
                    .value
                    .downcast_mut::<R>()
                    .expect("resource type match"),
            ));
        }
        Ok(None)
    }

    pub fn added_tick<R: 'static>(&self) -> Result<Option<ChangeTick>, WorldError> {
        self.ensure_accessible::<R>()?;
        let index = self.require_registered::<R>()?;
        Ok(self.entries[index].as_ref().map(|entry| entry.added))
    }

    pub fn changed_tick<R: 'static>(&self) -> Result<Option<ChangeTick>, WorldError> {
        self.ensure_accessible::<R>()?;
        let index = self.require_registered::<R>()?;
        Ok(self.entries[index].as_ref().map(|entry| entry.changed))
    }

    pub fn lock<R: 'static>(&mut self) {
        let type_id = TypeId::of::<R>();
        if !self.locked.contains(&type_id) {
            self.locked.push(type_id);
        }
    }

    pub fn unlock<R: 'static>(&mut self) {
        let type_id = TypeId::of::<R>();
        if let Some(index) = self.locked.iter().position(|locked| *locked == type_id) {
            self.locked.swap_remove(index);
        }
    }

    pub fn lock_type(&mut self, type_id: TypeId) {
        if !self.locked.contains(&type_id) {
            self.locked.push(type_id);
        }
    }

    pub fn unlock_type(&mut self, type_id: TypeId) {
        if let Some(index) = self.locked.iter().position(|locked| *locked == type_id) {
            self.locked.swap_remove(index);
        }
    }

    pub fn type_name(&self, type_id: TypeId) -> Option<&str> {
        self.registered
            .iter()
            .position(|id| *id == type_id)
            .and_then(|index| self.registered_names.get(index))
            .map(String::as_str)
    }

    pub fn added_tick_for(&self, type_id: TypeId) -> Result<Option<ChangeTick>, WorldError> {
        let index = self
            .registered
            .iter()
            .position(|id| *id == type_id)
            .ok_or_else(|| WorldError::UnregisteredResource {
                name: String::from("<resource>"),
            })?;
        Ok(self.entries[index].as_ref().map(|entry| entry.added))
    }

    pub fn changed_tick_for(&self, type_id: TypeId) -> Result<Option<ChangeTick>, WorldError> {
        let index = self
            .registered
            .iter()
            .position(|id| *id == type_id)
            .ok_or_else(|| WorldError::UnregisteredResource {
                name: String::from("<resource>"),
            })?;
        Ok(self.entries[index].as_ref().map(|entry| entry.changed))
    }

    pub fn transition_tick_for(&self, type_id: TypeId) -> Result<Option<ChangeTick>, WorldError> {
        let index = self
            .registered
            .iter()
            .position(|id| *id == type_id)
            .ok_or_else(|| WorldError::UnregisteredResource {
                name: String::from("<resource>"),
            })?;
        let Some(entry) = self.entries[index].as_ref() else {
            return Ok(None);
        };
        let reader = self.state_transition_readers.get(&type_id).copied();
        Ok(reader.and_then(|read| read(entry.value.as_ref())))
    }

    pub fn cancel_scope(&mut self) {
        self.scoped = None;
        self.scoped_added = None;
    }

    fn index_of<R: 'static>(&self) -> Option<usize> {
        let type_id = TypeId::of::<R>();
        self.registered.iter().position(|id| *id == type_id)
    }

    fn require_registered<R: 'static>(&self) -> Result<usize, WorldError> {
        self.index_of::<R>()
            .ok_or_else(|| WorldError::UnregisteredResource {
                name: String::from(type_name::<R>()),
            })
    }

    fn ensure_accessible<R: 'static>(&self) -> Result<(), WorldError> {
        let type_id = TypeId::of::<R>();
        if self.scoped == Some(type_id) {
            return Err(WorldError::ResourceScoped {
                name: String::from(type_name::<R>()),
            });
        }
        Ok(())
    }

    fn is_locked<R: 'static>(&self) -> bool {
        let type_id = TypeId::of::<R>();
        self.locked.contains(&type_id)
    }
}

impl Default for ResourceStore {
    fn default() -> Self {
        Self::new()
    }
}
