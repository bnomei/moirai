use alloc::string::String;

use crate::time::ChangeTick;

/// Failure to queue a state transition.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    ConflictingTransition,
}

/// Generic host-owned state resource with explicit transition boundaries.
pub struct State<S: Eq + 'static> {
    current: S,
    previous: Option<S>,
    pending: Option<S>,
    transition_tick: Option<ChangeTick>,
}

impl<S: Eq + 'static> State<S> {
    pub fn new(initial: S) -> Self {
        Self {
            current: initial,
            previous: None,
            pending: None,
            transition_tick: None,
        }
    }

    pub fn current(&self) -> &S {
        &self.current
    }

    pub fn previous(&self) -> Option<&S> {
        self.previous.as_ref()
    }

    pub fn pending(&self) -> Option<&S> {
        self.pending.as_ref()
    }

    pub fn transition_tick(&self) -> Option<ChangeTick> {
        self.transition_tick
    }

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
            let Some(state) = world
                .resource_mut::<State<S>>()
                .map_err(|error| alloc::format!("{error:?}"))?
            else {
                return Err(String::from("state resource missing"));
            };
            state.apply_pending(tick);
            Ok(())
        },
    )
    .requires_resource::<State<S>>()
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
    }

    #[test]
    fn apply_system_errors_when_state_resource_missing() {
        use crate::schedule::stage;
        use crate::world::WorldBuilder;

        #[derive(Clone, Eq, PartialEq)]
        enum Menu {
            #[allow(dead_code)]
            Open,
        }

        let mut builder = WorldBuilder::new();
        builder.register_resource::<State<Menu>>();
        let mut world = builder.build().expect("world");
        let mut system = apply::<Menu>("apply", stage::UPDATE);
        world
            .begin_run(crate::operation::StageOperation::Update)
            .expect("begin");
        let err = (system.body)(&mut world, 0.0).expect_err("missing state");
        assert_eq!(err, "state resource missing");
        world.end_run();
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
}
