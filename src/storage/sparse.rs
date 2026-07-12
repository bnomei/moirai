use alloc::vec::Vec;

use crate::entity::EntityId;
use crate::time::ChangeTick;

/// Entity-indexed sparse array with dense iteration and change tracking.
#[allow(dead_code)]
pub(crate) struct SparseSet<T> {
    sparse: Vec<Option<usize>>,
    dense: Vec<u32>,
    data: Vec<T>,
    added: Vec<u64>,
    changed: Vec<u64>,
}

#[allow(dead_code)]
pub(crate) struct SparseIter<'a, T> {
    index: usize,
    set: &'a SparseSet<T>,
}

#[allow(dead_code)]
impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            added: Vec::new(),
            changed: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn dense_slots(&self) -> &[u32] {
        &self.dense
    }

    pub fn dense_slot(&self, index: usize) -> Option<u32> {
        self.dense.get(index).copied()
    }

    pub fn added_tick(&self, index: usize) -> Option<ChangeTick> {
        self.added.get(index).copied().map(ChangeTick::from_raw)
    }

    pub fn changed_tick(&self, index: usize) -> Option<ChangeTick> {
        self.changed.get(index).copied().map(ChangeTick::from_raw)
    }

    pub fn insert_with_tick(
        &mut self,
        entity: EntityId,
        value: T,
        tick: ChangeTick,
    ) -> Option<T> {
        let slot = entity.slot() as usize;
        self.ensure_sparse(slot);
        let raw = tick.raw();
        if let Some(dense_idx) = self.sparse[slot] {
            self.changed[dense_idx] = raw;
            return Some(core::mem::replace(&mut self.data[dense_idx], value));
        }
        let dense_idx = self.dense.len();
        self.dense.push(entity.slot());
        self.data.push(value);
        self.added.push(raw);
        self.changed.push(raw);
        self.sparse[slot] = Some(dense_idx);
        None
    }

    pub fn get(&self, entity: EntityId) -> Option<&T> {
        let dense_idx = self.sparse.get(entity.slot() as usize).and_then(|v| *v)?;
        self.data.get(dense_idx)
    }

    pub fn get_mut_with_tick(&mut self, entity: EntityId, tick: ChangeTick) -> Option<&mut T> {
        let dense_idx = self.sparse.get(entity.slot() as usize).and_then(|v| *v)?;
        self.changed[dense_idx] = tick.raw();
        self.data.get_mut(dense_idx)
    }

    pub fn contains_slot(&self, entity: EntityId) -> bool {
        let slot = entity.slot() as usize;
        self.sparse
            .get(slot)
            .and_then(|v| *v)
            .map(|dense_idx| dense_idx < self.dense.len())
            .unwrap_or(false)
    }

    pub fn remove(&mut self, entity: EntityId) -> Option<T> {
        let slot = entity.slot() as usize;
        let dense_idx = self.sparse.get(slot).and_then(|v| *v)?;
        let last_idx = self.dense.len() - 1;
        self.sparse[slot] = None;

        if dense_idx == last_idx {
            self.dense.pop();
            self.added.pop();
            self.changed.pop();
            return self.data.pop();
        }

        let last_slot = self.dense[last_idx];
        self.dense[dense_idx] = last_slot;
        self.dense.pop();

        let value = self.data.swap_remove(dense_idx);
        let _ = self.added.swap_remove(dense_idx);
        let _ = self.changed.swap_remove(dense_idx);
        self.sparse[last_slot as usize] = Some(dense_idx);
        Some(value)
    }

    pub fn iter(&self) -> SparseIter<'_, T> {
        SparseIter { index: 0, set: self }
    }

    fn ensure_sparse(&mut self, slot: usize) {
        if self.sparse.len() <= slot {
            self.sparse.resize(slot + 1, None);
        }
    }
}

impl<'a, T> Iterator for SparseIter<'a, T> {
    type Item = (u32, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.set.dense.len() {
            return None;
        }
        let idx = self.index;
        self.index += 1;
        Some((self.set.dense[idx], self.set.data.get(idx)?))
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests;