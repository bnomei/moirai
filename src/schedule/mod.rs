mod builder;
mod compiled;
mod condition;
mod error;
mod owner;
mod runner;
pub mod stage;
mod system;

pub use builder::ScheduleBuilder;
pub use condition::Condition;
pub use error::{BuildError, ScheduleError};
pub(crate) use owner::ExecutionLease;
pub(crate) use runner::RunOutcome;
pub use stage::StageId;
pub use system::{FlushMode, System, SystemId, SystemSet};

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::any::TypeId;

use crate::operation::StageOperation;
use crate::schedule::compiled::CompiledSchedule;
use crate::time::{ChangeTick, FixedStep};
use crate::world::World;

/// Per-pass execution scratch state for conditions and fixed steps.
pub struct RunContext {
    pub fixed_step: Option<FixedStep>,
    resource_added_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    resource_changed_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    state_transition_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    set_resource_added_cursors: BTreeMap<(alloc::string::String, TypeId), ChangeTick>,
    set_resource_changed_cursors: BTreeMap<(alloc::string::String, TypeId), ChangeTick>,
    set_state_transition_cursors: BTreeMap<(alloc::string::String, TypeId), ChangeTick>,
    set_gate_cache: BTreeMap<alloc::string::String, bool>,
}

impl RunContext {
    pub fn new() -> Self {
        Self {
            fixed_step: None,
            resource_added_cursors: BTreeMap::new(),
            resource_changed_cursors: BTreeMap::new(),
            state_transition_cursors: BTreeMap::new(),
            set_resource_added_cursors: BTreeMap::new(),
            set_resource_changed_cursors: BTreeMap::new(),
            set_state_transition_cursors: BTreeMap::new(),
            set_gate_cache: BTreeMap::new(),
        }
    }

    pub(crate) fn clear_set_cache(&mut self) {
        self.set_gate_cache.clear();
    }

    pub(crate) fn set_gate(&mut self, label: &str, allowed: bool) {
        self.set_gate_cache.insert(label.into(), allowed);
    }

    pub(crate) fn set_gate_cached(&self, label: &str) -> Option<bool> {
        self.set_gate_cache.get(label).copied()
    }

