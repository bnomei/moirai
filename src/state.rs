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
}
