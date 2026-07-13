use crate::command::{CommandOp, ErasedComponentValue};
use crate::component::ComponentId;
use crate::entity::EntityId;
use crate::world::{World, WorldError};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Checked bundle insertion through [`BundleWriter`].
pub trait Bundle {
    fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError>;
}

/// Conditional bundle with validated component ids and owned values.
pub struct DynamicBundle {
    entries: Vec<DynamicEntry>,
}

struct DynamicEntry {
    component_id: ComponentId,
    value: Option<Box<dyn ErasedComponentValue>>,
}

impl DynamicBundle {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push<T: 'static>(&mut self, world: &World, value: T) -> Result<(), WorldError> {
        let component_id = world.component_id::<T>()?;
        if world.is_tag_component(&component_id) {
            return Err(WorldError::WrongStorageKind {
                name: alloc::string::String::from("tag components cannot carry values"),
            });
        }
        self.push_entry(component_id, Some(Box::new(value)))
    }

    pub fn push_tag(&mut self, tag: &ComponentId) -> Result<(), WorldError> {
        self.push_entry(tag.clone(), None)
    }

    fn push_entry(
        &mut self,
        component_id: ComponentId,
        value: Option<Box<dyn ErasedComponentValue>>,
    ) -> Result<(), WorldError> {
        if self
            .entries
            .iter()
            .any(|entry| entry.component_id.index() == component_id.index())
        {
            return Err(WorldError::WrongStorageKind {
                name: alloc::string::String::from("duplicate component in dynamic bundle"),
            });
        }
        self.entries.push(DynamicEntry {
            component_id,
            value,
        });
        Ok(())
    }
}

impl Default for DynamicBundle {
    fn default() -> Self {
        Self::new()
    }
}

impl Bundle for DynamicBundle {
    fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError> {
        for entry in self.entries {
            entry.component_id.validate_owner(writer.world_owner())?;
            if writer.is_tag_component(&entry.component_id) {
                if entry.value.is_some() {
                    return Err(WorldError::WrongStorageKind {
                        name: alloc::string::String::from("tag components cannot carry values"),
                    });
                }
                writer.insert_tag_id(entry.component_id)?;
            } else if let Some(value) = entry.value {
                writer.insert_dynamic(entry.component_id, value)?;
            } else {
                return Err(WorldError::WrongStorageKind {
                    name: alloc::string::String::from("table/sparse components require values"),
                });
            }
        }
        Ok(())
    }
}

/// Safe bundle write surface without storage access.
pub struct BundleWriter<'w> {
    entity: EntityId,
    target: BundleTarget<'w>,
}

