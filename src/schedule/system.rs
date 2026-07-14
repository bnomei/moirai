//! Authoring-time system descriptors, flush policy, and opaque runtime handles.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::TypeId;

use crate::event::{ComponentAdded, ComponentRemoved, EventReader, EventReaderStart};
use crate::query::{PreparedQuery1, PreparedQuery2, QueryError, QueryPolicy, QuerySpec};
use crate::schedule::condition::Condition;
use crate::schedule::owner::ScheduleOwner;
use crate::world::{World, WorldError};

/// When deferred structural commands become visible during Update.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FlushMode {
    /// Flush once after all Update stages in the pass (default for Render).
    Final,
    /// Flush at the end of each Update stage (standard builder default).
    Stage,
    /// Flush immediately after this system when running under Update.
    AfterSystem,
}

pub(crate) type SystemBody = Box<dyn FnMut(&mut crate::world::World, f32) -> Result<(), String>>;
pub(crate) type SystemInitializer =
    Box<dyn for<'world> FnOnce(&mut SystemInitContext<'world>) -> Result<SystemBody, String>>;

pub(crate) enum SystemBodySource {
    Ready(SystemBody),
    Initialize(SystemInitializer),
}

/// Restricted build-time access used to create persistent system-local state.
///
/// Initializers may inspect resources and create event readers, but cannot
/// mutate the world. The context is constructed only while a schedule builds.
pub struct SystemInitContext<'world> {
    world: &'world mut World,
}

impl<'world> SystemInitContext<'world> {
    pub(crate) fn new(world: &'world mut World) -> Self {
        Self { world }
    }

