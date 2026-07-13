use alloc::boxed::Box;
use core::any::Any;

use crate::entity::EntityId;
use crate::storage::sparse::SparseSet;
use crate::time::ChangeTick;

#[allow(dead_code)]
pub(crate) trait ErasedSparseStorage {
    fn remove_entity(&mut self, entity: EntityId);
    fn contains_entity(&self, entity: EntityId) -> bool;
    fn len(&self) -> usize;
    fn added_tick(&self, entity: EntityId) -> Option<ChangeTick>;
    fn changed_tick(&self, entity: EntityId) -> Option<ChangeTick>;
    fn dense_slots(&self) -> &[u32];
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

    pub fn dense_slots(&self) -> &[u32] {
        self.set.dense_slots()
    }

    pub fn dense_value(&self, index: usize) -> Option<&T> {
        self.set.dense_value(index)
    }

    pub fn added_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        let dense_idx = self.set.dense_index(entity)?;
        self.set.added_tick(dense_idx)
    }

    pub fn changed_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        let dense_idx = self.set.dense_index(entity)?;
        self.set.changed_tick(dense_idx)
    }
}

impl<T: 'static> ErasedSparseStorage for TypedSparseStorage<T> {
    fn remove_entity(&mut self, entity: EntityId) {
        let _ = self.set.remove(entity);
    }

    fn contains_entity(&self, entity: EntityId) -> bool {
        self.set.contains_slot(entity)
    }

    fn len(&self) -> usize {
        self.set.len()
    }

    fn added_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        TypedSparseStorage::added_tick(self, entity)
    }

    fn changed_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        TypedSparseStorage::changed_tick(self, entity)
    }

    fn dense_slots(&self) -> &[u32] {
        self.set.dense_slots()
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

    pub fn added_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        let dense_idx = self.set.dense_index(entity)?;
        self.set.added_tick(dense_idx)
    }

    pub fn changed_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        let dense_idx = self.set.dense_index(entity)?;
        self.set.changed_tick(dense_idx)
    }

    pub fn dense_slots(&self) -> &[u32] {
        self.set.dense_slots()
    }
}

impl ErasedSparseStorage for TagSparseStorage {
    fn remove_entity(&mut self, entity: EntityId) {
        let _ = self.set.remove(entity);
    }

    fn contains_entity(&self, entity: EntityId) -> bool {
        self.set.contains_slot(entity)
    }

    fn len(&self) -> usize {
        self.set.len()
    }

