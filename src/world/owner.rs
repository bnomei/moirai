use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU32, Ordering};

static NEXT_OWNER_TOKEN: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub(crate) struct WorldOwner(u32);

impl WorldOwner {
    pub fn new() -> Self {
        let token = NEXT_OWNER_TOKEN
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("world owner token exhausted");
        Self(token)
    }

    pub fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    pub(crate) fn token(&self) -> u32 {
        self.0
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
        self.0.hash(state);
    }
}

impl core::fmt::Debug for WorldOwner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("WorldOwner").field(&self.0).finish()
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

    struct TokenHasher(u64);

    impl Hasher for TokenHasher {
        fn finish(&self) -> u64 {
            self.0
        }

        fn write(&mut self, _: &[u8]) {}

        fn write_u32(&mut self, i: u32) {
            self.0 = i as u64;
        }
    }

    #[test]
    fn same_owner_is_equal_and_hashes_consistently() {
        let a = WorldOwner::new();
        let b = a.clone();
        assert_eq!(a, b);
        let mut hasher_a = TokenHasher(0);
        let mut hasher_b = TokenHasher(0);
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
    fn debug_formats_as_owner_tuple() {
        let owner = WorldOwner::new();
        let text = alloc::format!("{owner:?}");
        assert!(text.starts_with("WorldOwner("));
        assert!(text.ends_with(')'));
    }

    #[test]
    fn token_hasher_records_u32_writes() {
        let owner = WorldOwner::new();
        let mut hasher = TokenHasher(0);
        owner.hash(&mut hasher);
        hasher.write(&[]);
        assert_ne!(hasher.finish(), 0);
    }
}
