use crate::time::ChangeTick;
use crate::world::{World, WorldError};

struct ResourceScopeGuard<'world, R: 'static> {
    world: &'world mut World,
    resource: Option<R>,
    tick: ChangeTick,
    active: bool,
}

impl<'world, R: 'static> ResourceScopeGuard<'world, R> {
    fn new(world: &'world mut World, resource: Option<R>, tick: ChangeTick) -> Self {
        Self {
            world,
            resource,
            tick,
            active: true,
        }
    }

    fn call<T>(&mut self, f: impl FnOnce(Option<&mut R>, &mut World) -> T) -> T {
        f(self.resource.as_mut(), self.world)
    }

    fn restore(&mut self) -> Result<(), WorldError> {
        if !self.active {
            return Ok(());
        }
        self.active = false;

        let result = if let Some(resource) = self.resource.take() {
            self.world.resources.restore_scope::<R>(resource, self.tick)
        } else {
            self.world.resources.cancel_scope();
            Ok(())
        };
        if result.is_err() {
            self.world.resources.cancel_scope();
        }
        result
    }
}

impl<R: 'static> Drop for ResourceScopeGuard<'_, R> {
    fn drop(&mut self) {
        // `restore` can only fail if ResourceStore's private scope invariant is
        // broken. The public callback cannot alter registration or the scope
        // sentinel, so unwinding remains non-panicking in normal operation.
        let _ = self.restore();
    }
}

impl World {
    pub fn contains_resource<R: 'static>(&self) -> bool {
        self.resources.contains::<R>()
    }

    pub fn insert_resource<R: 'static>(&mut self, value: R) -> Result<Option<R>, WorldError> {
        self.ensure_mutable()?;
        self.resources.prepare_insert::<R>()?;
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
        if !self.resources.prepare_mut::<R>()? {
            return Ok(None);
        }
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
        self.resources.prepare_scope::<R>()?;
        let tick = self.issue_change_tick()?;
        let taken = self.resources.take_for_scope::<R>()?;
        let mut guard = ResourceScopeGuard::new(self, taken, tick);
        let result = guard.call(f);
        guard.restore()?;
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
