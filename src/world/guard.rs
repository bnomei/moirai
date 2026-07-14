//! Run guard and per-system event access for schedule execution.
//!
//! [`RunGuard`] tracks whether the world is idle or running a stage operation and,
//! when present, which event channels the active system may emit or consume.

use crate::event::EventId;
use crate::operation::StageOperation;
use alloc::rc::Rc;
use alloc::vec::Vec;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EventAccess {
    emitted: Vec<EventId>,
    consumed: Vec<EventId>,
}

impl EventAccess {
    pub fn new(emitted: Vec<EventId>, consumed: Vec<EventId>) -> Self {
        Self { emitted, consumed }
    }

    pub fn can_emit(&self, event_id: &EventId) -> bool {
        self.emitted.contains(event_id)
    }

    pub fn can_consume(&self, event_id: &EventId) -> bool {
        self.consumed.contains(event_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunGuard {
    Idle,
    Running {
        operation: StageOperation,
        event_access: Option<Rc<EventAccess>>,
    },
}

impl RunGuard {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn operation(&self) -> Option<StageOperation> {
        match self {
            Self::Idle => None,
            Self::Running { operation, .. } => Some(*operation),
        }
    }

    pub fn permits_emit(&self, event_id: &EventId) -> bool {
        match self {
            Self::Idle
            | Self::Running {
                event_access: None, ..
            } => true,
            Self::Running {
                event_access: Some(access),
                ..
            } => access.can_emit(event_id),
        }
    }

    pub fn permits_consume(&self, event_id: &EventId) -> bool {
        match self {
            Self::Idle
            | Self::Running {
                event_access: None, ..
            } => true,
            Self::Running {
                event_access: Some(access),
                ..
            } => access.can_consume(event_id),
        }
    }
}
