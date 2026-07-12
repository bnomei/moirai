use alloc::string::String;

use crate::operation::StageOperation;
use crate::schedule::owner::ScheduleOwner;

/// Built-in stage label for one-shot initialization systems.
pub const STARTUP: &str = "Startup";
/// Built-in stage label for fixed-timestep simulation systems.
pub const FIXED_UPDATE: &str = "FixedUpdate";
/// Built-in stage label for variable-timestep simulation systems.
pub const UPDATE: &str = "Update";
/// Built-in stage label for presentation systems.
pub const RENDER: &str = "Render";

/// Opaque compiled stage handle.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StageId {
    owner: ScheduleOwner,
    index: u32,
}

impl StageId {
    #[allow(dead_code)]
    pub(crate) fn new(owner: ScheduleOwner, index: u32) -> Self {
        Self { owner, index }
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    #[allow(dead_code)]
    pub(crate) fn validate_owner(
        &self,
        owner: &ScheduleOwner,
    ) -> Result<(), crate::schedule::ScheduleError> {
        if self.owner.same(owner) {
            Ok(())
        } else {
            Err(crate::schedule::ScheduleError::OwnerMismatch)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StageDescriptor {
    pub label: String,
    pub operation: StageOperation,
    pub flush_mode: crate::schedule::FlushMode,
}
