use alloc::string::String;

use crate::time::ChangeTick;
use crate::world::WorldError;

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

    pub fn request(&mut self, next: S) -> Result<(), WorldError> {
        if let Some(pending) = &self.pending {
            if *pending == next {
                return Ok(());
            }
            return Err(WorldError::WrongStorageKind {
                name: String::from("conflicting state transition request"),
            });
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
}