enum BundleTarget<'w> {
    Immediate(&'w mut World),
    Deferred(&'w mut World),
    Query {
        allocator: &'w crate::entity::EntityAllocator,
        queue: &'w mut crate::command::CommandQueue,
    },
}

impl<'w> BundleWriter<'w> {
    pub(crate) fn new(world: &'w mut World, entity: EntityId) -> Self {
        Self {
            entity,
            target: BundleTarget::Immediate(world),
        }
    }

    pub(crate) fn deferred(world: &'w mut World, entity: EntityId) -> Self {
        Self {
            entity,
            target: BundleTarget::Deferred(world),
        }
    }

    pub(crate) fn query(
        allocator: &'w crate::entity::EntityAllocator,
        queue: &'w mut crate::command::CommandQueue,
        entity: EntityId,
    ) -> Self {
        Self {
            entity,
            target: BundleTarget::Query { allocator, queue },
        }
    }

    pub fn insert<T: 'static>(&mut self, value: T) -> Result<(), WorldError> {
        match &mut self.target {
            BundleTarget::Immediate(world) => world.insert(self.entity, value).map(|_| ()),
            BundleTarget::Deferred(world) => {
                world.ensure_mutable()?;
                world.ensure_command_target(self.entity)?;
                world.command_queue_mut().enqueue_insert(self.entity, value)
            }
            BundleTarget::Query { allocator, queue } => {
                ensure_query_target(allocator, self.entity)?;
                queue.enqueue_insert(self.entity, value)
            }
        }
    }

    pub(crate) fn insert_dynamic(
        &mut self,
        component_id: ComponentId,
        value: Box<dyn ErasedComponentValue>,
    ) -> Result<(), WorldError> {
        match &mut self.target {
            BundleTarget::Immediate(world) => world
                .insert_dynamic(self.entity, component_id, value)
                .map(|_| ()),
            BundleTarget::Deferred(world) => {
                world.ensure_mutable()?;
                world.ensure_command_target(self.entity)?;
                world.validate_component_insert(
                    self.entity,
                    component_id.index() as u32,
                    value.as_ref().type_id(),
                )?;
                world.command_queue_mut().push(CommandOp::Insert {
                    entity: self.entity,
                    component_index: component_id.index() as u32,
                    value,
                });
                Ok(())
            }
            BundleTarget::Query { allocator, queue } => {
                ensure_query_target(allocator, self.entity)?;
                queue.enqueue_dynamic_insert(self.entity, component_id.index(), value)
            }
        }
    }

    pub(crate) fn insert_tag_id(&mut self, component_id: ComponentId) -> Result<(), WorldError> {
        match &mut self.target {
            BundleTarget::Immediate(world) => world.add_tag_id(self.entity, component_id),
            BundleTarget::Deferred(world) => {
                world.ensure_mutable()?;
                world.ensure_command_target(self.entity)?;
                world
                    .command_queue_mut()
                    .enqueue_tag(self.entity, component_id.index())
            }
            BundleTarget::Query { allocator, queue } => {
                ensure_query_target(allocator, self.entity)?;
                queue.enqueue_tag(self.entity, component_id.index())
            }
        }
    }

    pub(crate) fn world_owner(&self) -> &crate::world::WorldOwner {
        match &self.target {
            BundleTarget::Immediate(world) | BundleTarget::Deferred(world) => world.owner(),
            BundleTarget::Query { queue, .. } => queue.owner(),
        }
    }

    pub(crate) fn is_tag_component(&self, component_id: &ComponentId) -> bool {
        match &self.target {
            BundleTarget::Immediate(world) | BundleTarget::Deferred(world) => {
                world.is_tag_component(component_id)
            }
            BundleTarget::Query { queue, .. } => queue.is_tag_component(component_id.index()),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_entity(&self) -> EntityId {
        self.entity
    }

    #[cfg(test)]
    pub(crate) fn test_world(&mut self) -> &mut World {
        match &mut self.target {
            BundleTarget::Immediate(world) | BundleTarget::Deferred(world) => world,
            BundleTarget::Query { .. } => panic!("query bundle writer has no world"),
        }
    }
}

fn ensure_query_target(
    allocator: &crate::entity::EntityAllocator,
    entity: EntityId,
) -> Result<(), WorldError> {
    if allocator.is_alive(entity) || allocator.is_reserved(entity) {
        Ok(())
    } else {
        Err(WorldError::StaleEntity { entity })
    }
}

macro_rules! impl_bundle_tuple {
    ($($name:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($name: 'static),+> Bundle for ($($name,)+) {
            fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError> {
                let ($($name,)+) = self;
                $(writer.insert($name)?;)+
                Ok(())
            }
        }
    };
}

impl_bundle_tuple!(A);
impl_bundle_tuple!(A, B);
impl_bundle_tuple!(A, B, C);
impl_bundle_tuple!(A, B, C, D);
impl_bundle_tuple!(A, B, C, D, E);
impl_bundle_tuple!(A, B, C, D, E, F);
impl_bundle_tuple!(A, B, C, D, E, F, G);
impl_bundle_tuple!(A, B, C, D, E, F, G, H);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_bundle_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Health(i32);

    #[derive(Clone, Copy)]
    struct Marker;

    #[test]
    fn dynamic_bundle_default_and_write_validation_errors() {
        assert_eq!(DynamicBundle::default().entries.len(), 0);
        let mut builder = WorldBuilder::new();
        let tag = builder
            .register_component::<Marker>(ComponentOptions::tag())
            .expect("tag");
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("health");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");

        let mut tag_with_value = DynamicBundle::new();
        tag_with_value.push_tag(&tag).expect("tag");
        tag_with_value.entries[0].value = Some(Box::new(Health(1)));
        assert!(matches!(
            tag_with_value.write(&mut BundleWriter::new(&mut world, entity)),
            Err(WorldError::WrongStorageKind { .. })
        ));

        let health_id = world.component_id::<Health>().expect("health");
        let mut missing_value = DynamicBundle::new();
        missing_value.push_entry(health_id, None).expect("entry");
        assert!(matches!(
            missing_value.write(&mut BundleWriter::new(&mut world, entity)),
            Err(WorldError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn deferred_bundle_writer_queues_inserts() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("health");
        let mut world = builder.build().expect("build");
        let entity = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        BundleWriter::deferred(&mut world, entity)
            .insert(Health(3))
            .expect("queue");
        assert!(world.has_pending_commands());
    }

    #[test]
    fn tuple_bundle_writes_components() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("health");
        let mut world = builder.build().expect("build");
        let entity = world.spawn().expect("spawn");
        (Health(4),)
            .write(&mut BundleWriter::new(&mut world, entity))
            .expect("tuple");
        assert_eq!(
            world.get::<Health>(entity).expect("get").map(|h| h.0),
            Some(4)
        );
    }
}