    pub(crate) fn resource_added_cursor(&self, system_index: usize, type_id: TypeId) -> ChangeTick {
        self.resource_added_cursors
            .get(&(system_index, type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn resource_changed_cursor(
        &self,
        system_index: usize,
        type_id: TypeId,
    ) -> ChangeTick {
        self.resource_changed_cursors
            .get(&(system_index, type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn state_transition_cursor(
        &self,
        system_index: usize,
        type_id: TypeId,
    ) -> ChangeTick {
        self.state_transition_cursors
            .get(&(system_index, type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn set_resource_added_cursor(
        &mut self,
        system_index: usize,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.resource_added_cursors
            .insert((system_index, type_id), tick);
    }

    pub(crate) fn set_resource_changed_cursor(
        &mut self,
        system_index: usize,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.resource_changed_cursors
            .insert((system_index, type_id), tick);
    }

    pub(crate) fn set_state_transition_cursor(
        &mut self,
        system_index: usize,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.state_transition_cursors
            .insert((system_index, type_id), tick);
    }

    pub(crate) fn set_resource_added_cursor_for_set(
        &mut self,
        set_label: &str,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.set_resource_added_cursors
            .insert((set_label.into(), type_id), tick);
    }

    pub(crate) fn set_resource_changed_cursor_for_set(
        &mut self,
        set_label: &str,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.set_resource_changed_cursors
            .insert((set_label.into(), type_id), tick);
    }

    pub(crate) fn set_state_transition_cursor_for_set(
        &mut self,
        set_label: &str,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.set_state_transition_cursors
            .insert((set_label.into(), type_id), tick);
    }

    pub(crate) fn resource_added_cursor_for_set(
        &self,
        set_label: &str,
        type_id: TypeId,
    ) -> ChangeTick {
        self.set_resource_added_cursors
            .get(&(set_label.into(), type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn resource_changed_cursor_for_set(
        &self,
        set_label: &str,
        type_id: TypeId,
    ) -> ChangeTick {
        self.set_resource_changed_cursors
            .get(&(set_label.into(), type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn state_transition_cursor_for_set(
        &self,
        set_label: &str,
        type_id: TypeId,
    ) -> ChangeTick {
        self.set_state_transition_cursors
            .get(&(set_label.into(), type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn evaluated_set_labels(&self) -> impl Iterator<Item = &str> {
        self.set_gate_cache
            .keys()
            .map(alloc::string::String::as_str)
    }
}

impl Default for RunContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Validated compiled schedule executed only through `App`.
pub struct Schedule {
    pub(crate) compiled: CompiledSchedule,
}

impl Schedule {
    pub fn builder() -> ScheduleBuilder {
        ScheduleBuilder::new()
    }

    pub fn standard_builder() -> ScheduleBuilder {
        ScheduleBuilder::standard()
    }

    pub(crate) fn execution_lease(&self) -> &ExecutionLease {
        &self.compiled.lease
    }

    pub fn system_id(&self, name: &str) -> Option<SystemId> {
        self.compiled
            .systems
            .iter()
            .find(|system| system.name == name)
            .map(|system| system.id.clone())
    }

    pub fn set_system_enabled(
        &mut self,
        id: &SystemId,
        enabled: bool,
    ) -> Result<(), ScheduleError> {
        self.compiled.set_system_enabled(id, enabled)
    }

    pub(crate) fn run_stage(
        &mut self,
        world: &mut World,
        stage_index: usize,
        context: &mut RunContext,
        dt: f32,
        observer: &mut Option<alloc::boxed::Box<dyn crate::diagnostics::Observer>>,
    ) -> Result<(), runner::RunOutcome> {
        runner::run_stage(
            &mut self.compiled,
            world,
            stage_index,
            context,
            dt,
            observer,
        )
    }

    pub(crate) fn run_operation(
        &mut self,
        world: &mut World,
        operation: StageOperation,
        context: &mut RunContext,
        dt: f32,
        observer: &mut Option<alloc::boxed::Box<dyn crate::diagnostics::Observer>>,
    ) -> Result<(), runner::RunOutcome> {
        runner::run_operation(&mut self.compiled, world, operation, context, dt, observer)
    }

    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn stage_index(&self, label: &str) -> Option<usize> {
        self.compiled
            .stages
            .iter()
            .position(|stage| stage.descriptor.label == label)
    }

    pub(crate) fn run_final_update_flush(
        &mut self,
        world: &mut World,
        observer: &mut Option<alloc::boxed::Box<dyn crate::diagnostics::Observer>>,
    ) -> Result<(), runner::RunOutcome> {
        runner::final_update_flush(world, observer)
    }

    pub(crate) fn clear_frame_events(&mut self, world: &mut World, operation: StageOperation) {
        world.clear_frame_events(operation);
    }

    pub(crate) fn fixed_config(&self) -> Option<&crate::time::FixedConfig> {
        self.compiled.fixed_config.as_ref()
    }

    pub(crate) fn fixed_accumulator(&self) -> &crate::time::FixedAccumulator {
        &self.compiled.fixed_accumulator
    }

    pub(crate) fn fixed_accumulator_mut(&mut self) -> &mut crate::time::FixedAccumulator {
        &mut self.compiled.fixed_accumulator
    }

    pub(crate) fn update_stage_indices(&self) -> Vec<usize> {
        self.compiled.update_stage_order.clone()
    }

    pub(crate) fn stage_label(&self, stage_index: usize) -> &str {
        self.compiled.stage_label(stage_index)
    }

    #[cfg(any(test, feature = "testkit"))]
    pub fn stage_flush_mode_for_test(&self, label: &str) -> Option<FlushMode> {
        self.stage_index(label)
            .map(|index| self.compiled.stage_flush_mode(index))
    }
}
