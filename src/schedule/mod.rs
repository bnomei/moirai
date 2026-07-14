mod builder;
mod compiled;
mod condition;
mod error;
mod owner;
mod runner;
pub mod stage;
mod system;

pub use builder::ScheduleBuilder;
pub use condition::{Condition, ConditionError};
pub use error::{BuildError, ScheduleError};
pub(crate) use owner::ExecutionLease;
pub(crate) use runner::RunOutcome;
pub use stage::StageId;
pub use system::{FlushMode, System, SystemId, SystemInitContext, SystemSet};

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::any::TypeId;

use crate::operation::StageOperation;
use crate::schedule::compiled::CompiledSchedule;
use crate::time::{ChangeTick, FixedStep};
use crate::world::World;

/// Per-pass execution scratch state for conditions and fixed steps.
pub(crate) struct RunContext {
    pub(crate) fixed_step: Option<FixedStep>,
    resource_added_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    resource_changed_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    state_transition_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    set_resource_added_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    set_resource_changed_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    set_state_transition_cursors: BTreeMap<(usize, TypeId), ChangeTick>,
    set_gate_cache: Vec<Option<bool>>,
}

impl RunContext {
    pub(crate) fn new() -> Self {
        Self::with_set_capacity(0)
    }

    pub(crate) fn with_set_capacity(set_count: usize) -> Self {
        Self {
            fixed_step: None,
            resource_added_cursors: BTreeMap::new(),
            resource_changed_cursors: BTreeMap::new(),
            state_transition_cursors: BTreeMap::new(),
            set_resource_added_cursors: BTreeMap::new(),
            set_resource_changed_cursors: BTreeMap::new(),
            set_state_transition_cursors: BTreeMap::new(),
            set_gate_cache: vec![None; set_count],
        }
    }

    pub(crate) fn clear_set_cache(&mut self) {
        for slot in &mut self.set_gate_cache {
            *slot = None;
        }
    }

    pub(crate) fn set_gate(&mut self, set_index: usize, allowed: bool) {
        if self.set_gate_cache.len() <= set_index {
            self.set_gate_cache.resize(set_index + 1, None);
        }
        self.set_gate_cache[set_index] = Some(allowed);
    }

    pub(crate) fn set_gate_cached(&self, set_index: usize) -> Option<bool> {
        self.set_gate_cache.get(set_index).copied().flatten()
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
        set_index: usize,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.set_resource_added_cursors
            .insert((set_index, type_id), tick);
    }

    pub(crate) fn set_resource_changed_cursor_for_set(
        &mut self,
        set_index: usize,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.set_resource_changed_cursors
            .insert((set_index, type_id), tick);
    }

    pub(crate) fn set_state_transition_cursor_for_set(
        &mut self,
        set_index: usize,
        type_id: TypeId,
        tick: ChangeTick,
    ) {
        self.set_state_transition_cursors
            .insert((set_index, type_id), tick);
    }

