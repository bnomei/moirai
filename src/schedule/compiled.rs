use alloc::string::String;
use alloc::vec::Vec;

use crate::operation::StageOperation;
use crate::schedule::condition::Condition;
use crate::schedule::owner::{ExecutionLease, ScheduleOwner};
use crate::schedule::stage::StageDescriptor;
use crate::schedule::system::{FlushMode, SystemBody, SystemId};
use crate::time::{FixedAccumulator, FixedConfig};

pub(crate) struct CompiledStage {
    pub descriptor: StageDescriptor,
    pub system_order: Vec<usize>,
}

pub(crate) struct CompiledSystem {
    pub name: String,
    #[allow(dead_code)]
    pub stage_index: usize,
    pub body: SystemBody,
    pub enabled: bool,
    pub flush_mode: FlushMode,
    pub conditions: Vec<Condition>,
    pub in_set_index: Option<usize>,
    pub id: SystemId,
}

pub(crate) struct CompiledSchedule {
    pub owner: ScheduleOwner,
    pub lease: ExecutionLease,
    pub generation: u32,
    pub stages: Vec<CompiledStage>,
    pub systems: Vec<CompiledSystem>,
    pub update_stage_order: Vec<usize>,
    pub render_stage_order: Vec<usize>,
    pub fixed_config: Option<FixedConfig>,
    pub fixed_accumulator: FixedAccumulator,
    pub startup_complete: bool,
    pub system_enabled: Vec<bool>,
    pub set_conditions: Vec<Condition>,
}

impl CompiledSchedule {
    pub fn operation_stages(&self, operation: StageOperation) -> &[usize] {
        match operation {
            StageOperation::Update => &self.update_stage_order,
            StageOperation::Render => &self.render_stage_order,
        }
    }

    pub fn stage_label(&self, stage_index: usize) -> &str {
        &self.stages[stage_index].descriptor.label
    }

    pub fn stage_operation(&self, stage_index: usize) -> StageOperation {
        self.stages[stage_index].descriptor.operation
    }

    pub fn stage_flush_mode(&self, stage_index: usize) -> FlushMode {
        self.stages[stage_index].descriptor.flush_mode
    }

    #[allow(dead_code)]
    pub fn system_name(&self, system_index: usize) -> &str {
        &self.systems[system_index].name
    }

    pub fn set_system_enabled(
        &mut self,
        id: &SystemId,
        enabled: bool,
    ) -> Result<(), crate::schedule::ScheduleError> {
        id.validate_owner(&self.owner, self.generation)?;
        let index = id.index();
        if index >= self.system_enabled.len() {
            return Err(crate::schedule::ScheduleError::StaleHandle);
        }
        self.system_enabled[index] = enabled;
        self.systems[index].enabled = enabled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::stage;
    use alloc::boxed::Box;
    use alloc::vec;

    fn compiled_with_update_stage() -> CompiledSchedule {
        CompiledSchedule {
            owner: ScheduleOwner::new(),
            lease: ExecutionLease::new(),
            generation: 1,
            stages: vec![CompiledStage {
                descriptor: StageDescriptor {
                    label: String::from(stage::UPDATE),
                    operation: StageOperation::Update,
                    flush_mode: FlushMode::Final,
                },
                system_order: Vec::new(),
            }],
            systems: Vec::new(),
            update_stage_order: vec![0],
            render_stage_order: Vec::new(),
            fixed_config: None,
            fixed_accumulator: FixedAccumulator::new(),
            startup_complete: false,
            system_enabled: Vec::new(),
            set_conditions: Vec::new(),
        }
    }

    #[test]
    fn system_name_returns_registered_label() {
        let mut schedule = compiled_with_update_stage();
        schedule.systems.push(CompiledSystem {
            name: String::from("work"),
            stage_index: 0,
            body: Box::new(|_world, _dt| Ok(())),
            enabled: true,
            flush_mode: FlushMode::Final,
            in_set_index: None,
            conditions: Vec::new(),
            id: SystemId::new(schedule.owner.clone(), 0, schedule.generation),
        });
        schedule.system_enabled.push(true);
        assert_eq!(schedule.system_name(0), "work");
    }

    #[test]
    fn operation_stages_returns_update_order() {
        let compiled = compiled_with_update_stage();
        assert_eq!(compiled.operation_stages(StageOperation::Update), &[0]);
    }

    #[test]
    fn set_system_enabled_rejects_stale_handle_index() {
        let mut compiled = compiled_with_update_stage();
        let id = SystemId::new(compiled.owner.clone(), 9, compiled.generation);
        assert!(matches!(
            compiled.set_system_enabled(&id, false),
            Err(crate::schedule::ScheduleError::StaleHandle)
        ));
    }
}
