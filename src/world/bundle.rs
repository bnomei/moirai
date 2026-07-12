use alloc::boxed::Box;
use alloc::vec::Vec;
use crate::command::{CommandOp, ErasedComponentValue};
use crate::component::ComponentId;
use crate::entity::EntityId;
use crate::world::{World, WorldError};

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

    pub fn push<T: Clone + 'static>(
        &mut self,
        world: &World,
        value: T,
    ) -> Result<(), WorldError> {
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
        self.entries.push(DynamicEntry { component_id, value });
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
    world: &'w mut World,
    entity: EntityId,
    deferred: bool,
}

impl<'w> BundleWriter<'w> {
    pub(crate) fn new(world: &'w mut World, entity: EntityId) -> Self {
        Self {
            world,
            entity,
            deferred: false,
        }
    }

    pub(crate) fn deferred(world: &'w mut World, entity: EntityId) -> Self {
        Self {
            world,
            entity,
            deferred: true,
        }
    }

    pub fn insert<T: Clone + 'static>(&mut self, value: T) -> Result<(), WorldError> {
        if self.deferred {
            self.world.ensure_mutable()?;
            self.world.ensure_command_target(self.entity)?;
            let component_index = self.world.component_index::<T>()? as u32;
            self.world.command_queue_mut().push(CommandOp::Insert {
                entity: self.entity,
                component_index,
                value: Box::new(value),
            });
            Ok(())
        } else {
            self.world.insert(self.entity, value).map(|_| ())
        }
    }

    pub(crate) fn insert_dynamic(
        &mut self,
        component_id: ComponentId,
        value: Box<dyn ErasedComponentValue>,
    ) -> Result<(), WorldError> {
        if self.deferred {
            self.world.ensure_mutable()?;
            self.world.ensure_command_target(self.entity)?;
            self.world.command_queue_mut().push(CommandOp::Insert {
                entity: self.entity,
                component_index: component_id.index() as u32,
                value,
            });
            Ok(())
        } else {
            self.world
                .insert_dynamic(self.entity, component_id, value)
                .map(|_| ())
        }
    }

    pub(crate) fn insert_tag_id(&mut self, component_id: ComponentId) -> Result<(), WorldError> {
        if self.deferred {
            self.world.ensure_mutable()?;
            self.world.ensure_command_target(self.entity)?;
            self.world.command_queue_mut().push(CommandOp::InsertTag {
                entity: self.entity,
                component_index: component_id.index() as u32,
            });
            Ok(())
        } else {
            self.world.add_tag_id(self.entity, component_id)
        }
    }

    pub(crate) fn world_owner(&self) -> &crate::world::WorldOwner {
        self.world.owner()
    }

    pub(crate) fn is_tag_component(&self, component_id: &ComponentId) -> bool {
        self.world.is_tag_component(component_id)
    }
}

macro_rules! impl_bundle_tuple {
    ($($name:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($name: Clone + 'static),+> Bundle for ($($name,)+) {
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