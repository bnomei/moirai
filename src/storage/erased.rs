use alloc::boxed::Box;
use core::any::Any;

use crate::entity::EntityId;
use crate::storage::sparse::SparseSet;
use crate::time::ChangeTick;

#[allow(dead_code)]
pub(crate) trait ErasedSparseStorage {
    fn remove_entity(&mut self, entity: EntityId);
    fn len(&self) -> usize;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) struct TypedSparseStorage<T: 'static> {
    set: SparseSet<T>,
}

#[allow(dead_code)]
impl<T: 'static> TypedSparseStorage<T> {
    pub fn new() -> Self {
        Self {
            set: SparseSet::new(),
        }
    }

    pub fn insert_with_tick(&mut self, entity: EntityId, value: T, tick: ChangeTick) -> Option<T> {
        self.set.insert_with_tick(entity, value, tick)
    }

    pub fn get(&self, entity: EntityId) -> Option<&T> {
        self.set.get(entity)
    }

    pub fn get_mut_with_tick(&mut self, entity: EntityId, tick: ChangeTick) -> Option<&mut T> {
        self.set.get_mut_with_tick(entity, tick)
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.set.contains_slot(entity)
    }

    pub fn remove(&mut self, entity: EntityId) -> Option<T> {
        self.set.remove(entity)
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }
}

impl<T: 'static> ErasedSparseStorage for TypedSparseStorage<T> {
    fn remove_entity(&mut self, entity: EntityId) {
        let _ = self.set.remove(entity);
    }

    fn len(&self) -> usize {
        self.set.len()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[allow(dead_code)]
pub(crate) struct TagSparseStorage {
    set: SparseSet<()>,
}

#[allow(dead_code)]
impl TagSparseStorage {
    pub fn new() -> Self {
        Self {
            set: SparseSet::new(),
        }
    }

    pub fn insert_with_tick(&mut self, entity: EntityId, tick: ChangeTick) -> bool {
        self.set.insert_with_tick(entity, (), tick).is_none()
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.set.contains_slot(entity)
    }

    pub fn remove(&mut self, entity: EntityId) -> bool {
        self.set.remove(entity).is_some()
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }
}

impl ErasedSparseStorage for TagSparseStorage {
    fn remove_entity(&mut self, entity: EntityId) {
        let _ = self.set.remove(entity);
    }

    fn len(&self) -> usize {
        self.set.len()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) enum SparseStore {
    Tag(TagSparseStorage),
    Erased(Box<dyn ErasedSparseStorage>),
    Empty,
}

#[allow(dead_code)]
impl SparseStore {
    pub fn new_tag() -> Self {
        Self::Tag(TagSparseStorage::new())
    }

    pub fn new_typed<T: 'static>() -> Self {
        Self::Erased(Box::new(TypedSparseStorage::<T>::new()))
    }

    pub fn new_empty() -> Self {
        Self::Empty
    }

    pub fn remove_entity(&mut self, entity: EntityId) {
        match self {
            Self::Tag(store) => store.remove_entity(entity),
            Self::Erased(store) => store.remove_entity(entity),
            Self::Empty => {}
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Tag(store) => store.len(),
            Self::Erased(store) => store.len(),
            Self::Empty => 0,
        }
    }

    pub fn typed_mut<T: 'static>(&mut self) -> Option<&mut TypedSparseStorage<T>> {
        match self {
            Self::Erased(store) => store.as_any_mut().downcast_mut(),
            _ => None,
        }
    }

    pub fn typed<T: 'static>(&self) -> Option<&TypedSparseStorage<T>> {
        match self {
            Self::Erased(store) => store.as_any().downcast_ref(),
            _ => None,
        }
    }

    pub fn tag(&self) -> Option<&TagSparseStorage> {
        match self {
            Self::Tag(store) => Some(store),
            _ => None,
        }
    }

    pub fn tag_mut(&mut self) -> Option<&mut TagSparseStorage> {
        match self {
            Self::Tag(store) => Some(store),
            _ => None,
        }
    }

    pub(crate) fn as_erased_mut(&mut self) -> Option<&mut dyn ErasedSparseStorage> {
        match self {
            Self::Erased(store) => Some(store.as_mut()),
            _ => None,
        }
    }
}
