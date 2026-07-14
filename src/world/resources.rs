//! World-scoped resource and state storage.
//!
//! Resource inserts and mutable borrows issue change ticks. [`World::resource_scope_mut`]
//! and [`World::resource_scope_ref`] temporarily remove a resource while running a
//! closure against the rest of the world.

use crate::resource::ScopedResource;
use crate::time::ChangeTick;
use crate::world::{World, WorldError};

struct ResourceScopeGuard<'world, R: 'static> {
    world: &'world mut World,
    resource: Option<ScopedResource<R>>,
    changed: Option<ChangeTick>,
    active: bool,
}

impl<'world, R: 'static> ResourceScopeGuard<'world, R> {
    fn new(
        world: &'world mut World,
        resource: Option<ScopedResource<R>>,
        changed: Option<ChangeTick>,
    ) -> Self {
        Self {
            world,
            resource,
            changed,
            active: true,
        }
    }

    fn call_mut<T>(&mut self, f: impl FnOnce(Option<&mut R>, &mut World) -> T) -> T {
        f(
            self.resource.as_mut().map(ScopedResource::get_mut),
            self.world,
        )
    }

    fn call_ref<T>(&mut self, f: impl FnOnce(Option<&R>, &mut World) -> T) -> T {
        f(self.resource.as_ref().map(ScopedResource::get), self.world)
    }

    fn restore(&mut self) -> Result<(), WorldError> {
        if !self.active {
            return Ok(());
        }
        self.active = false;

        let result = if let Some(resource) = self.resource.take() {
            self.world
                .resources
                .restore_scope::<R>(resource, self.changed)
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
    /// Whether resource `R` is registered and currently stored.
    pub fn contains_resource<R: 'static>(&self) -> bool {
        self.resources.contains::<R>()
    }

    /// Insert or replace resource `R`, returning any previous value.
    pub fn insert_resource<R: 'static>(&mut self, value: R) -> Result<Option<R>, WorldError> {
        self.ensure_mutable()?;
        self.resources.prepare_insert::<R>()?;
        let tick = self.issue_change_tick()?;
        self.resources.insert(value, tick)
    }

    /// Remove resource `R` when it is not locked or scoped.
    pub fn remove_resource<R: 'static>(&mut self) -> Result<Option<R>, WorldError> {
        self.ensure_mutable()?;
        self.resources.remove::<R>()
    }

    /// Immutable access to resource `R`.
    pub fn resource<R: 'static>(&self) -> Result<Option<&R>, WorldError> {
        self.resources.get::<R>()
    }

    /// Mutable access to resource `R`, issuing a change tick when present.
    pub fn resource_mut<R: 'static>(&mut self) -> Result<Option<&mut R>, WorldError> {
        self.ensure_mutable()?;
        if !self.resources.prepare_mut::<R>()? {
            return Ok(None);
        }
        let tick = self.issue_change_tick()?;
        self.resources.get_mut::<R>(tick)
    }

    /// Change tick recorded when resource `R` was last inserted.
    pub fn resource_added_tick<R: 'static>(&self) -> Result<Option<ChangeTick>, WorldError> {
        self.resources.added_tick::<R>()
    }

    /// Change tick recorded when resource `R` was last mutated.
    pub fn resource_changed_tick<R: 'static>(&self) -> Result<Option<ChangeTick>, WorldError> {
        self.resources.changed_tick::<R>()
    }

    /// Temporarily remove resource `R`, run `f` with mutable access, then restore it.
    pub fn resource_scope_mut<R: 'static, T>(
        &mut self,
        f: impl FnOnce(Option<&mut R>, &mut World) -> T,
    ) -> Result<T, WorldError> {
        self.ensure_mutable()?;
        let present = self.resources.prepare_scope::<R>()?;
        let changed = if present {
            Some(self.issue_change_tick()?)
        } else {
            None
        };
        let taken = self.resources.take_for_scope::<R>()?;
        let mut guard = ResourceScopeGuard::new(self, taken, changed);
        let result = guard.call_mut(f);
        guard.restore()?;
        Ok(result)
    }

    /// Temporarily remove resource `R`, run `f` with shared access, then restore it.
    pub fn resource_scope_ref<R: 'static, T>(
        &mut self,
        f: impl FnOnce(Option<&R>, &mut World) -> T,
    ) -> Result<T, WorldError> {
        self.ensure_mutable()?;
        self.resources.prepare_scope::<R>()?;
        let taken = self.resources.take_for_scope::<R>()?;
        let mut guard = ResourceScopeGuard::new(self, taken, None);
        let result = guard.call_ref(f);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WorldBuilder;

    struct Primary;
    struct Other;

    #[test]
    fn restore_failure_cancels_the_scope_sentinel() {
        let mut builder = WorldBuilder::new();
        builder.insert_resource(Primary);
        builder.insert_resource(Other);
        let mut world = builder.build().expect("world");
        let primary = world
            .resources
            .take_for_scope::<Primary>()
            .expect("primary scope");
        let mut guard = ResourceScopeGuard::new(&mut world, primary, None);

        guard.world.resources.cancel_scope();
        let other = guard
            .world
            .resources
            .take_for_scope::<Other>()
            .expect("other scope");
        assert!(guard.restore().is_err());
        assert!(!guard
            .world
            .resources
            .prepare_scope::<Other>()
            .expect("scope sentinel cleared"));
        drop(other);
    }
}
