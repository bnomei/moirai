use core::any::{type_name, TypeId};

use crate::event::{ComponentAdded, ComponentRemoved, EventReader, EventReaderStart};
use crate::world::{EventReadError, World, WorldError};

impl World {
    pub fn send<E: Clone + 'static>(&mut self, event: E) -> Result<(), WorldError> {
        let event_id = self
            .events
            .registry
            .id_of::<E>(&self.owner)
            .ok_or_else(|| WorldError::UnregisteredEvent {
                name: alloc::string::String::from(type_name::<E>()),
            })?;
        self.ensure_event_emit_allowed(&event_id)?;
        self.events.storage.send(&event_id, event)
    }

    pub fn event_reader<E: Clone + 'static>(
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
        self.ensure_event_consume_allowed(&event_id)?;
        self.events
            .storage
            .create_reader(self.owner.clone(), event_id, start)
    }

    pub fn read_event<'a, E: Clone + 'static>(
        &mut self,
        reader: &'a mut EventReader<E>,
    ) -> Result<Option<&'a E>, EventReadError> {
        if reader.event_id.validate_owner(&self.owner).is_err() {
            return Err(EventReadError::OwnerMismatch {
                name: alloc::format!("event {}", reader.event_id.index()),
            });
        }
        if !self.run_guard.permits_consume(&reader.event_id) {
            return Err(EventReadError::UnregisteredEvent {
                name: alloc::format!("undeclared event {}", reader.event_id.index()),
            });
        }
        self.events.storage.read_next(&self.owner, reader)
    }

    pub fn on_add_reader<T: 'static>(
        &mut self,
        start: EventReaderStart,
    ) -> Result<EventReader<ComponentAdded>, WorldError> {
        let component_index = self.component_index::<T>()?;
        let event_id = self
            .events
            .lifecycle
            .added_event_id(&self.owner, component_index)
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: alloc::string::String::from(type_name::<T>()),
            })?;
        self.ensure_event_consume_allowed(&event_id)?;
        self.events
            .storage
            .create_reader(self.owner.clone(), event_id, start)
    }

    pub fn on_remove_reader<T: 'static>(
        &mut self,
        start: EventReaderStart,
    ) -> Result<EventReader<ComponentRemoved>, WorldError> {
        let component_index = self.component_index::<T>()?;
        let event_id = self
            .events
            .lifecycle
            .removed_event_id(&self.owner, component_index)
            .ok_or_else(|| WorldError::UnregisteredComponent {
                name: alloc::string::String::from(type_name::<T>()),
            })?;
        self.ensure_event_consume_allowed(&event_id)?;
        self.events
            .storage
            .create_reader(self.owner.clone(), event_id, start)
    }

    pub(crate) fn fork_event_reader<E: Clone + 'static>(
        &mut self,
        reader: &EventReader<E>,
    ) -> Result<EventReader<E>, WorldError> {
        self.ensure_event_consume_allowed(&reader.event_id)?;
        self.events.storage.fork_reader(&self.owner, reader)
    }

    pub(crate) fn event_id_of_type(&self, type_id: TypeId) -> Option<crate::event::EventId> {
        self.events.registry.id_of_type_id(&self.owner, type_id)
    }

    pub(crate) fn event_options(
        &self,
        event_id: &crate::event::EventId,
    ) -> Option<crate::event::EventOptions> {
        self.events.registry.options(event_id)
    }

    pub(crate) fn lifecycle_event_id(
        &self,
        component_type: TypeId,
        added: bool,
    ) -> Option<crate::event::EventId> {
        let component_index = self.registry_id_of_type(component_type)?.index();
        if added {
            self.events
                .lifecycle
                .added_event_id(&self.owner, component_index)
        } else {
            self.events
                .lifecycle
                .removed_event_id(&self.owner, component_index)
        }
    }

    fn ensure_event_emit_allowed(
        &self,
        event_id: &crate::event::EventId,
    ) -> Result<(), WorldError> {
        if self.run_guard.permits_emit(event_id) {
            Ok(())
        } else {
            Err(WorldError::UnregisteredEvent {
                name: alloc::format!("undeclared event {}", event_id.index()),
            })
        }
    }

    fn ensure_event_consume_allowed(
        &self,
        event_id: &crate::event::EventId,
    ) -> Result<(), WorldError> {
        if self.run_guard.permits_consume(event_id) {
            Ok(())
        } else {
            Err(WorldError::UnregisteredEvent {
                name: alloc::format!("undeclared event {}", event_id.index()),
            })
        }
    }

    #[cfg(any(test, feature = "testkit"))]
    pub fn set_event_sequence_for_test(
        &mut self,
        event_index: usize,
        next_sequence: u64,
        closed: bool,
    ) {
        self.events
            .storage
            .set_channel_state_for_test(event_index, next_sequence, closed);
    }

    #[allow(dead_code)]
    pub(crate) fn clear_frame_events(&mut self, operation: crate::operation::StageOperation) {
        self.events.storage.clear_frame(operation);
    }

    pub(crate) fn emit_component_added(
        &mut self,
        entity: crate::entity::EntityId,
        component_index: usize,
        is_new: bool,
    ) -> Result<(), WorldError> {
        if self.lifecycle_events_suppressed {
            return Ok(());
        }
        if !is_new {
            return Ok(());
        }
        self.bump_query_topology();
        match self.events.lifecycle.emit_added(
            &mut self.events.storage,
            &self.owner,
            entity,
            component_index,
        ) {
            Ok(()) | Err(WorldError::EventChannelClosed) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn emit_component_removed_if(
        &mut self,
        should_emit: bool,
        entity: crate::entity::EntityId,
        component_index: usize,
    ) -> Result<(), WorldError> {
        match should_emit {
            true => self.emit_component_removed(entity, component_index),
            false => Ok(()),
        }
    }

    pub(crate) fn emit_component_removed(
        &mut self,
        entity: crate::entity::EntityId,
        component_index: usize,
    ) -> Result<(), WorldError> {
        if self.lifecycle_events_suppressed {
            return Ok(());
        }
        self.bump_query_topology();
        match self.events.lifecycle.emit_removed(
            &mut self.events.storage,
            &self.owner,
            entity,
            component_index,
        ) {
            Ok(()) | Err(WorldError::EventChannelClosed) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::event::EventReaderStart;
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Health(#[allow(dead_code)] i32);

    #[test]
    fn event_reader_rejects_unregistered_event_type() {
        let mut world = WorldBuilder::new().build().expect("world");
        assert!(matches!(
            world.event_reader::<Health>(EventReaderStart::OldestRetained),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn lifecycle_readers_reject_missing_lifecycle_channels() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("world");
        world.events.lifecycle.clear_added_event_for_test(0);
        assert!(matches!(
            world.on_add_reader::<Health>(EventReaderStart::OldestRetained),
            Err(WorldError::UnregisteredComponent { .. })
        ));
        world.events.lifecycle.clear_removed_event_for_test(0);
        assert!(matches!(
            world.on_remove_reader::<Health>(EventReaderStart::OldestRetained),
            Err(WorldError::UnregisteredComponent { .. })
        ));
    }

    #[test]
    fn emit_component_added_propagates_non_closed_send_errors() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.emit_component_added(entity, 0, true),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }

    #[test]
    fn emit_component_removed_if_skips_emit_when_not_requested() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Health(1)).expect("insert");
        let mut reader = world
            .on_remove_reader::<Health>(EventReaderStart::OldestRetained)
            .expect("reader");

        world
            .emit_component_removed_if(false, entity, 0)
            .expect("skip emit");
        assert!(world
            .read_event(&mut reader)
            .expect("read after skip")
            .is_none());

        world
            .emit_component_removed_if(true, entity, 0)
            .expect("emit");
        let event = world
            .read_event(&mut reader)
            .expect("read after emit")
            .expect("removed event");
        assert_eq!(event.entity, entity);
    }

    #[test]
    fn emit_component_removed_propagates_non_closed_send_errors() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("register");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Health(1)).expect("insert");
        world.events.storage.clear_channels_for_test();
        assert!(matches!(
            world.emit_component_removed(entity, 0),
            Err(WorldError::UnregisteredEvent { .. })
        ));
    }
}
