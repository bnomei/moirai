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

    /// Creates a cloneable condition from a read-only world predicate.
    pub fn from_world<F>(predicate: F) -> Self
    where
        F: Fn(&World) -> bool + 'static,
    {
        Self::predicate(Rc::new(predicate))
    }

    pub fn in_state<S: Eq + 'static>(value: S) -> Self {
        let expected = value;
        Self::from_world(move |world| {
            world
                .state_current::<S>()
                .ok()
                .flatten()
                .is_some_and(|current| *current == expected)
        })
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
        evaluate_kind(&self.0, world, system_index, context)
    }

    pub(crate) fn evaluate_for_set(
        &self,
        world: &World,
        set_index: usize,
        context: &RunContext,
    ) -> bool {
        evaluate_kind_for_set(&self.0, world, set_index, context)
    }

    pub(crate) fn advance_cursors(
        &self,
        world: &World,
        system_index: usize,
        context: &mut RunContext,
    ) {
        advance_kind_cursors(&self.0, world, system_index, context);
    }

    pub(crate) fn advance_set_cursors(
        &self,
        world: &World,
        set_index: usize,
        context: &mut RunContext,
    ) {
        advance_kind_set_cursors(&self.0, world, set_index, context);
    }
}

fn evaluate_kind(
    kind: &ConditionKind,
    world: &World,
    system_index: usize,
    context: &RunContext,
) -> bool {
    match kind {
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
            evaluate_kind(left, world, system_index, context)
                && evaluate_kind(right, world, system_index, context)
        }
        ConditionKind::Or(left, right) => {
            evaluate_kind(left, world, system_index, context)
                || evaluate_kind(right, world, system_index, context)
        }
        ConditionKind::Predicate(predicate) => predicate(world),
    }
}

fn evaluate_kind_for_set(
    kind: &ConditionKind,
    world: &World,
    set_index: usize,
    context: &RunContext,
) -> bool {
    match kind {
        ConditionKind::Always => true,
        ConditionKind::Never => false,
        ConditionKind::ResourceExists(type_id) => world.resource_present(*type_id),
        ConditionKind::ResourceAdded(type_id) => resource_tick_advanced(
            world.resource_added_tick_for(*type_id),
            context.resource_added_cursor_for_set(set_index, *type_id),
        ),
        ConditionKind::ResourceChanged(type_id) => resource_tick_advanced(
            world.resource_changed_tick_for(*type_id),
            context.resource_changed_cursor_for_set(set_index, *type_id),
        ),
        ConditionKind::StateChanged(type_id) => state_tick_advanced(
            world.state_transition_tick_for(*type_id),
            context.state_transition_cursor_for_set(set_index, *type_id),
        ),
        ConditionKind::And(left, right) => {
            evaluate_kind_for_set(left, world, set_index, context)
                && evaluate_kind_for_set(right, world, set_index, context)
        }
        ConditionKind::Or(left, right) => {
            evaluate_kind_for_set(left, world, set_index, context)
                || evaluate_kind_for_set(right, world, set_index, context)
        }
        ConditionKind::Predicate(predicate) => predicate(world),
    }
}

fn advance_kind_cursors(
    kind: &ConditionKind,
    world: &World,
    system_index: usize,
    context: &mut RunContext,
) {
    match kind {
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
        ConditionKind::And(left, right) | ConditionKind::Or(left, right) => {
            advance_kind_cursors(left, world, system_index, context);
            advance_kind_cursors(right, world, system_index, context);
        }
        _ => {}
    }
}

