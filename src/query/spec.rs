use alloc::vec::Vec;
use core::any::TypeId;

use crate::component::ComponentId;
use crate::query::ExactIdPolicy;
use crate::EntityId;

/// Structural query selection and filter authoring.
#[derive(Clone, Debug, Default)]
pub struct QuerySpec {
    pub(crate) required: Vec<TypeId>,
    pub(crate) required_ids: Vec<ComponentId>,
    pub(crate) without: Vec<TypeId>,
    pub(crate) without_ids: Vec<ComponentId>,
    pub(crate) with_tags: Vec<TypeId>,
    pub(crate) with_tag_ids: Vec<ComponentId>,
    pub(crate) without_tags: Vec<TypeId>,
    pub(crate) without_tag_ids: Vec<ComponentId>,
    pub(crate) added: Vec<TypeId>,
    pub(crate) added_ids: Vec<ComponentId>,
    pub(crate) changed: Vec<TypeId>,
    pub(crate) changed_ids: Vec<ComponentId>,
    pub(crate) exact_ids: Option<Vec<EntityId>>,
    pub(crate) exact_id_policy: Option<ExactIdPolicy>,
}

impl QuerySpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<T: 'static>(mut self) -> Self {
        self.required.push(TypeId::of::<T>());
        self
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.required_ids.push(id);
        self
    }

    pub fn without<T: 'static>(mut self) -> Self {
        self.without.push(TypeId::of::<T>());
        self
    }

    pub fn without_id(mut self, id: ComponentId) -> Self {
        self.without_ids.push(id);
        self
    }

    pub fn with_tag<T: 'static>(mut self) -> Self {
        self.with_tags.push(TypeId::of::<T>());
        self
    }

    pub fn with_tag_id(mut self, id: ComponentId) -> Self {
        self.with_tag_ids.push(id);
        self
    }

    pub fn without_tag<T: 'static>(mut self) -> Self {
        self.without_tags.push(TypeId::of::<T>());
        self
    }

    pub fn without_tag_id(mut self, id: ComponentId) -> Self {
        self.without_tag_ids.push(id);
        self
    }

    pub fn added<T: 'static>(mut self) -> Self {
        self.added.push(TypeId::of::<T>());
        self
    }

    pub fn added_id(mut self, id: ComponentId) -> Self {
        self.added_ids.push(id);
        self
    }

    pub fn changed<T: 'static>(mut self) -> Self {
        self.changed.push(TypeId::of::<T>());
        self
    }

    pub fn changed_id(mut self, id: ComponentId) -> Self {
        self.changed_ids.push(id);
        self
    }

    pub fn exact_ids(mut self, ids: Vec<EntityId>, policy: ExactIdPolicy) -> Self {
        self.exact_ids = Some(ids);
        self.exact_id_policy = Some(policy);
        self
    }
}
