use crate::time::ChangeTick;
use crate::world::{World, WorldError};

impl World {
    pub fn contains_resource<R: 'static>(&self) -> bool {
        self.resources.contains::<R>()
    }

    pub fn insert_resource<R: 'static>(&mut self, value: R) -> Result<Option<R>, WorldError> {
        self.ensure_mutable()?;
        let tick = self.issue_change_tick()?;
        self.resources.insert(value, tick)
    }

    pub fn remove_resource<R: 'static>(&mut self) -> Result<Option<R>, WorldError> {
        self.ensure_mutable()?;
        self.resources.remove::<R>()
    }

    pub fn resource<R: 'static>(&self) -> Result<Option<&R>, WorldError> {
        self.resources.get::<R>()
    }

    pub fn resource_mut<R: 'static>(&mut self) -> Result<Option<&mut R>, WorldError> {
        self.ensure_mutable()?;
        let tick = self.issue_change_tick()?;
        self.resources.get_mut::<R>(tick)
    }

    pub fn resource_added_tick<R: 'static>(&self) -> Result<Option<ChangeTick>, WorldError> {
        self.resources.added_tick::<R>()
    }

    pub fn resource_changed_tick<R: 'static>(&self) -> Result<Option<ChangeTick>, WorldError> {
        self.resources.changed_tick::<R>()
    }

    pub fn resource_scope<R: 'static, T>(
        &mut self,
        f: impl FnOnce(Option<&mut R>, &mut World) -> T,
    ) -> Result<T, WorldError> {
        self.ensure_mutable()?;
        let mut taken = self.resources.begin_scope::<R>()?;
        let tick = self.issue_change_tick()?;
        let result = match taken.as_mut() {
            Some(resource) => f(Some(resource), self),
            None => f(None, self),
        };
        if let Some(resource) = taken {
            self.resources.end_scope::<R>(resource, tick)?;
        } else {
            self.resources.cancel_scope();
        }
        Ok(result)
    }

    #[allow(dead_code)]
    pub(crate) fn lock_resource<R: 'static>(&mut self) {
        self.resources.lock::<R>();
    }

    #[allow(dead_code)]
    pub(crate) fn unlock_resource<R: 'static>(&mut self) {
        self.resources.unlock::<R>();
    }
}