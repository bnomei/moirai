use alloc::collections::BTreeMap;
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
    pub in_set: Option<String>,
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
    pub set_conditions: BTreeMap<String, Condition>,
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
