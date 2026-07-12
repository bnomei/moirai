use crate::entity::EntityId;
use crate::world::{World, WorldError};

/// Checked bundle insertion through [`BundleWriter`].
pub trait Bundle {
    fn write(self, writer: &mut BundleWriter<'_>) -> Result<(), WorldError>;
}

/// Safe bundle write surface without storage access.
pub struct BundleWriter<'w> {
    world: &'w mut World,
    entity: EntityId,
}

impl<'w> BundleWriter<'w> {
    pub(crate) fn new(world: &'w mut World, entity: EntityId) -> Self {
        Self { world, entity }
    }

    pub fn insert<T: Clone + 'static>(&mut self, value: T) -> Result<(), WorldError> {
        self.world.insert(self.entity, value).map(|_| ())
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
