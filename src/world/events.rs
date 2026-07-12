use core::any::type_name;

use crate::event::{EventId, EventOptions, EventReader, EventReaderStart, EventRegistrationError};
use crate::world::{EventReadError, World, WorldError};

impl World {
    pub fn add_event<E: 'static>(&mut self, options: EventOptions) -> Result<EventId, WorldError> {
        let event_id = self
            .events
            .registry
            .register::<E>(&self.owner, options)
            .map_err(map_event_registration_error)?;
        self.events
            .storage
            .ensure_channel(event_id.index(), options.retention());
        Ok(event_id)
    }

    pub fn send<E: Clone + 'static>(&mut self, event: E) -> Result<(), WorldError> {
        let event_id = self
            .events
            .registry
            .id_of::<E>(&self.owner)
            .ok_or_else(|| WorldError::UnregisteredEvent {
                name: alloc::string::String::from(type_name::<E>()),
            })?;
        self.events.storage.send(&event_id, event)
    }

    pub fn event_reader<E: 'static>(
        &mut self,
        start: EventReaderStart,
    ) -> Result<EventReader<E>, WorldError> {
        let event_id = self
            .events
            .registry
            .id_of::<E>(&self.owner)
            .ok_or_else(|| WorldError::UnregisteredEvent {
                name: alloc::string::String::from(type_name::<E>()),
            })?;
        self.events
            .storage
            .create_reader(self.owner.clone(), event_id, start)
    }

    pub fn read_event<E: 'static>(
        &self,
        reader: &mut EventReader<E>,
    ) -> Result<Option<&E>, EventReadError> {
        self.events.storage.read_next(reader)
    }

    pub(crate) fn fork_event_reader<E: 'static>(
        &mut self,
        reader: &EventReader<E>,
    ) -> Result<EventReader<E>, WorldError> {
        reader
            .event_id
            .validate_owner(&self.owner)
            .map_err(map_event_registration_error)?;
        self.events.storage.fork_reader(reader)
    }

    #[allow(dead_code)]
    pub(crate) fn clear_frame_events(&mut self, operation: crate::operation::StageOperation) {
        self.events.storage.clear_frame(operation);
    }
}

fn map_event_registration_error(error: EventRegistrationError) -> WorldError {
    match error {
        EventRegistrationError::TypeConflict { name, .. } => WorldError::UnregisteredEvent { name },
    }
}