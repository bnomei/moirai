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
