use crate::world::{FlushReport, World, WorldError};

impl World {
    pub fn has_pending_commands(&self) -> bool {
        !self.command_queue.is_empty()
    }

    pub fn flush(&mut self) -> Result<FlushReport, WorldError> {
        if !self.run_guard.is_idle() {
            return Err(WorldError::FlushDuringRun);
        }
        self.flush_commands()
    }

    pub fn discard_commands(&mut self) -> Result<(), WorldError> {
        if !self.run_guard.is_idle() {
            return Err(WorldError::DiscardDuringRun);
        }
        self.ensure_mutable()?;
        self.command_queue.discard(&mut self.allocator)
    }

    pub(crate) fn flush_commands(&mut self) -> Result<FlushReport, WorldError> {
        if self.command_queue.is_empty() {
            return Ok(FlushReport {
                commands_applied: 0,
                change_tick: self.change_tick,
            });
        }
        self.ensure_mutable()?;
        if let Err(error) = self.command_queue.preflight(self) {
            self.command_queue.discard(&mut self.allocator)?;
            return Err(WorldError::from(error));
        }
        let tick = match self.issue_change_tick() {
            Ok(tick) => tick,
            Err(error) => {
                self.command_queue.discard(&mut self.allocator)?;
                return Err(error);
            }
        };
        let applied = match self.commit_command_ops(tick) {
            Ok(applied) => applied,
            Err(error) => {
                self.command_queue.discard(&mut self.allocator)?;
                return Err(error);
            }
        };
        Ok(FlushReport {
            commands_applied: applied,
            change_tick: tick,
        })
    }
}