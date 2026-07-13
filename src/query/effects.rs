use alloc::string::String;
use core::any::type_name;

use crate::command::{CommandOp, CommandQueue};
use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::operation::StageOperation;
use crate::query::QueryError;
use crate::world::guard::RunGuard;
use crate::world::{Bundle, BundleWriter, WorldError, WorldEvents, WorldOwner};

/// Restricted command surface available during query traversal.
pub struct QueryCommands<'w> {
    allocator: &'w mut EntityAllocator,
    queue: &'w mut CommandQueue,
}

impl<'w> QueryCommands<'w> {
    pub fn spawn(&mut self) -> Result<EntityId, QueryError> {
        let entity = self
            .allocator
            .reserve()
            .map_err(map_allocator_error_query)?;
        self.queue.push(CommandOp::SpawnReserved { entity });
        Ok(entity)
    }

    pub fn despawn(&mut self, entity: EntityId) -> Result<(), QueryError> {
        self.ensure_target(entity)?;
        self.queue.push(CommandOp::Despawn { entity });
        Ok(())
    }

    pub fn insert<T: 'static>(&mut self, entity: EntityId, value: T) -> Result<(), QueryError> {
        self.ensure_target(entity)?;
        self.queue
            .enqueue_insert(entity, value)
            .map_err(map_command_error)
    }

    pub fn remove<T: 'static>(&mut self, entity: EntityId) -> Result<(), QueryError> {
        self.ensure_target(entity)?;
        self.queue
            .enqueue_remove::<T>(entity)
            .map_err(map_command_error)
    }

    pub fn insert_bundle<B: Bundle>(
        &mut self,
        entity: EntityId,
        bundle: B,
    ) -> Result<(), QueryError> {
        self.ensure_target(entity)?;
        let queue_len = self.queue.len();
        match bundle.write(&mut BundleWriter::query(self.allocator, self.queue, entity)) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.queue.truncate(queue_len);
                Err(map_command_error(error))
            }
        }
    }

    fn ensure_target(&self, entity: EntityId) -> Result<(), QueryError> {
        if self.allocator.is_alive(entity) || self.allocator.is_reserved(entity) {
            Ok(())
        } else {
            Err(QueryError::CommandRejected {
                detail: alloc::format!("stale command target {entity:?}"),
            })
        }
    }
}

/// Restricted side-effect surface for query traversal callbacks.
pub struct QueryEffects<'w> {
    owner: WorldOwner,
    run_guard: RunGuard,
    command_queue: &'w mut CommandQueue,
    allocator: &'w mut EntityAllocator,
    events: &'w mut WorldEvents,
}

impl<'w> QueryEffects<'w> {
    pub(crate) fn from_parts(
        command_queue: &'w mut CommandQueue,
        allocator: &'w mut EntityAllocator,
        events: &'w mut WorldEvents,
        run_guard: RunGuard,
        owner: WorldOwner,
    ) -> Self {
        Self {
            owner,
            run_guard,
            command_queue,
            allocator,
            events,
        }
    }

    pub fn commands(&mut self) -> Result<QueryCommands<'_>, QueryError> {
        match self.run_guard {
            RunGuard::Running(StageOperation::Update) => {}
            RunGuard::Running(StageOperation::Render) => {
                return Err(QueryError::BorrowConflict {
                    detail: String::from("structural commands are unavailable during Render"),
                });
            }
            RunGuard::Idle => {
                return Err(QueryError::BorrowConflict {
                    detail: String::from(
                        "structural commands require an active Update operation context",
                    ),
                });
            }
        }
        Ok(QueryCommands {
            allocator: self.allocator,
            queue: self.command_queue,
        })
    }

    pub fn send<E: Clone + 'static>(&mut self, event: E) -> Result<(), QueryError> {
        let event_id = self
            .events
            .registry
            .id_of::<E>(&self.owner)
            .ok_or_else(|| QueryError::WrongQuery {
                detail: alloc::format!("unregistered event {}", type_name::<E>()),
            })?;
        self.events
            .storage
            .send(&event_id, event)
            .map_err(|error| QueryError::WrongQuery {
                detail: alloc::format!("{error:?}"),
            })
    }
}

fn map_allocator_error_query(error: AllocatorError) -> QueryError {
    let detail = match error {
        AllocatorError::GenerationOverflow => String::from("allocator generation overflow"),
        AllocatorError::SlotRetired => String::from("allocator slot retired"),
        AllocatorError::StaleEntity | AllocatorError::DoubleFree | AllocatorError::NotLive => {
            String::from("allocator rejected entity")
        }
    };
    QueryError::CommandRejected { detail }
}

fn map_command_error(error: WorldError) -> QueryError {
    QueryError::CommandRejected {
        detail: alloc::format!("{error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn send_ok_path_propagates_success() {
        use crate::component::ComponentOptions;
        use crate::event::{EventOptions, EventReaderStart};
        use crate::operation::StageOperation;
        use crate::world::WorldBuilder;

        #[derive(Clone, Copy, Debug, PartialEq)]
        struct Ping(u8);

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Ping>(ComponentOptions::sparse())
            .expect("component");
        builder
            .add_event::<Ping>(EventOptions::frame(StageOperation::Update))
            .expect("event");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Ping(1)).expect("insert");
        let mut reader = world
            .event_reader::<Ping>(EventReaderStart::OldestRetained)
            .expect("reader");
        world.begin_run(StageOperation::Update).expect("begin");
        world
            .for_each_mut_with_effects::<Ping>(
                &crate::query::QuerySpec::new(),
                crate::query::QueryParams::new(),
                |_, _, effects| effects.send(Ping(2)).map(|_| ()),
            )
            .expect("send");
        world.end_run();
        assert_eq!(
            world.read_event(&mut reader).expect("read").map(|p| p.0),
            Some(2)
        );
        assert!(world.read_event(&mut reader).expect("drain").is_none());
    }

    #[test]
    fn send_maps_closed_channel_errors() {
        use crate::component::ComponentOptions;
        use crate::event::EventOptions;
        use crate::operation::StageOperation;
        use crate::world::WorldBuilder;

        #[derive(Clone, Copy)]
        struct Ping(#[allow(dead_code)] u8);

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Ping>(ComponentOptions::sparse())
            .expect("component");
        builder
            .add_event::<Ping>(EventOptions::frame(StageOperation::Update))
            .expect("event");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Ping(1)).expect("insert");
        world.set_event_sequence_for_test(2, 0, true);
        world.begin_run(StageOperation::Update).expect("begin");
        let err = world
            .for_each_mut_with_effects::<Ping>(
                &crate::query::QuerySpec::new(),
                crate::query::QueryParams::new(),
                |_, _, effects| effects.send(Ping(2)).map(|_| ()),
            )
            .expect_err("closed");
        world.end_run();
        assert!(matches!(err, QueryError::WrongQuery { .. }));
    }

    #[test]
    fn map_allocator_error_query_covers_all_variants() {
        assert!(matches!(
            map_allocator_error_query(AllocatorError::GenerationOverflow),
            QueryError::CommandRejected { .. }
        ));
        assert!(matches!(
            map_allocator_error_query(AllocatorError::SlotRetired),
            QueryError::CommandRejected { .. }
        ));
        assert!(matches!(
            map_allocator_error_query(AllocatorError::StaleEntity),
            QueryError::CommandRejected { .. }
        ));
    }
}
