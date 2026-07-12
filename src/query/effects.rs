use alloc::string::String;
use core::any::type_name;

use crate::command::{CommandOp, CommandQueue};
use crate::entity::{AllocatorError, EntityAllocator, EntityId};
use crate::operation::StageOperation;
use crate::query::QueryError;
use crate::world::guard::RunGuard;
use crate::world::{WorldEvents, WorldOwner};

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
        self.queue.push(CommandOp::Despawn { entity });
        Ok(())
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
    match error {
        AllocatorError::GenerationOverflow => QueryError::BorrowConflict {
            detail: String::from("allocator generation overflow"),
        },
        AllocatorError::SlotRetired => QueryError::BorrowConflict {
            detail: String::from("allocator slot retired"),
        },
        AllocatorError::StaleEntity | AllocatorError::DoubleFree | AllocatorError::NotLive => {
            QueryError::BorrowConflict {
                detail: String::from("allocator rejected entity"),
            }
        }
    }
}
