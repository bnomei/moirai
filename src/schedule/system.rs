use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::TypeId;

use crate::schedule::condition::Condition;
use crate::schedule::owner::ScheduleOwner;

/// When deferred structural commands become visible during Update.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FlushMode {
    Final,
    Stage,
    AfterSystem,
}

pub(crate) type SystemBody = Box<dyn FnMut(&mut crate::world::World, f32) -> Result<(), String>>;

/// Opaque compiled system handle.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SystemId {
    owner: ScheduleOwner,
    index: u32,
    generation: u32,
}

impl SystemId {
    pub(crate) fn new(owner: ScheduleOwner, index: u32, generation: u32) -> Self {
        Self {
            owner,
            index,
            generation,
        }
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub(crate) fn validate_owner(
        &self,
        owner: &ScheduleOwner,
        generation: u32,
    ) -> Result<(), crate::schedule::ScheduleError> {
        if !self.owner.same(owner) {
            return Err(crate::schedule::ScheduleError::OwnerMismatch);
        }
        if self.generation != generation {
            return Err(crate::schedule::ScheduleError::StaleHandle);
        }
        Ok(())
    }
}

/// Authoring-time system-set label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemSet {
    label: String,
}

impl SystemSet {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Checked system descriptor with a required body.
pub struct System {
    pub(crate) name: String,
    pub(crate) stage_label: String,
    pub(crate) body: SystemBody,
    pub(crate) enabled: bool,
    pub(crate) flush_mode: FlushMode,
    pub(crate) before: Vec<String>,
    pub(crate) after: Vec<String>,
    pub(crate) in_set: Option<String>,
    pub(crate) conditions: Vec<Condition>,
    pub(crate) required_resources: Vec<TypeId>,
}

impl System {
    pub fn new(
        name: impl Into<String>,
        stage: impl Into<String>,
        body: impl FnMut(&mut crate::world::World, f32) + 'static,
    ) -> Self {
        let mut handler = body;
        Self {
            name: name.into(),
            stage_label: stage.into(),
            body: Box::new(move |world, dt| {
                handler(world, dt);
                Ok(())
            }),
            enabled: true,
            flush_mode: FlushMode::Final,
            before: Vec::new(),
            after: Vec::new(),
            in_set: None,
            conditions: Vec::new(),
            required_resources: Vec::new(),
        }
    }

    pub fn try_new(
        name: impl Into<String>,
        stage: impl Into<String>,
        body: impl FnMut(&mut crate::world::World, f32) -> Result<(), String> + 'static,
    ) -> Self {
        let mut handler = body;
        Self {
            name: name.into(),
            stage_label: stage.into(),
            body: Box::new(move |world, dt| handler(world, dt)),
            enabled: true,
            flush_mode: FlushMode::Final,
            before: Vec::new(),
            after: Vec::new(),
            in_set: None,
            conditions: Vec::new(),
            required_resources: Vec::new(),
        }
    }

    pub fn before(mut self, label: impl Into<String>) -> Self {
        self.before.push(label.into());
        self
    }

    pub fn after(mut self, label: impl Into<String>) -> Self {
        self.after.push(label.into());
        self
    }

    pub fn in_set(mut self, set: &SystemSet) -> Self {
        self.in_set = Some(set.label.clone());
        self
    }

    pub fn run_if(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn requires_resource<R: 'static>(mut self) -> Self {
        self.required_resources.push(TypeId::of::<R>());
        self
    }

    pub fn flush_mode(mut self, mode: FlushMode) -> Self {
        self.flush_mode = mode;
        self
    }

    pub fn flush_after(mut self) -> Self {
        self.flush_mode = FlushMode::AfterSystem;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::ScheduleError;
    use crate::world::WorldBuilder;

    #[test]
    fn system_id_validate_owner_and_generation() {
        let owner = ScheduleOwner::new();
        let id = SystemId::new(owner.clone(), 0, 1);
        assert!(id.validate_owner(&owner, 1).is_ok());
        assert!(matches!(
            id.validate_owner(&ScheduleOwner::new(), 1),
            Err(ScheduleError::OwnerMismatch)
        ));
        assert!(matches!(
            id.validate_owner(&owner, 0),
            Err(ScheduleError::StaleHandle)
        ));
    }

    #[test]
    fn system_builder_fluent_api() {
        let set = SystemSet::new("physics");
        let _ = System::new("move", "Update", |_world, _dt| {})
            .before("setup")
            .after("cleanup")
            .in_set(&set)
            .run_if(Condition::always())
            .requires_resource::<WorldBuilder>()
            .flush_mode(FlushMode::Stage)
            .flush_after()
            .disabled()
            .name();
    }
}
