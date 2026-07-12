use super::*;
use crate::entity::EntityAllocator;
use crate::time::ChangeTick;
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