    /// Whether the resource type is registered and present.
    pub fn contains_resource<R: 'static>(&self) -> bool {
        self.world.contains_resource::<R>()
    }

    /// Read-only resource access during initializer execution.
    pub fn resource<R: 'static>(&self) -> Result<Option<&R>, WorldError> {
        self.world.resource::<R>()
    }

    /// Persistent event reader seeded for the compiled system's lifetime.
    pub fn event_reader<E: Clone + 'static>(
        &mut self,
        start: EventReaderStart,
    ) -> Result<EventReader<E>, WorldError> {
        self.world.event_reader::<E>(start)
    }

    /// Persistent component-added lifecycle reader for this system.
    pub fn on_add_reader<T: 'static>(
        &mut self,
        start: EventReaderStart,
    ) -> Result<EventReader<ComponentAdded>, WorldError> {
        self.world.on_add_reader::<T>(start)
    }

    /// Persistent component-removed lifecycle reader for this system.
    pub fn on_remove_reader<T: 'static>(
        &mut self,
        start: EventReaderStart,
    ) -> Result<EventReader<ComponentRemoved>, WorldError> {
        self.world.on_remove_reader::<T>(start)
    }

    /// Resolves and stores a reusable single-component query for this system.
    pub fn prepare_query1<T: 'static>(
        &mut self,
        spec: QuerySpec,
        policy: QueryPolicy,
    ) -> Result<PreparedQuery1<T>, QueryError> {
        self.world.prepare_query1(spec, policy)
    }

    /// Resolves and stores a reusable two-component query for this system.
    pub fn prepare_query2<A: 'static, B: 'static>(
        &mut self,
        spec: QuerySpec,
        policy: QueryPolicy,
    ) -> Result<PreparedQuery2<A, B>, QueryError> {
        self.world.prepare_query2(spec, policy)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum EventRoleKind {
    Emits,
    Consumes,
    ConsumesOnAdd,
    ConsumesOnRemove,
}

#[derive(Clone, Debug)]
pub(crate) struct EventRole {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub kind: EventRoleKind,
}

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

    /// Stable compiled index for diagnostics; prefer label lookup for authoring.
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
    /// Declares a named group for shared ordering edges and run-if gates.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Set label used by the builder and ordering APIs.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Authoring-time system node: stage placement, ordering, conditions, and event roles.
pub struct System {
    pub(crate) name: String,
    pub(crate) stage_label: String,
    pub(crate) body: SystemBodySource,
    pub(crate) enabled: bool,
    pub(crate) flush_mode: FlushMode,
    pub(crate) before: Vec<String>,
    pub(crate) after: Vec<String>,
    pub(crate) before_sets: Vec<String>,
    pub(crate) after_sets: Vec<String>,
    pub(crate) in_set: Option<String>,
    pub(crate) conditions: Vec<Condition>,
    pub(crate) required_resources: Vec<TypeId>,
    pub(crate) event_roles: Vec<EventRole>,
}

impl System {
    /// Infallible body wrapper; panics and world errors must be handled inside the closure.
    pub fn new(
        name: impl Into<String>,
        stage: impl Into<String>,
        body: impl FnMut(&mut crate::world::World, f32) + 'static,
    ) -> Self {
        let mut handler = body;
        Self {
            name: name.into(),
            stage_label: stage.into(),
            body: SystemBodySource::Ready(Box::new(move |world, dt| {
                handler(world, dt);
                Ok(())
            })),
            enabled: true,
            flush_mode: FlushMode::Final,
            before: Vec::new(),
            after: Vec::new(),
            before_sets: Vec::new(),
            after_sets: Vec::new(),
            in_set: None,
            conditions: Vec::new(),
            required_resources: Vec::new(),
            event_roles: Vec::new(),
        }
    }

    /// Fallible body that can abort the stage pass with a detail string.
    pub fn try_new(
        name: impl Into<String>,
        stage: impl Into<String>,
        body: impl FnMut(&mut crate::world::World, f32) -> Result<(), String> + 'static,
    ) -> Self {
        let mut handler = body;
        Self {
            name: name.into(),
            stage_label: stage.into(),
            body: SystemBodySource::Ready(Box::new(move |world, dt| handler(world, dt))),
            enabled: true,
            flush_mode: FlushMode::Final,
            before: Vec::new(),
            after: Vec::new(),
            before_sets: Vec::new(),
            after_sets: Vec::new(),
            in_set: None,
            conditions: Vec::new(),
            required_resources: Vec::new(),
            event_roles: Vec::new(),
        }
    }

    /// Creates a system whose persistent local state is initialized at build time.
    pub fn with_local<L: 'static>(
        name: impl Into<String>,
        stage: impl Into<String>,
        init: impl FnOnce(&mut SystemInitContext<'_>) -> Result<L, String> + 'static,
        run: impl FnMut(&mut World, f32, &mut L) -> Result<(), String> + 'static,
    ) -> Self {
        let mut run = run;
        let initializer = move |context: &mut SystemInitContext<'_>| {
            let mut local = init(context)?;
            let body: SystemBody = Box::new(move |world, dt| run(world, dt, &mut local));
            Ok(body)
        };
        Self {
            name: name.into(),
            stage_label: stage.into(),
            body: SystemBodySource::Initialize(Box::new(initializer)),
            enabled: true,
            flush_mode: FlushMode::Final,
            before: Vec::new(),
            after: Vec::new(),
            before_sets: Vec::new(),
            after_sets: Vec::new(),
            in_set: None,
            conditions: Vec::new(),
            required_resources: Vec::new(),
            event_roles: Vec::new(),
        }
    }

    /// Runs before the named system within the same stage.
    pub fn before(mut self, label: impl Into<String>) -> Self {
        self.before.push(label.into());
        self
    }

    /// Runs after the named system within the same stage.
    pub fn after(mut self, label: impl Into<String>) -> Self {
        self.after.push(label.into());
        self
    }

    /// Runs before every system in the set that shares this stage.
    pub fn before_set(mut self, set: &SystemSet) -> Self {
        self.before_sets.push(set.label.clone());
        self
    }

    /// Runs after every system in the set that shares this stage.
    pub fn after_set(mut self, set: &SystemSet) -> Self {
        self.after_sets.push(set.label.clone());
        self
    }

    /// Membership for set-level ordering edges and shared run-if gates.
    pub fn in_set(mut self, set: &SystemSet) -> Self {
        self.in_set = Some(set.label.clone());
        self
    }

    /// Skips the system body when the condition evaluates false.
    pub fn run_if(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Build fails unless the resource is present; attaches a world lease lock.
    pub fn requires_resource<R: 'static>(mut self) -> Self {
        self.required_resources.push(TypeId::of::<R>());
        self
    }

    /// Declares that this system may send events of type `E`.
    pub fn emits<E: Clone + 'static>(mut self) -> Self {
        self.push_event_role::<E>(EventRoleKind::Emits);
        self
    }

    /// Declares that this system may create readers for and read events of type `E`.
    pub fn consumes<E: Clone + 'static>(mut self) -> Self {
        self.push_event_role::<E>(EventRoleKind::Consumes);
        self
    }

    /// Declares that this system consumes the added lifecycle channel for `T`.
    pub fn consumes_on_add<T: 'static>(mut self) -> Self {
        self.push_event_role::<T>(EventRoleKind::ConsumesOnAdd);
        self
    }

    /// Declares that this system consumes the removed lifecycle channel for `T`.
    pub fn consumes_on_remove<T: 'static>(mut self) -> Self {
        self.push_event_role::<T>(EventRoleKind::ConsumesOnRemove);
        self
    }

    fn push_event_role<T: 'static>(&mut self, kind: EventRoleKind) {
        let type_id = TypeId::of::<T>();
        if self
            .event_roles
            .iter()
            .any(|role| role.type_id == type_id && role.kind == kind)
        {
            return;
        }
        self.event_roles.push(EventRole {
            type_id,
            type_name: core::any::type_name::<T>(),
            kind,
        });
    }

    /// Overrides deferred-command flush timing for this system on Update stages.
    pub fn flush_mode(mut self, mode: FlushMode) -> Self {
        self.flush_mode = mode;
        self
    }

    /// Shorthand for [`FlushMode::AfterSystem`] on Update stages.
    pub fn flush_after(mut self) -> Self {
        self.flush_mode = FlushMode::AfterSystem;
        self
    }

    /// Registers the system but leaves it disabled until toggled at runtime.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Authoring label and runtime diagnostic name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::event::EventOptions;
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
            .before_set(&set)
            .after_set(&set)
            .in_set(&set)
            .run_if(Condition::always())
            .requires_resource::<WorldBuilder>()
            .emits::<u32>()
            .consumes::<u32>()
            .consumes_on_add::<u32>()
            .consumes_on_remove::<u32>()
            .flush_mode(FlushMode::Stage)
            .flush_after()
            .disabled()
            .name();
    }

    #[test]
    fn init_context_exposes_registered_runtime_state_and_prepared_queries() {
        struct Position;
        struct Velocity;
        #[derive(Clone)]
        struct Tick;

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("position");
        builder
            .register_component::<Velocity>(ComponentOptions::sparse())
            .expect("velocity");
        builder.insert_resource(7_u32);
        builder
            .add_event::<Tick>(EventOptions::manual())
            .expect("event");
        let mut world = builder.build().expect("world");
        let mut context = SystemInitContext::new(&mut world);

        assert!(context.contains_resource::<u32>());
        assert_eq!(context.resource::<u32>().expect("resource"), Some(&7));
        context
            .event_reader::<Tick>(EventReaderStart::FromNow)
            .expect("event reader");
        context
            .on_add_reader::<Position>(EventReaderStart::FromNow)
            .expect("add reader");
        context
            .on_remove_reader::<Position>(EventReaderStart::FromNow)
            .expect("remove reader");
        context
            .prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("query1");
        context
            .prepare_query2::<Position, Velocity>(QuerySpec::new(), QueryPolicy::Prepared)
            .expect("query2");
    }

    #[test]
    fn duplicate_event_role_is_suppressed() {
        let system = System::new("writer", "Update", |_world, _dt| {})
            .emits::<u32>()
            .emits::<u32>();
        assert_eq!(system.event_roles.len(), 1);
    }
}