fn advance_kind_set_cursors(
    kind: &ConditionKind,
    world: &World,
    set_index: usize,
    context: &mut RunContext,
) {
    match kind {
        ConditionKind::ResourceAdded(type_id) => {
            if let Some(tick) = world.resource_added_tick_for(*type_id) {
                context.set_resource_added_cursor_for_set(set_index, *type_id, tick);
            }
        }
        ConditionKind::ResourceChanged(type_id) => {
            if let Some(tick) = world.resource_changed_tick_for(*type_id) {
                context.set_resource_changed_cursor_for_set(set_index, *type_id, tick);
            }
        }
        ConditionKind::StateChanged(type_id) => {
            if let Some(tick) = world.state_transition_tick_for(*type_id) {
                context.set_state_transition_cursor_for_set(set_index, *type_id, tick);
            }
        }
        ConditionKind::And(left, right) | ConditionKind::Or(left, right) => {
            advance_kind_set_cursors(left, world, set_index, context);
            advance_kind_set_cursors(right, world, set_index, context);
        }
        _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::RunContext;
    use crate::time::ChangeTick;
    use crate::world::WorldBuilder;

    #[derive(Clone)]
    struct Score(#[allow(dead_code)] i32);

    #[test]
    fn resource_added_and_changed_advance_system_cursors() {
        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        let mut context = RunContext::new();

        assert!(!Condition::resource_added::<Score>().evaluate(&world, 0, &context));
        world.insert_resource(Score(1)).expect("insert");
        assert!(Condition::resource_added::<Score>().evaluate(&world, 0, &context));
        Condition::resource_added::<Score>().advance_cursors(&world, 0, &mut context);
        assert!(!Condition::resource_added::<Score>().evaluate(&world, 0, &context));

        world.insert_resource(Score(2)).expect("replace");
        assert!(Condition::resource_changed::<Score>().evaluate(&world, 1, &context));
        Condition::resource_changed::<Score>().advance_cursors(&world, 1, &mut context);
        assert!(!Condition::resource_changed::<Score>().evaluate(&world, 1, &context));
    }

    #[test]
    fn resource_added_evaluate_for_set_advances_set_cursors() {
        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        let mut context = RunContext::with_set_capacity(1);

        world.insert_resource(Score(1)).expect("insert");
        let condition = Condition::resource_added::<Score>();
        assert!(condition.evaluate_for_set(&world, 0, &context));
        condition.advance_set_cursors(&world, 0, &mut context);
        assert!(!condition.evaluate_for_set(&world, 0, &context));
    }

    #[test]
    fn and_or_combinators_delegate_cursor_advance() {
        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        let mut world = builder.build().expect("build");
        let mut context = RunContext::new();
        world.insert_resource(Score(1)).expect("insert");

        let condition =
            Condition::resource_added::<Score>().and(Condition::resource_changed::<Score>());
        assert!(condition.evaluate(&world, 0, &context));
        condition.advance_cursors(&world, 0, &mut context);

        world.insert_resource(Score(2)).expect("change");
        let or_condition = Condition::never().or(Condition::resource_changed::<Score>());
        assert!(or_condition.evaluate(&world, 1, &context));
        or_condition.advance_set_cursors(&world, 1, &mut context);
        let tick = world
            .resource_changed_tick_for(core::any::TypeId::of::<Score>())
            .expect("tick");
        assert_eq!(
            context.resource_changed_cursor_for_set(1, core::any::TypeId::of::<Score>()),
            tick
        );
    }

    #[test]
    fn state_changed_condition_tracks_transitions() {
        use crate::state::State;

        let mut builder = WorldBuilder::new();
        builder.register_state::<u8>();
        let mut world = builder.build().expect("build");
        let mut context = RunContext::new();

        world.insert_resource(State::new(1u8)).expect("state");
        world
            .resource_mut::<State<u8>>()
            .expect("mut")
            .expect("present")
            .request(2)
            .expect("request");
        let tick = world.issue_change_tick_for_state().expect("tick");
        world
            .resource_mut::<State<u8>>()
            .expect("mut")
            .expect("present")
            .apply_pending(tick);

        let condition = Condition::state_changed::<u8>();
        assert!(condition.evaluate(&world, 0, &context));
        condition.advance_cursors(&world, 0, &mut context);
        assert!(!condition.evaluate(&world, 0, &context));
    }

    #[test]
    fn resource_tick_advanced_helper() {
        let tick = ChangeTick::from_raw(5);
        assert!(!resource_tick_advanced(Some(ChangeTick::from_raw(4)), tick));
        assert!(resource_tick_advanced(Some(ChangeTick::from_raw(6)), tick));
        assert!(!resource_tick_advanced(None, tick));
        assert!(!state_tick_advanced(Some(ChangeTick::from_raw(4)), tick));
        assert!(state_tick_advanced(Some(ChangeTick::from_raw(6)), tick));
    }

    #[test]
    fn evaluate_for_set_covers_exists_changed_state_and_combinators() {
        use crate::state::State;

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        builder.register_state::<u8>();
        let mut world = builder.build().expect("build");
        let mut context = RunContext::with_set_capacity(2);

        assert!(!Condition::resource_exists::<Score>().evaluate_for_set(&world, 0, &context));
        assert!(!Condition::never().evaluate_for_set(&world, 0, &context));
        assert!(Condition::always().evaluate_for_set(&world, 0, &context));

        world.insert_resource(Score(1)).expect("insert");
        assert!(Condition::resource_exists::<Score>().evaluate_for_set(&world, 0, &context));

        world.insert_resource(Score(2)).expect("change");
        let changed = Condition::resource_changed::<Score>();
        assert!(changed.evaluate_for_set(&world, 0, &context));
        changed.advance_set_cursors(&world, 0, &mut context);
        assert!(!changed.evaluate_for_set(&world, 0, &context));

        world.insert_resource(State::new(1u8)).expect("state");
        world
            .resource_mut::<State<u8>>()
            .expect("mut")
            .expect("present")
            .request(2)
            .expect("request");
        let tick = world.issue_change_tick_for_state().expect("tick");
        world
            .resource_mut::<State<u8>>()
            .expect("mut")
            .expect("present")
            .apply_pending(tick);
        let state_changed = Condition::state_changed::<u8>();
        assert!(state_changed.evaluate_for_set(&world, 1, &context));
        state_changed.advance_set_cursors(&world, 1, &mut context);
        assert!(!state_changed.evaluate_for_set(&world, 1, &context));

        let and = Condition::always().and(Condition::resource_exists::<Score>());
        assert!(and.evaluate_for_set(&world, 0, &context));
        let or = Condition::never().or(Condition::resource_exists::<Score>());
        assert!(or.evaluate_for_set(&world, 0, &context));

        let predicate = Condition::in_state(2u8);
        assert!(predicate.evaluate_for_set(&world, 0, &context));

        let and = Condition::resource_added::<Score>().and(Condition::resource_changed::<Score>());
        and.advance_cursors(&world, 0, &mut RunContext::new());
        and.advance_set_cursors(&world, 0, &mut context);
    }
}
