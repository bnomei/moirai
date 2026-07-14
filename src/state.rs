//! Explicit host state machine backed by a [`crate::world::World`] resource.
//!
//! [`State`] queues at most one pending transition per frame. [`apply`] commits pending values on a
//! dedicated schedule boundary; [`on_exit`], [`on_transition`], and [`on_enter`] observe the ordered
//! transition lifecycle through [`crate::schedule::Condition`] gates.

use crate::time::ChangeTick;

/// Failure to queue a state transition on [`State`].
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    /// A different pending target is already queued.
    ConflictingTransition,
}

/// Host-owned state resource with explicit current, previous, and pending boundaries.
pub struct State<S: Eq + 'static> {
    current: S,
    previous: Option<S>,
    pending: Option<S>,
    transition_tick: Option<ChangeTick>,
}

impl<S: Eq + 'static> State<S> {
    /// Creates state with `initial` as [`Self::current`] and no pending transition.
    pub fn new(initial: S) -> Self {
        Self {
            current: initial,
            previous: None,
            pending: None,
            transition_tick: None,
        }
    }

    /// Active state value after the last committed transition.
    pub fn current(&self) -> &S {
        &self.current
    }

    /// Outgoing value from the most recent committed transition, if any.
    pub fn previous(&self) -> Option<&S> {
        self.previous.as_ref()
    }

    /// Requested next value awaiting [`apply`], if any.
    pub fn pending(&self) -> Option<&S> {
        self.pending.as_ref()
    }

    /// [`ChangeTick`] recorded when the last transition committed.
    pub fn transition_tick(&self) -> Option<ChangeTick> {
        self.transition_tick
    }

    /// Queues `next` for commit by [`apply`]; idempotent when already current or pending.
    pub fn request(&mut self, next: S) -> Result<(), StateError> {
        if let Some(pending) = &self.pending {
            if *pending == next {
                return Ok(());
            }
            return Err(StateError::ConflictingTransition);
        }
        if self.current == next {
            return Ok(());
        }
        self.pending = Some(next);
        Ok(())
    }

    pub(crate) fn apply_pending(&mut self, tick: ChangeTick) {
        let Some(next) = self.pending.take() else {
            return;
        };
        self.previous = Some(core::mem::replace(&mut self.current, next));
        self.transition_tick = Some(tick);
    }
}

/// Installs an explicit state-transition system for `State<S>`.
pub fn apply<S: Eq + 'static>(
    name: impl Into<alloc::string::String>,
    stage_label: impl Into<alloc::string::String>,
) -> crate::schedule::System {
    let label = name.into();
    crate::schedule::System::try_new(
        label,
        stage_label,
        move |world: &mut crate::world::World, _dt| {
            let tick = world
                .issue_change_tick_for_state()
                .map_err(|error| alloc::format!("{error:?}"))?;
            let state = world
                .resource_mut::<State<S>>()
                .map_err(|error| alloc::format!("{error:?}"))?
                .expect("required state resource remains present while the schedule lease is live");
            state.apply_pending(tick);
            Ok(())
        },
    )
    .requires_resource::<State<S>>()
}

/// Creates an exit hook that runs after a transition request and before
/// [`apply`] commits it. The hook observes the outgoing [`State::current`]
/// and requested [`State::pending`] values.
pub fn on_exit<S: Eq + 'static>(
    name: impl Into<alloc::string::String>,
    stage_label: impl Into<alloc::string::String>,
    body: impl FnMut(&mut crate::world::World, f32) + 'static,
) -> crate::schedule::System {
    crate::schedule::System::new(name, stage_label, body)
        .run_if(crate::schedule::Condition::state_pending::<S>())
        .requires_resource::<State<S>>()
}

/// Creates a post-apply transition hook. It observes [`State::previous`] and
/// [`State::current`] after an ordered [`apply`] system runs.
pub fn on_transition<S: Eq + 'static>(
    name: impl Into<alloc::string::String>,
    stage_label: impl Into<alloc::string::String>,
    body: impl FnMut(&mut crate::world::World, f32) + 'static,
) -> crate::schedule::System {
    crate::schedule::System::new(name, stage_label, body)
        .run_if(crate::schedule::Condition::state_changed::<S>())
        .requires_resource::<State<S>>()
}

/// Creates a post-apply enter hook. It has the same transition boundary as
/// [`on_transition`] and is named for host lifecycle readability.
pub fn on_enter<S: Eq + 'static>(
    name: impl Into<alloc::string::String>,
    stage_label: impl Into<alloc::string::String>,
    body: impl FnMut(&mut crate::world::World, f32) + 'static,
) -> crate::schedule::System {
    on_transition::<S>(name, stage_label, body)
}

