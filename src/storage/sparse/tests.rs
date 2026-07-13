use super::*;
use crate::entity::EntityAllocator;
use crate::time::ChangeTick;
use alloc::vec;
use rstest::rstest;

#[rstest]
fn swap_remove_repairs_reverse_indices() {
    let mut alloc = EntityAllocator::new();
    let a = alloc.alloc();
    let b = alloc.alloc();
    let c = alloc.alloc();
    let mut set = SparseSet::new();
    set.insert_with_tick(a, 1u32, ChangeTick::ZERO);
    set.insert_with_tick(b, 2u32, ChangeTick::ZERO);
    set.insert_with_tick(c, 3u32, ChangeTick::ZERO);
    assert_eq!(set.remove(b), Some(2));
    assert_eq!(set.get(a), Some(&1));
    assert_eq!(set.get(c), Some(&3));
    assert_eq!(set.len(), 2);
}

#[rstest]
fn insert_replace_and_iteration_cover_dense_paths() {
    let mut alloc = EntityAllocator::new();
    let a = alloc.alloc();
    let b = alloc.alloc();
    let tick = ChangeTick::from_raw(3);
    let later = ChangeTick::from_raw(4);
    let mut set = SparseSet::new();
    assert!(set.insert_with_tick(a, 1u32, tick).is_none());
    assert_eq!(set.insert_with_tick(a, 2u32, later), Some(1));
    assert_eq!(set.get(a), Some(&2));
    assert_eq!(set.added_tick(0), Some(tick));
    assert_eq!(set.changed_tick(0), Some(later));
    assert_eq!(set.dense_slot(0), Some(a.slot()));
    assert!(set.contains_slot(a));
    assert!(!set.contains_slot(b));
    assert_eq!(set.dense_index(a), Some(0));
    assert_eq!(set.iter().collect::<Vec<_>>(), vec![(a.slot(), &2u32)]);
    assert_eq!(set.remove(a), Some(2));
    assert_eq!(set.len(), 0);
    assert_eq!(SparseSet::<u32>::default().len(), 0);
}
