use alloc::boxed::Box;
use alloc::string::String;
use core::time::Duration;

use crate::diagnostics::{DiagnosticEvent, Observer};
use crate::operation::StageOperation;
use crate::schedule::stage;
pub use crate::schedule::BuildError;
use crate::schedule::RunOutcome;
use crate::schedule::{RunContext, Schedule, ScheduleBuilder, ScheduleError, System, SystemId};
use crate::time::FixedConfig;
use crate::world::{World, WorldBuilder};

/// Terminal execution record retained after a fault.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppFault {
    pub stage: Option<String>,
    pub system: Option<String>,
    pub detail: Option<String>,
}

/// Recoverable and terminal App lifecycle failures.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppError {
    InvalidDelta,
    PendingIdleCommands,
    WorldMutationPoisoned,
    TerminalFault,
    WorldTickExhausted,
    FixedStepExhausted,
    Fault(AppFault),
}

/// Top-level ECS application owning `World` and `Schedule`.
pub struct App {
    world: World,
    schedule: Schedule,
    run_context: RunContext,
    faulted: bool,
    fault: Option<AppFault>,
    observer: Option<Box<dyn Observer>>,
}

/// Checked application construction.
pub struct AppBuilder {
    world_builder: WorldBuilder,
    schedule_builder: ScheduleBuilder,
    observer: Option<Box<dyn Observer>>,
}

