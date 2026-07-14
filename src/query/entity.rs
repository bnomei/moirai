use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;

use crate::entity::EntityId;
use crate::time::ChangeTick;
use crate::world::{World, WorldError};

#[derive(Clone, Copy)]
pub struct EntityRef<'w> {
    pub(crate) world: &'w World,
    pub(crate) entity: EntityId,
}

impl<'w> EntityRef<'w> {
    pub fn id(self) -> EntityId {
        self.entity
    }

    pub fn has<T: 'static>(self) -> Result<bool, WorldError> {
        let id =
            self.world
                .registry_id_of::<T>()
                .ok_or_else(|| WorldError::UnregisteredComponent {
                    name: String::from(type_name::<T>()),
                })?;
        Ok(self.world.entity_has_component(self.entity, id.index()))
    }

    pub fn get<T: 'static>(self) -> Result<Option<&'w T>, WorldError> {
        self.world.get::<T>(self.entity)
    }
}

pub struct QueryIds<'w, 'c> {
    pub(crate) world: &'w World,
    pub(crate) ids: Vec<EntityId>,
    pub(crate) index: usize,
    pub(crate) exhausted: bool,
    pub(crate) fingerprint: u64,
    pub(crate) captured_now: ChangeTick,
    pub(crate) cursor_committed: bool,
    pub(crate) cursor: Option<&'c mut crate::query::QueryCursor>,
}

impl QueryIds<'_, '_> {
    fn commit_cursor_if_needed(&mut self) {
        if self.cursor_committed {
            return;
        }
        let world = self.world;
        let fingerprint = self.fingerprint;
        let cursor = self
            .cursor
            .as_mut()
            .filter(|cursor| cursor.validate(world, fingerprint).is_ok());
        if let Some(cursor) = cursor {
            cursor.commit(self.captured_now);
        }
        self.cursor_committed = true;
    }
}

impl Iterator for QueryIds<'_, '_> {
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(&entity) = self.ids.get(self.index) {
            self.index += 1;
            return Some(entity);
        }
        self.exhausted = true;
        self.commit_cursor_if_needed();
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.ids.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for QueryIds<'_, '_> {}

impl Drop for QueryIds<'_, '_> {
    fn drop(&mut self) {
        if self.exhausted {
            self.commit_cursor_if_needed();
        }
    }
}

pub struct QueryEntities<'w, 'c> {
    pub(crate) inner: QueryIds<'w, 'c>,
}

impl<'w> Iterator for QueryEntities<'w, '_> {
    type Item = EntityRef<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let entity = self.inner.next()?;
        Some(EntityRef {
            world: self.inner.world,
            entity,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for QueryEntities<'_, '_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{QueryCursor, QueryParams, QuerySpec};
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Position;

    #[derive(Clone, Copy)]
    struct Unregistered;

    #[test]
    fn entity_ref_has_reports_unregistered_component() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("position");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position).expect("position");
        let entity_ref = EntityRef {
            world: &world,
            entity,
        };

        assert!(matches!(
            entity_ref.has::<Unregistered>(),
            Err(WorldError::UnregisteredComponent { .. })
        ));
    }

    #[test]
    fn exhausted_query_ids_commit_the_entity_cursor() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("position");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position).expect("position");
        let spec = QuerySpec::new().added::<Position>();
        let mut cursor = QueryCursor::for_entities_from_start(&mut world, &spec).expect("cursor");
        let before = cursor.since();

        let ids = world
            .query_ids(&spec, QueryParams::new().cursor(&mut cursor))
            .expect("ids")
            .collect::<Vec<_>>();

        assert_eq!(ids, alloc::vec![entity]);
        assert!(cursor.since() > before);
    }
}
