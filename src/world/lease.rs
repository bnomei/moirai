use alloc::rc::Weak;
use core::any::TypeId;

use crate::schedule::ExecutionLease;
use crate::state::State;
use crate::time::ChangeTick;

impl super::World {
    pub(crate) fn attach_execution_lease_with_locks(
        &mut self,
        lease: Weak<()>,
        locked_resources: &[TypeId],
    ) {
        self.execution_lease = Some(lease);
        self.lease_locked_resources = locked_resources.to_vec();
        for type_id in locked_resources {
            self.resources.lock_type(*type_id);
        }
    }

    pub(crate) fn prune_dead_execution_lease(&mut self) {
        if let Some(weak) = &self.execution_lease {
            if !ExecutionLease::is_weak_alive(weak) {
                for type_id in &self.lease_locked_resources {
                    self.resources.unlock_type(*type_id);
                }
                self.lease_locked_resources.clear();
                self.execution_lease = None;
            }
        }
    }

    pub(crate) fn has_live_execution_lease(&self) -> bool {
        self.execution_lease
            .as_ref()
            .is_some_and(ExecutionLease::is_weak_alive)
    }

    pub(crate) fn validate_execution_lease(&self, lease: &ExecutionLease) -> bool {
        self.execution_lease
            .as_ref()
            .is_some_and(|weak| ExecutionLease::same_weak(weak, lease))
    }

    pub fn run_guard_is_idle(&self) -> bool {
        self.run_guard.is_idle()
    }

    pub fn is_mutation_poisoned(&self) -> bool {
        self.mutation_poisoned
    }

    pub(crate) fn resource_present(&self, type_id: TypeId) -> bool {
        self.resources.contains_type(type_id)
    }

    pub(crate) fn resource_type_name(&self, type_id: TypeId) -> Option<&str> {
        self.resources.type_name(type_id)
    }

    pub(crate) fn preflight_world_tick(&self) -> Result<(), crate::time::WorldTickError> {
        let mut tick = self.world_tick;
        tick.advance().map(|_| ())
    }

    pub(crate) fn issue_change_tick_for_state(
        &mut self,
    ) -> Result<ChangeTick, crate::world::WorldError> {
        self.issue_change_tick()
    }

    pub(crate) fn set_fixed_step(&mut self, step: Option<crate::time::FixedStep>) {
        self.fixed_step = step;
    }

    pub fn fixed_step(&self) -> Option<crate::time::FixedStep> {
        self.fixed_step
    }

    pub(crate) fn resource_added_tick_for(&self, type_id: TypeId) -> Option<ChangeTick> {
        self.resources.added_tick_for(type_id).ok().flatten()
    }

    pub(crate) fn resource_changed_tick_for(&self, type_id: TypeId) -> Option<ChangeTick> {
        self.resources.changed_tick_for(type_id).ok().flatten()
    }

    pub(crate) fn state_current<S: Eq + 'static>(&self) -> Result<Option<&S>, ()> {
        self.resources
            .get::<State<S>>()
            .map_err(|_| ())
            .map(|state| state.map(|state| state.current()))
    }

    pub(crate) fn state_transition_tick_for(&self, type_id: TypeId) -> Option<ChangeTick> {
        self.resources.transition_tick_for(type_id).ok().flatten()
    }
}
