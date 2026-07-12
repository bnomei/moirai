use crate::operation::StageOperation;

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunGuard {
    Idle,
    Running(StageOperation),
}

impl RunGuard {
    pub fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}
