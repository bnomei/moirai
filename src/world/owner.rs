use alloc::rc::Rc;
use core::hash::{Hash, Hasher};

#[derive(Clone)]
pub(crate) struct WorldOwner(Rc<()>);

impl WorldOwner {
    pub fn new() -> Self {
        Self(Rc::new(()))
    }

    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for WorldOwner {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}

impl Eq for WorldOwner {}

impl Hash for WorldOwner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

impl core::fmt::Debug for WorldOwner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("WorldOwner")
            .field(&Rc::as_ptr(&self.0))
            .finish()
    }
}

impl Default for WorldOwner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hash::{Hash, Hasher};

    struct PtrHasher(u64);

    impl Hasher for PtrHasher {
        fn finish(&self) -> u64 {
            self.0
        }

        fn write(&mut self, _: &[u8]) {}

        fn write_usize(&mut self, i: usize) {
            self.0 = i as u64;
        }
    }

    #[test]
    fn same_owner_is_equal_and_hashes_consistently() {
        let a = WorldOwner::new();
        let b = a.clone();
        assert_eq!(a, b);
        let mut hasher_a = PtrHasher(0);
        let mut hasher_b = PtrHasher(0);
        a.hash(&mut hasher_a);
        b.hash(&mut hasher_b);
        assert_eq!(hasher_a.finish(), hasher_b.finish());
    }

    #[test]
    fn different_owners_are_not_equal() {
        let a = WorldOwner::new();
        let b = WorldOwner::new();
        assert_ne!(a, b);
        assert!(!a.same(&b));
    }

    #[test]
    fn default_constructed_owner_is_well_formed() {
        let owner = WorldOwner::default();
        assert!(owner.same(&owner));
    }

    #[test]
    fn debug_formats_as_pointer_tuple() {
        let owner = WorldOwner::new();
        let text = alloc::format!("{owner:?}");
        assert!(text.starts_with("WorldOwner("));
        assert!(text.ends_with(')'));
    }

    #[test]
    fn ptr_hasher_records_usize_writes() {
        let owner = WorldOwner::new();
        let mut hasher = PtrHasher(0);
        owner.hash(&mut hasher);
        hasher.write(&[]);
        assert_ne!(hasher.finish(), 0);
    }
}