    pub(crate) fn resource_added_cursor_for_set(
        &self,
        set_index: usize,
        type_id: TypeId,
    ) -> ChangeTick {
        self.set_resource_added_cursors
            .get(&(set_index, type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn resource_changed_cursor_for_set(
        &self,
        set_index: usize,
        type_id: TypeId,
    ) -> ChangeTick {
        self.set_resource_changed_cursors
            .get(&(set_index, type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
    }

    pub(crate) fn state_transition_cursor_for_set(
        &self,
        set_index: usize,
        type_id: TypeId,
    ) -> ChangeTick {
        self.set_state_transition_cursors
            .get(&(set_index, type_id))
            .copied()
            .unwrap_or(ChangeTick::ZERO)
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

/// Validated subset of Update stages run by [`crate::App::update_plan`].
///
/// The plan is schedule-owned through its opaque stage handles. Execution always
/// follows the compiled stage order, never the caller's input order.
#[derive(Clone, Debug)]
pub struct UpdatePlan {
    stages: Vec<usize>,
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

    /// Resolve a compiled stage label to an opaque handle owned by this schedule.
    pub fn stage_id(&self, label: &str) -> Option<StageId> {
        self.compiled
            .stages
            .iter()
            .position(|stage| stage.descriptor.label == label)
            .and_then(|index| u32::try_from(index).ok())
            .map(|index| StageId::new(self.compiled.owner.clone(), index))
    }

    /// Resolve an opaque stage handle back to its label after owner and bounds checks.
    pub fn stage_label(&self, id: &StageId) -> Result<&str, ScheduleError> {
        id.validate_owner(&self.compiled.owner)?;
        self.compiled
            .stages
            .get(id.index())
            .map(|stage| stage.descriptor.label.as_str())
            .ok_or(ScheduleError::StaleHandle)
    }

    pub fn set_system_enabled(
        &mut self,
        id: &SystemId,
        enabled: bool,
    ) -> Result<(), ScheduleError> {
        self.compiled.set_system_enabled(id, enabled)
    }

    /// Creates a plan that runs only the supplied Update stages.
    ///
    /// Startup is automatic on the first successful update and cannot be named
    /// in a plan. Render stages belong to [`crate::App::render`].
    pub fn update_plan(
        &self,
        stages: impl IntoIterator<Item = StageId>,
    ) -> Result<UpdatePlan, ScheduleError> {
        let mut selected = Vec::new();
        for id in stages {
            id.validate_owner(&self.compiled.owner)?;
            let index = id.index();
            let stage = self
                .compiled
                .stages
                .get(index)
                .ok_or(ScheduleError::StaleHandle)?;
            if stage.descriptor.operation != StageOperation::Update {
                return Err(ScheduleError::NonUpdateStageInPlan);
            }
            if stage.descriptor.label == stage::STARTUP {
                return Err(ScheduleError::StartupStageInPlan);
            }
            if selected.contains(&index) {
                return Err(ScheduleError::DuplicateStageInPlan);
            }
            selected.push(index);
        }
        Ok(UpdatePlan { stages: selected })
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

    #[cfg(test)]
    #[allow(dead_code)]
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

    pub(crate) fn update_stage_indices(&self) -> &[usize] {
        &self.compiled.update_stage_order
    }

    pub(crate) fn plan_contains_stage(&self, plan: &UpdatePlan, stage_index: usize) -> bool {
        plan.stages.contains(&stage_index)
    }

    pub(crate) fn set_count(&self) -> usize {
        self.compiled.set_conditions.len()
    }

    pub(crate) fn stage_label_at(&self, stage_index: usize) -> &str {
        self.compiled.stage_label(stage_index)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn stage_flush_mode_for_test(&self, label: &str) -> Option<FlushMode> {
        self.stage_index(label)
            .map(|index| self.compiled.stage_flush_mode(index))
    }
}

#[cfg(test)]
pub(crate) fn stage_flush_mode_for_test(schedule: &Schedule, label: &str) -> Option<FlushMode> {
    schedule
        .stage_id(label)
        .map(|id| schedule.compiled.stage_flush_mode(id.index()))
}

#[cfg(test)]
mod default_tests {
    use super::{stage_flush_mode_for_test, RunContext, Schedule};
    use crate::schedule::FlushMode;
    use crate::world::WorldBuilder;

    #[test]
    fn defaults_construct() {
        assert_eq!(RunContext::default().set_gate_cache.len(), 0);
        let _builder = Schedule::standard_builder();
    }

    #[test]
    fn schedule_builder_entry_point_constructs() {
        let _builder = Schedule::builder();
    }

    #[test]
    fn set_gate_resizes_cache_for_out_of_range_indices() {
        let mut context = RunContext::with_set_capacity(0);
        context.set_gate(3, true);
        assert_eq!(context.set_gate_cache.len(), 4);
        assert_eq!(context.set_gate_cached(3), Some(true));
    }

    #[test]
    fn schedule_test_accessors_resolve_standard_stages() {
        let mut world = WorldBuilder::new().build().expect("world");
        let schedule = Schedule::standard_builder()
            .build(&mut world)
            .expect("schedule");
        assert!(schedule
            .stage_index(crate::schedule::stage::UPDATE)
            .is_some());
        assert_eq!(
            schedule.stage_flush_mode_for_test(crate::schedule::stage::UPDATE),
            Some(FlushMode::Stage)
        );
        assert_eq!(
            stage_flush_mode_for_test(&schedule, crate::schedule::stage::RENDER),
            Some(FlushMode::Final)
        );
    }
}