#[cfg(feature = "std")]
impl core::fmt::Display for StateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ConflictingTransition => f.write_str("conflicting state transition request"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::ChangeTick;
    #[cfg(feature = "std")]
    use alloc::string::ToString;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Phase {
        A,
        B,
        C,
    }

    #[test]
    fn state_request_idempotent_and_conflicting() {
        let mut state = State::new(Phase::A);
        state.request(Phase::B).expect("first");
        state.request(Phase::B).expect("repeat");
        assert!(matches!(
            state.request(Phase::C),
            Err(StateError::ConflictingTransition)
        ));
        assert_eq!(state.pending(), Some(&Phase::B));
        #[cfg(feature = "std")]
        assert_eq!(
            StateError::ConflictingTransition.to_string(),
            "conflicting state transition request"
        );
    }

    #[test]
    fn apply_system_wiring_requires_state_and_runs_transitions() {
        use crate::app::AppBuilder;
        use crate::schedule::{stage, BuildError};

        #[derive(Clone, Debug, Eq, PartialEq)]
        enum Menu {
            Open,
            Closed,
        }

        let mut missing = AppBuilder::new();
        missing
            .add_system(apply::<Menu>("apply", stage::UPDATE))
            .expect("system");
        assert!(matches!(
            missing.build(),
            Err(BuildError::MissingRequiredResource { .. })
        ));

        let mut builder = AppBuilder::new();
        builder.insert_state(Menu::Open);
        builder
            .add_system(apply::<Menu>("apply", stage::UPDATE))
            .expect("system");
        let mut app = builder.build().expect("app");
        app.world_mut()
            .resource_mut::<State<Menu>>()
            .expect("state access")
            .expect("state resource")
            .request(Menu::Closed)
            .expect("request");
        app.update(0.0).expect("update");
        let state = app
            .world()
            .resource::<State<Menu>>()
            .expect("state access")
            .expect("state resource");
        assert_eq!(state.current(), &Menu::Closed);
        assert_eq!(state.previous(), Some(&Menu::Open));
        assert!(state.pending().is_none());
        assert!(state.transition_tick().is_some());
    }

    #[test]
    fn apply_pending_moves_current_and_records_tick() {
        let mut state = State::new(Phase::A);
        state.request(Phase::B).expect("request");
        let tick = ChangeTick::from_raw(9);
        state.apply_pending(tick);
        assert_eq!(state.current(), &Phase::B);
        assert_eq!(state.previous(), Some(&Phase::A));
        assert_eq!(state.transition_tick(), Some(tick));
        state.apply_pending(tick);
        assert_eq!(state.current(), &Phase::B);
    }

    #[test]
    fn lifecycle_helpers_observe_the_ordered_transition_boundary() {
        use alloc::rc::Rc;
        use core::cell::RefCell;

        use crate::app::AppBuilder;
        use crate::schedule::stage;

        #[derive(Debug, Eq, PartialEq)]
        enum Mode {
            Menu,
            Playing,
        }

        let order = Rc::new(RefCell::new(alloc::vec::Vec::new()));
        let exit_order = Rc::clone(&order);
        let transition_order = Rc::clone(&order);
        let enter_order = Rc::clone(&order);
        let mut builder = AppBuilder::new();
        // State lifecycle helpers also support the ordinary resource builder
        // path; hosts need not use the `insert_state` convenience.
        builder.insert_resource(State::new(Mode::Menu));
        builder
            .add_system(on_exit::<Mode>("exit", stage::UPDATE, move |world, _| {
                let state = world
                    .resource::<State<Mode>>()
                    .expect("state")
                    .expect("present");
                assert_eq!(state.current(), &Mode::Menu);
                assert_eq!(state.pending(), Some(&Mode::Playing));
                exit_order.borrow_mut().push("exit");
            }))
            .expect("exit");
        builder
            .add_system(apply::<Mode>("apply", stage::UPDATE).after("exit"))
            .expect("apply");
        builder
            .add_system(
                on_transition::<Mode>("transition", stage::UPDATE, move |world, _| {
                    let state = world
                        .resource::<State<Mode>>()
                        .expect("state")
                        .expect("present");
                    assert_eq!(state.previous(), Some(&Mode::Menu));
                    assert_eq!(state.current(), &Mode::Playing);
                    transition_order.borrow_mut().push("transition");
                })
                .after("apply"),
            )
            .expect("transition");
        builder
            .add_system(
                on_enter::<Mode>("enter", stage::UPDATE, move |_, _| {
                    enter_order.borrow_mut().push("enter");
                })
                .after("transition"),
            )
            .expect("enter");
        let mut app = builder.build().expect("app");
        app.world_mut()
            .resource_mut::<State<Mode>>()
            .expect("state")
            .expect("present")
            .request(Mode::Playing)
            .expect("request");

        app.update(0.0).expect("update");
        assert_eq!(&*order.borrow(), &["exit", "transition", "enter"]);
    }
}
