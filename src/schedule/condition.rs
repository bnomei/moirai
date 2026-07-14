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

/// Invalid fixed-step cadence configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionError {
    ZeroPeriod,
    PeriodNotPowerOfTwo { period: u64 },
    PhaseOutOfRange { period: u64, phase: u64 },
}

#[derive(Clone)]
enum ConditionKind {
    Always,
    Never,
    ResourceExists(TypeId),
    ResourceAdded(TypeId),
    ResourceChanged(TypeId),
    StateChanged(TypeId),
    FixedStepMod { mask: u64, phase: u64 },
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

    /// Runs on one phase of a power-of-two fixed-step cadence.
    ///
    /// The condition is false outside `FixedUpdate`. Fixed-step indices are
    /// zero-based, so `(8, 0)` includes the first fixed substep.
    pub fn fixed_step_mod(period: u64, phase: u64) -> Result<Self, ConditionError> {
        if period == 0 {
            return Err(ConditionError::ZeroPeriod);
        }
        if !period.is_power_of_two() {
            return Err(ConditionError::PeriodNotPowerOfTwo { period });
        }
        if phase >= period {
            return Err(ConditionError::PhaseOutOfRange { period, phase });
        }
        Ok(Self(ConditionKind::FixedStepMod {
            mask: period - 1,
            phase,
        }))
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
        ConditionKind::FixedStepMod { mask, phase } => context
            .fixed_step
            .is_some_and(|step| step.index & mask == *phase),
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
        ConditionKind::FixedStepMod { mask, phase } => context
            .fixed_step
            .is_some_and(|step| step.index & mask == *phase),
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

impl core::fmt::Display for ConditionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ZeroPeriod => f.write_str("fixed-step cadence period must be nonzero"),
            Self::PeriodNotPowerOfTwo { period } => {
                write!(
                    f,
                    "fixed-step cadence period {period} is not a power of two"
                )
            }
            Self::PhaseOutOfRange { period, phase } => write!(
                f,
                "fixed-step cadence phase {phase} is outside period {period}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConditionError {}

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
    use alloc::string::ToString;

    #[derive(Clone)]
    struct Score(#[allow(dead_code)] i32);

    #[test]
    fn fixed_step_mod_validates_binary_cadence() {
        assert_eq!(
            Condition::fixed_step_mod(0, 0).err(),
            Some(ConditionError::ZeroPeriod)
        );
        assert_eq!(
            Condition::fixed_step_mod(3, 0).err(),
            Some(ConditionError::PeriodNotPowerOfTwo { period: 3 })
        );
        assert_eq!(
            Condition::fixed_step_mod(4, 4).err(),
            Some(ConditionError::PhaseOutOfRange {
                period: 4,
                phase: 4,
            })
        );
        assert_eq!(
            ConditionError::ZeroPeriod.to_string(),
            "fixed-step cadence period must be nonzero"
        );
        assert!(ConditionError::PeriodNotPowerOfTwo { period: 3 }
            .to_string()
            .contains('3'));
        assert!(ConditionError::PhaseOutOfRange {
            period: 4,
            phase: 4
        }
        .to_string()
        .contains('4'));
    }

    #[test]
    fn fixed_step_mod_uses_zero_based_mask_and_is_false_outside_fixed_update() {
        let world = WorldBuilder::new().build().expect("world");
        let condition = Condition::fixed_step_mod(4, 0).expect("condition");
        let mut context = RunContext::new();
        assert!(!condition.evaluate(&world, 0, &context));

        for index in 0..8 {
            context.fixed_step = Some(crate::time::FixedStep {
                index,
                delta: core::time::Duration::from_millis(16),
            });
            assert_eq!(condition.evaluate(&world, 0, &context), index % 4 == 0);
            assert_eq!(
                condition.evaluate_for_set(&world, 0, &context),
                index % 4 == 0
            );
        }
    }

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

    #[test]
    fn advancing_absent_temporal_values_leaves_cursors_at_zero() {
        use crate::state::State;
        use core::any::TypeId;

        let mut builder = WorldBuilder::new();
        builder.register_resource::<Score>();
        builder.register_state::<u8>();
        let world = builder.build().expect("build");
        let mut context = RunContext::with_set_capacity(1);

        let score = TypeId::of::<Score>();
        let state = TypeId::of::<State<u8>>();
        for condition in [
            Condition::resource_added::<Score>(),
            Condition::resource_changed::<Score>(),
            Condition::state_changed::<u8>(),
        ] {
            condition.advance_cursors(&world, 0, &mut context);
            condition.advance_set_cursors(&world, 0, &mut context);
        }

        assert_eq!(context.resource_added_cursor(0, score), ChangeTick::ZERO);
        assert_eq!(context.resource_changed_cursor(0, score), ChangeTick::ZERO);
        assert_eq!(context.state_transition_cursor(0, state), ChangeTick::ZERO);
        assert_eq!(
            context.resource_added_cursor_for_set(0, score),
            ChangeTick::ZERO
        );
        assert_eq!(
            context.resource_changed_cursor_for_set(0, score),
            ChangeTick::ZERO
        );
        assert_eq!(
            context.state_transition_cursor_for_set(0, state),
            ChangeTick::ZERO
        );
    }
}
