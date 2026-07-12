use alloc::boxed::Box;
use alloc::rc::Rc;
use core::any::TypeId;

use crate::schedule::RunContext;
use crate::state::State;
use crate::time::ChangeTick;
use crate::world::World;

type Predicate = Rc<dyn Fn(&World) -> bool>;

/// Run condition evaluated against read-only world state before a system body runs.
#[derive(Clone)]
pub struct Condition(ConditionKind);

#[derive(Clone)]
enum ConditionKind {
    Always,
    Never,
    ResourceExists(TypeId),
    ResourceAdded(TypeId),
    ResourceChanged(TypeId),
    StateChanged(TypeId),
    And(Box<ConditionKind>, Box<ConditionKind>),
    Or(Box<ConditionKind>, Box<ConditionKind>),
    Predicate(Predicate),
}

impl Condition {
    pub const fn always() -> Self {
        Self(ConditionKind::Always)
    }

    pub const fn never() -> Self {
        Self(ConditionKind::Never)
    }

    pub fn resource_exists<R: 'static>() -> Self {
        Self(ConditionKind::ResourceExists(TypeId::of::<R>()))
    }

    pub fn resource_added<R: 'static>() -> Self {
        Self(ConditionKind::ResourceAdded(TypeId::of::<R>()))
    }

    pub fn resource_changed<R: 'static>() -> Self {
        Self(ConditionKind::ResourceChanged(TypeId::of::<R>()))
    }

    pub fn state_changed<S: Eq + 'static>() -> Self {
        Self(ConditionKind::StateChanged(TypeId::of::<State<S>>()))
    }

    pub fn in_state<S: Eq + 'static>(value: S) -> Self {
        let expected = value;
        Self::predicate(Rc::new(move |world| {
            world
                .state_current::<S>()
                .ok()
                .flatten()
                .is_some_and(|current| *current == expected)
        }))
    }

    pub fn and(self, other: Self) -> Self {
        Self(ConditionKind::And(Box::new(self.0), Box::new(other.0)))
    }

    pub fn or(self, other: Self) -> Self {
        Self(ConditionKind::Or(Box::new(self.0), Box::new(other.0)))
    }

    fn predicate(predicate: Predicate) -> Self {
        Self(ConditionKind::Predicate(predicate))
    }

    pub(crate) fn evaluate(
        &self,
        world: &World,
        system_index: usize,
        context: &RunContext,
    ) -> bool {
        match &self.0 {
            ConditionKind::Always => true,
            ConditionKind::Never => false,
            ConditionKind::ResourceExists(type_id) => world.resource_present(*type_id),
            ConditionKind::ResourceAdded(type_id) => resource_tick_advanced(
                world.resource_added_tick_for(*type_id),
                context.resource_added_cursor(system_index, *type_id),
            ),
            ConditionKind::ResourceChanged(type_id) => resource_tick_advanced(
                world.resource_changed_tick_for(*type_id),
                context.resource_changed_cursor(system_index, *type_id),
            ),
            ConditionKind::StateChanged(type_id) => state_tick_advanced(
                world.state_transition_tick_for(*type_id),
                context.state_transition_cursor(system_index, *type_id),
            ),
            ConditionKind::And(left, right) => {
                let left = Condition(left.as_ref().clone());
                let right = Condition(right.as_ref().clone());
                left.evaluate(world, system_index, context)
                    && right.evaluate(world, system_index, context)
            }
            ConditionKind::Or(left, right) => {
                let left = Condition(left.as_ref().clone());
                let right = Condition(right.as_ref().clone());
                left.evaluate(world, system_index, context)
                    || right.evaluate(world, system_index, context)
            }
            ConditionKind::Predicate(predicate) => predicate(world),
        }
    }

    pub(crate) fn evaluate_for_set(
        &self,
        world: &World,
        set_label: &str,
        context: &RunContext,
    ) -> bool {
        match &self.0 {
            ConditionKind::Always => true,
            ConditionKind::Never => false,
            ConditionKind::ResourceExists(type_id) => world.resource_present(*type_id),
            ConditionKind::ResourceAdded(type_id) => resource_tick_advanced(
                world.resource_added_tick_for(*type_id),
                context.resource_added_cursor_for_set(set_label, *type_id),
            ),
            ConditionKind::ResourceChanged(type_id) => resource_tick_advanced(
                world.resource_changed_tick_for(*type_id),
                context.resource_changed_cursor_for_set(set_label, *type_id),
            ),
            ConditionKind::StateChanged(type_id) => state_tick_advanced(
                world.state_transition_tick_for(*type_id),
                context.state_transition_cursor_for_set(set_label, *type_id),
            ),
            ConditionKind::And(left, right) => {
                let left = Condition(left.as_ref().clone());
                let right = Condition(right.as_ref().clone());
                left.evaluate_for_set(world, set_label, context)
                    && right.evaluate_for_set(world, set_label, context)
            }
            ConditionKind::Or(left, right) => {
                let left = Condition(left.as_ref().clone());
                let right = Condition(right.as_ref().clone());
                left.evaluate_for_set(world, set_label, context)
                    || right.evaluate_for_set(world, set_label, context)
            }
            ConditionKind::Predicate(predicate) => predicate(world),
        }
    }

    pub(crate) fn advance_cursors(
        &self,
        world: &World,
        system_index: usize,
        context: &mut RunContext,
    ) {
        match &self.0 {
            ConditionKind::ResourceAdded(type_id) => {
                if let Some(tick) = world.resource_added_tick_for(*type_id) {
                    context.set_resource_added_cursor(system_index, *type_id, tick);
                }
            }
            ConditionKind::ResourceChanged(type_id) => {
                if let Some(tick) = world.resource_changed_tick_for(*type_id) {
                    context.set_resource_changed_cursor(system_index, *type_id, tick);
                }
            }
            ConditionKind::StateChanged(type_id) => {
                if let Some(tick) = world.state_transition_tick_for(*type_id) {
                    context.set_state_transition_cursor(system_index, *type_id, tick);
                }
            }
            ConditionKind::And(left, right) => {
                Condition(left.as_ref().clone()).advance_cursors(world, system_index, context);
                Condition(right.as_ref().clone()).advance_cursors(world, system_index, context);
            }
            ConditionKind::Or(left, right) => {
                Condition(left.as_ref().clone()).advance_cursors(world, system_index, context);
                Condition(right.as_ref().clone()).advance_cursors(world, system_index, context);
            }
            _ => {}
        }
    }

    pub(crate) fn advance_set_cursors(
        &self,
        world: &World,
        set_label: &str,
        context: &mut RunContext,
    ) {
        match &self.0 {
            ConditionKind::ResourceAdded(type_id) => {
                if let Some(tick) = world.resource_added_tick_for(*type_id) {
                    context.set_resource_added_cursor_for_set(set_label, *type_id, tick);
                }
            }
            ConditionKind::ResourceChanged(type_id) => {
                if let Some(tick) = world.resource_changed_tick_for(*type_id) {
                    context.set_resource_changed_cursor_for_set(set_label, *type_id, tick);
                }
            }
            ConditionKind::StateChanged(type_id) => {
                if let Some(tick) = world.state_transition_tick_for(*type_id) {
                    context.set_state_transition_cursor_for_set(set_label, *type_id, tick);
                }
            }
            ConditionKind::And(left, right) => {
                Condition(left.as_ref().clone()).advance_set_cursors(world, set_label, context);
                Condition(right.as_ref().clone()).advance_set_cursors(world, set_label, context);
            }
            ConditionKind::Or(left, right) => {
                Condition(left.as_ref().clone()).advance_set_cursors(world, set_label, context);
                Condition(right.as_ref().clone()).advance_set_cursors(world, set_label, context);
            }
            _ => {}
        }
    }
}

impl core::fmt::Debug for Condition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Condition")
    }
}

fn resource_tick_advanced(current: Option<ChangeTick>, cursor: ChangeTick) -> bool {
    current.is_some_and(|tick| tick > cursor)
}

fn state_tick_advanced(current: Option<ChangeTick>, cursor: ChangeTick) -> bool {
    current.is_some_and(|tick| tick > cursor)
}
