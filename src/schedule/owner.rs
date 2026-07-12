use alloc::rc::Rc;
use core::hash::{Hash, Hasher};

/// Opaque schedule identity shared by runtime handles.
#[derive(Clone)]
pub(crate) struct ScheduleOwner(Rc<()>);

impl ScheduleOwner {
    pub fn new() -> Self {
        Self(Rc::new(()))
    }

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
    pub fn new() -> Self {
        Self(Rc::new(()))
    }

    pub fn downgrade(&self) -> alloc::rc::Weak<()> {
        Rc::downgrade(&self.0)
    }

    pub fn same_weak(weak: &alloc::rc::Weak<()>, lease: &Self) -> bool {
        weak.ptr_eq(&Rc::downgrade(&lease.0))
    }

    pub fn is_weak_alive(weak: &alloc::rc::Weak<()>) -> bool {
        weak.strong_count() > 0
    }
}
