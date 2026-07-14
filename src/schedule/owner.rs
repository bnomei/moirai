//! Opaque schedule identity and the live execution lease shared with the world.

use alloc::rc::Rc;
use core::hash::{Hash, Hasher};

/// Opaque schedule identity shared by runtime handles.
#[derive(Clone)]
pub(crate) struct ScheduleOwner(Rc<()>);

impl ScheduleOwner {
    /// Allocates a fresh schedule identity for handles and lease pairing.
    pub fn new() -> Self {
        Self(Rc::new(()))
    }

    /// Whether two handles were issued by the same compiled schedule.
    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for ScheduleOwner {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}

impl Eq for ScheduleOwner {}

impl Hash for ScheduleOwner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

impl core::fmt::Debug for ScheduleOwner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ScheduleOwner")
            .field(&Rc::as_ptr(&self.0))
            .finish()
    }
}

impl Default for ScheduleOwner {
    fn default() -> Self {
        Self::new()
    }
}

/// Live compiled-schedule token owned by `Schedule` and weakly recorded on `World`.
#[derive(Clone)]
pub struct ExecutionLease(Rc<()>);

impl ExecutionLease {
    /// Creates the strong lease held by [`crate::schedule::Schedule`] for its lifetime.
    pub fn new() -> Self {
        Self(Rc::new(()))
    }

    /// Weak token stored on the world while this schedule remains attached.
    pub fn downgrade(&self) -> alloc::rc::Weak<()> {
        Rc::downgrade(&self.0)
    }

    /// Whether a world's weak lease pointer matches this schedule's lease.
    pub fn same_weak(weak: &alloc::rc::Weak<()>, lease: &Self) -> bool {
        weak.ptr_eq(&Rc::downgrade(&lease.0))
    }

    /// Whether the compiled schedule that issued the weak lease is still alive.
    pub fn is_weak_alive(weak: &alloc::rc::Weak<()>) -> bool {
        weak.strong_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use core::hash::{Hash, Hasher};

    #[test]
    fn schedule_owner_traits_and_lease_helpers() {
        let a = ScheduleOwner::new();
        let b = ScheduleOwner::new();
        assert!(a.same(&a));
        assert!(!a.same(&b));
        assert_eq!(a, a);
        let lease = ExecutionLease::new();
        let weak = lease.downgrade();
        assert!(ExecutionLease::same_weak(&weak, &lease));
        assert!(ExecutionLease::is_weak_alive(&weak));
        assert!(format!("{:?}", ScheduleOwner::default()).contains("ScheduleOwner"));

        struct Capture(u64);
        impl Hasher for Capture {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, _: &[u8]) {}
            fn write_u64(&mut self, i: u64) {
                self.0 = i;
            }
            fn write_u128(&mut self, i: u128) {
                self.0 = i as u64;
            }
            fn write_usize(&mut self, i: usize) {
                self.0 = i as u64;
            }
        }
        let mut hasher = Capture(0);
        a.hash(&mut hasher);
        hasher.write(&[]);
        hasher.write_u64(7);
        hasher.write_u128(9);
        assert_ne!(hasher.finish(), 0);
    }
}