impl App {
    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn from_parts(world: World, schedule: Schedule) -> Result<Self, BuildError> {
        if world.has_pending_commands() {
            return Err(BuildError::PendingCommands);
        }
        if !world.run_guard_is_idle() {
            return Err(BuildError::WorldRunning);
        }
        if world.is_mutation_poisoned() {
            return Err(BuildError::WorldMutationPoisoned);
        }
        if !world.validate_execution_lease(schedule.execution_lease()) {
            return Err(BuildError::LeaseMismatch);
        }
        Ok(Self {
            world,
            schedule,
            run_context: RunContext::new(),
            faulted: false,
            fault: None,
            observer: None,
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn is_faulted(&self) -> bool {
        self.faulted
    }

    pub fn set_system_enabled(
        &mut self,
        id: &SystemId,
        enabled: bool,
    ) -> Result<(), ScheduleError> {
        self.schedule.set_system_enabled(id, enabled)
    }

    pub fn update(&mut self, delta_seconds: f32) -> Result<(), AppError> {
        self.update_with(delta_seconds, |_| ())
    }

    pub fn update_with<R>(
        &mut self,
        delta_seconds: f32,
        observe: impl FnOnce(&World) -> R,
    ) -> Result<R, AppError> {
        self.ensure_ready()?;
        validate_delta(delta_seconds)?;
        emit(
            &mut self.observer,
            DiagnosticEvent::UpdateStart { delta_seconds },
        );

        let frame_delta = duration_from_seconds(delta_seconds)?;
        let fixed_config = self.schedule.fixed_config().copied();
        let substeps = if let Some(config) = fixed_config {
            let peek_substeps = self
                .schedule
                .fixed_accumulator()
                .peek_substeps(frame_delta, &config)
                .0;
            self.world
                .preflight_world_tick()
                .map_err(|_| self.fault_tick_exhaustion())?;
            self.schedule
                .fixed_accumulator()
                .preflight_substeps(peek_substeps)
                .map_err(|_| self.fault_fixed_exhaustion())?;
            let (substeps, debt) = self
                .schedule
                .fixed_accumulator_mut()
                .plan_substeps(frame_delta, &config);
            if let Some(debt) = debt {
                emit(
                    &mut self.observer,
                    DiagnosticEvent::FixedDebtDropped { steps: debt.steps },
                );
            }
            substeps
        } else {
            self.world
                .preflight_world_tick()
                .map_err(|_| self.fault_tick_exhaustion())?;
            0
        };

        self.world
            .advance_world_tick()
            .map_err(|_| self.fault_tick_exhaustion())?;

        self.run_context.fixed_step = None;
        let update_stages = self.schedule.update_stage_indices();
        for stage_index in update_stages {
            let stage_label = self.schedule.stage_label(stage_index);
            if stage_label == stage::FIXED_UPDATE {
                if let Some(config) = fixed_config {
                    for _ in 0..substeps {
                        let mut step = self.schedule.fixed_accumulator_mut().next_step(&config);
                        step.advance_index()
                            .map_err(|_| self.fault_fixed_exhaustion())?;
                        self.world.set_fixed_step(Some(step));
                        self.run_context.fixed_step = Some(step);
                        let result = self.run_stage(stage_index, seconds_from_duration(step.delta));
                        self.world.set_fixed_step(None);
                        self.run_context.fixed_step = None;
                        result?;
                    }
                }
                continue;
            }
            self.run_stage(stage_index, delta_seconds)?;
        }

        self.run_final_flush()?;
        let observed = observe(&self.world);
        self.schedule
            .clear_frame_events(&mut self.world, StageOperation::Update);
        emit(&mut self.observer, DiagnosticEvent::UpdateFinish);
        Ok(observed)
    }

    pub fn render(&mut self, delta_seconds: f32) -> Result<(), AppError> {
        self.render_with(delta_seconds, |_| ())
    }

    pub fn render_with<R>(
        &mut self,
        delta_seconds: f32,
        observe: impl FnOnce(&World) -> R,
    ) -> Result<R, AppError> {
        self.ensure_ready()?;
        validate_delta(delta_seconds)?;
        emit(
            &mut self.observer,
            DiagnosticEvent::RenderStart { delta_seconds },
        );
        self.run_context.fixed_step = None;
        let run_result = {
            let schedule = &mut self.schedule;
            let world = &mut self.world;
            let observer = &mut self.observer;
            let context = &mut self.run_context;
            catch_schedule_panic(|| {
                schedule.run_operation(
                    world,
                    StageOperation::Render,
                    context,
                    delta_seconds,
                    observer,
                )
            })
        };
        handle_guarded_run(self, run_result)?;
        let observed = observe(&self.world);
        self.schedule
            .clear_frame_events(&mut self.world, StageOperation::Render);
        emit(&mut self.observer, DiagnosticEvent::RenderFinish);
        Ok(observed)
    }

    fn ensure_ready(&self) -> Result<(), AppError> {
        if self.faulted {
            return Err(AppError::TerminalFault);
        }
        if self.world.is_mutation_poisoned() {
            return Err(AppError::WorldMutationPoisoned);
        }
        if self.world.has_pending_commands() {
            return Err(AppError::PendingIdleCommands);
        }
        Ok(())
    }

    fn run_stage(&mut self, stage_index: usize, dt: f32) -> Result<(), AppError> {
        let run_result = {
            let schedule = &mut self.schedule;
            let world = &mut self.world;
            let observer = &mut self.observer;
            let context = &mut self.run_context;
            catch_schedule_panic(|| schedule.run_stage(world, stage_index, context, dt, observer))
        };
        handle_guarded_run(self, run_result)
    }

    fn fault_tick_exhaustion(&mut self) -> AppError {
        self.record_exhaustion_fault("world tick exhausted");
        AppError::WorldTickExhausted
    }

    fn fault_fixed_exhaustion(&mut self) -> AppError {
        self.record_exhaustion_fault("fixed step exhausted");
        AppError::FixedStepExhausted
    }

    fn record_exhaustion_fault(&mut self, detail: &str) {
        self.faulted = true;
        self.fault = Some(AppFault {
            stage: None,
            system: None,
            detail: Some(String::from(detail)),
        });
        self.world.set_fixed_step(None);
        let _ = self.world.discard_commands();
        self.world.end_run();
        emit(
            &mut self.observer,
            DiagnosticEvent::Fault {
                stage: None,
                system: None,
            },
        );
    }

    fn run_final_flush(&mut self) -> Result<(), AppError> {
        let run_result = {
            let schedule = &mut self.schedule;
            let world = &mut self.world;
            let observer = &mut self.observer;
            catch_schedule_panic(|| schedule.run_final_update_flush(world, observer))
        };
        handle_guarded_run(self, run_result)
    }

    fn fault_from(&mut self, outcome: RunOutcome) -> AppError {
        self.record_fault(&outcome);
        AppError::Fault(AppFault {
            stage: outcome.fault_stage,
            system: outcome.fault_system,
            detail: outcome.fault_detail,
        })
    }

    fn record_fault(&mut self, outcome: &RunOutcome) {
        self.faulted = true;
        self.fault = Some(AppFault {
            stage: outcome.fault_stage.clone(),
            system: outcome.fault_system.clone(),
            detail: outcome.fault_detail.clone(),
        });
        let _ = self.world.discard_commands();
        self.world.set_fixed_step(None);
        self.world.end_run();
        emit(
            &mut self.observer,
            DiagnosticEvent::Fault {
                stage: outcome.fault_stage.as_deref(),
                system: outcome.fault_system.as_deref(),
            },
        );
    }

    #[cfg(feature = "std")]
    fn record_panic_fault(&mut self) {
        self.faulted = true;
        self.fault = Some(AppFault {
            stage: None,
            system: None,
            detail: Some(String::from("panic during execution")),
        });
        let _ = self.world.discard_commands();
        emit(
            &mut self.observer,
            DiagnosticEvent::Fault {
                stage: None,
                system: None,
            },
        );
    }
}

impl AppBuilder {
    pub fn new() -> AppBuilder {
        Self {
            world_builder: WorldBuilder::new(),
            schedule_builder: ScheduleBuilder::standard(),
            observer: None,
        }
    }

    pub fn world_builder(&mut self) -> &mut WorldBuilder {
        &mut self.world_builder
    }

    pub fn schedule_builder(&mut self) -> &mut ScheduleBuilder {
        &mut self.schedule_builder
    }

    pub fn add_system(&mut self, system: System) -> Result<&mut Self, BuildError> {
        self.schedule_builder.add_system(system)?;
        Ok(self)
    }

    pub fn fixed(&mut self, config: FixedConfig) -> &mut Self {
        self.schedule_builder.fixed(config);
        self
    }

    pub fn observer(&mut self, observer: impl Observer + 'static) -> &mut Self {
        self.observer = Some(Box::new(observer));
        self
    }

    pub fn build(self) -> Result<App, BuildError> {
        let mut world = self.world_builder.build()?;
        let schedule = self.schedule_builder.build(&mut world)?;
        let mut app = App::from_parts(world, schedule)?;
        app.observer = self.observer;
        Ok(app)
    }

    pub fn register_set(
        &mut self,
        set: crate::schedule::SystemSet,
    ) -> Result<&mut Self, BuildError> {
        self.schedule_builder.register_set(set)?;
        Ok(self)
    }

    pub fn set_run_if(
        &mut self,
        set: &crate::schedule::SystemSet,
        condition: crate::schedule::Condition,
    ) -> Result<&mut Self, BuildError> {
        self.schedule_builder.set_run_if(set, condition)?;
        Ok(self)
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_delta(delta_seconds: f32) -> Result<(), AppError> {
    if delta_seconds.is_sign_negative() || delta_seconds.is_nan() || delta_seconds.is_infinite() {
        return Err(AppError::InvalidDelta);
    }
    Ok(())
}

fn duration_from_seconds(delta_seconds: f32) -> Result<Duration, AppError> {
    Duration::try_from_secs_f32(delta_seconds).map_err(|_| AppError::InvalidDelta)
}

fn seconds_from_duration(duration: Duration) -> f32 {
    duration.as_secs_f32()
}

fn emit(observer: &mut Option<Box<dyn Observer>>, event: DiagnosticEvent<'_>) {
    if let Some(observer) = observer.as_mut() {
        observer.observe(event);
    }
}

enum GuardedRun<T> {
    Completed(T),
    #[cfg(feature = "std")]
    Panicked(alloc::boxed::Box<dyn core::any::Any + Send>),
}

fn catch_schedule_panic<R>(f: impl FnOnce() -> R) -> GuardedRun<R> {
    #[cfg(feature = "std")]
    {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
            Ok(result) => GuardedRun::Completed(result),
            Err(payload) => GuardedRun::Panicked(payload),
        }
    }
    #[cfg(not(feature = "std"))]
    {
        GuardedRun::Completed(f())
    }
}

fn handle_guarded_run<T>(
    app: &mut App,
    run: GuardedRun<Result<T, RunOutcome>>,
) -> Result<T, AppError> {
    match run {
        GuardedRun::Completed(Ok(value)) => Ok(value),
        GuardedRun::Completed(Err(outcome)) => Err(app.fault_from(outcome)),
        #[cfg(feature = "std")]
        GuardedRun::Panicked(payload) => {
            app.world.end_run();
            app.record_panic_fault();
            std::panic::resume_unwind(payload);
        }
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for AppError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidDelta => f.write_str("invalid delta"),
            Self::PendingIdleCommands => f.write_str("pending idle commands"),
            Self::WorldMutationPoisoned => f.write_str("world mutation poisoned"),
            Self::TerminalFault => f.write_str("terminal app fault"),
            Self::WorldTickExhausted => f.write_str("world tick exhausted"),
            Self::FixedStepExhausted => f.write_str("fixed step exhausted"),
            Self::Fault(_) => f.write_str("app execution fault"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AppError {}
