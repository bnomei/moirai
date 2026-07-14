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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::world::WorldBuilder;

    #[derive(Clone, Copy)]
    struct Position;

    #[derive(Clone, Copy)]
    struct Player;

    #[test]
    fn dynamic_id_builders_populate_each_selector_group() {
        let mut builder = WorldBuilder::new();
        let position = builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("position");
        let player = builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("player");

        let spec = QuerySpec::new()
            .with_id(position.clone())
            .without_id(position.clone())
            .with_tag_id(player.clone())
            .without_tag_id(player)
            .added_id(position.clone())
            .changed_id(position);

        assert_eq!(spec.required_ids.len(), 1);
        assert_eq!(spec.without_ids.len(), 1);
        assert_eq!(spec.with_tag_ids.len(), 1);
        assert_eq!(spec.without_tag_ids.len(), 1);
        assert_eq!(spec.added_ids.len(), 1);
        assert_eq!(spec.changed_ids.len(), 1);
    }
}