    fn added_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        TagSparseStorage::added_tick(self, entity)
    }

    fn changed_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        TagSparseStorage::changed_tick(self, entity)
    }

    fn dense_slots(&self) -> &[u32] {
        self.set.dense_slots()
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

    pub fn contains_entity(&self, entity: EntityId) -> bool {
        match self {
            Self::Tag(store) => store.contains(entity),
            Self::Erased(store) => store.contains_entity(entity),
            Self::Empty => false,
        }
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

    pub(crate) fn sparse_added_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        match self {
            Self::Tag(store) => store.added_tick(entity),
            Self::Erased(store) => store.added_tick(entity),
            Self::Empty => None,
        }
    }

    pub(crate) fn sparse_changed_tick(&self, entity: EntityId) -> Option<ChangeTick> {
        match self {
            Self::Tag(store) => store.changed_tick(entity),
            Self::Erased(store) => store.changed_tick(entity),
            Self::Empty => None,
        }
    }

    pub(crate) fn dense_slots(&self) -> &[u32] {
        match self {
            Self::Tag(store) => store.dense_slots(),
            Self::Erased(store) => store.dense_slots(),
            Self::Empty => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityId;
    use crate::time::ChangeTick;

    fn entity(slot: u32) -> EntityId {
        EntityId::from_parts(slot, 1)
    }

    #[test]
    fn typed_sparse_erased_trait_round_trip() {
        let mut store = SparseStore::new_typed::<i32>();
        let tick = ChangeTick::from_raw(1);
        let typed = store.typed_mut::<i32>().expect("typed");
        typed.insert_with_tick(entity(1), 7, tick);
        *typed.get_mut_with_tick(entity(1), tick).expect("mut") = 9;
        assert_eq!(store.sparse_added_tick(entity(1)), Some(tick));
        assert_eq!(store.sparse_changed_tick(entity(1)), Some(tick));
        store.remove_entity(entity(1));
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn tag_sparse_store_tracks_membership_and_ticks() {
        let mut store = SparseStore::new_tag();
        let tick = ChangeTick::from_raw(2);
        let tag = store.tag_mut().expect("tag");
        assert!(tag.insert_with_tick(entity(4), tick));
        assert!(store.contains_entity(entity(4)));
        assert_eq!(store.sparse_added_tick(entity(4)), Some(tick));
        store.remove_entity(entity(4));
        assert!(!store.contains_entity(entity(4)));
    }

    #[test]
    fn empty_sparse_store_is_inert() {
        let mut store = SparseStore::new_empty();
        assert_eq!(store.len(), 0);
        assert!(!store.contains_entity(entity(0)));
        assert!(store.dense_slots().is_empty());
        store.remove_entity(entity(0));
        assert!(store.typed::<i32>().is_none());
        assert!(store.tag().is_none());
        assert!(store.as_erased_mut().is_none());
        assert!(store.sparse_added_tick(entity(0)).is_none());
        assert!(store.sparse_changed_tick(entity(0)).is_none());
    }

    #[test]
    fn tag_sparse_storage_implements_erased_trait() {
        let mut tag = TagSparseStorage::new();
        let tick = ChangeTick::from_raw(4);
        assert!(tag.insert_with_tick(entity(5), tick));
        let erased: &mut dyn ErasedSparseStorage = &mut tag;
        assert!(erased.contains_entity(entity(5)));
        assert_eq!(erased.len(), 1);
        assert_eq!(erased.added_tick(entity(5)), Some(tick));
        assert_eq!(erased.changed_tick(entity(5)), Some(tick));
        assert_eq!(erased.dense_slots(), &[5]);
        assert!(erased.as_any().is::<TagSparseStorage>());
        erased.remove_entity(entity(5));
        assert_eq!(erased.len(), 0);
    }

    #[test]
    fn typed_store_exposes_erased_mut_and_changed_tick() {
        let mut store = SparseStore::new_typed::<i32>();
        let tick = ChangeTick::from_raw(5);
        let erased = store.as_erased_mut().expect("erased");
        assert_eq!(erased.len(), 0);
        let typed = store.typed_mut::<i32>().expect("typed");
        typed.insert_with_tick(entity(6), 3, tick);
        *typed.get_mut_with_tick(entity(6), tick).expect("mut") = 4;
        assert_eq!(store.sparse_changed_tick(entity(6)), Some(tick));
    }

    #[test]
    fn tag_sparse_changed_tick_is_exposed_through_store_facade() {
        let mut store = SparseStore::new_tag();
        let tick = ChangeTick::from_raw(6);
        let tag = store.tag_mut().expect("tag");
        assert!(tag.insert_with_tick(entity(7), tick));
        assert_eq!(store.sparse_changed_tick(entity(7)), Some(tick));
    }

    #[test]
    fn typed_store_tag_mut_returns_none() {
        let mut store = SparseStore::new_typed::<i32>();
        assert!(store.tag_mut().is_none());
    }

    #[test]
    fn tag_sparse_as_any_mut_round_trip() {
        let mut tag = TagSparseStorage::new();
        let erased: &mut dyn ErasedSparseStorage = &mut tag;
        assert!(erased.as_any_mut().is::<TagSparseStorage>());
    }

    #[test]
    fn tag_sparse_erased_trait_and_store_facade() {
        let mut store = SparseStore::new_tag();
        let tick = ChangeTick::from_raw(3);
        {
            let tag = store.tag_mut().expect("tag");
            assert!(tag.insert_with_tick(entity(2), tick));
            assert_eq!(tag.len(), 1);
            assert_eq!(tag.added_tick(entity(2)), Some(tick));
            assert_eq!(tag.changed_tick(entity(2)), Some(tick));
            assert_eq!(tag.dense_slots(), &[2]);
        }
        assert!(store.as_erased_mut().is_none());
        assert_eq!(store.dense_slots(), &[2]);
        store.remove_entity(entity(2));
        assert_eq!(store.len(), 0);
        assert!(!store.tag().expect("tag view").contains(entity(2)));
    }
}
