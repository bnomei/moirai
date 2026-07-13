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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::operation::StageOperation;
    #[cfg(feature = "testkit")]
    use crate::time::ChangeTick;
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Health(#[allow(dead_code)] i32);

    #[test]
    fn flush_and_discard_reject_during_run() {
        let mut world = WorldBuilder::new().build().expect("world");
        world.begin_run(StageOperation::Update).expect("begin");
        assert!(matches!(world.flush(), Err(WorldError::FlushDuringRun)));
        assert!(matches!(
            world.discard_commands(),
            Err(WorldError::DiscardDuringRun)
        ));
        world.end_run();
    }

    #[test]
    #[cfg(feature = "testkit")]
    fn flush_discards_commands_when_change_tick_exhausted() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("world");
        let _ = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));
        assert!(matches!(
            world.flush(),
            Err(WorldError::ChangeTickExhausted)
        ));
        assert!(!world.has_pending_commands());
    }

    #[test]
    fn flush_discards_commands_when_commit_fails() {
        let mut world = WorldBuilder::new().build().expect("world");
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world
            .allocator_mut()
            .set_generation_for_test(reserved, u32::MAX);
        assert!(matches!(world.flush(), Err(WorldError::StaleEntity { .. })));
    }

    #[test]
    fn flush_discards_commands_when_commit_emit_fails() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world
            .commands()
            .expect("commands")
            .insert(entity, Health(1))
            .expect("queue");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.flush(),
            Err(WorldError::UnregisteredEvent { .. })
        ));
        assert!(!world.has_pending_commands());
    }
}
