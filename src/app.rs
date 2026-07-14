use alloc::boxed::Box;
use alloc::string::String;
use core::time::Duration;

use crate::diagnostics::{DiagnosticEvent, Observer};
use crate::operation::StageOperation;
use crate::schedule::stage;
pub use crate::schedule::BuildError;
use crate::schedule::RunOutcome;
use crate::schedule::{
    FlushMode, RunContext, Schedule, ScheduleBuilder, ScheduleError, System, SystemId, SystemSet,
};
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
        let set_count = schedule.set_count();
        Ok(Self {
            world,
            schedule,
            run_context: RunContext::with_set_capacity(set_count),
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

    /// Returns the first terminal fault retained by this application.
    pub fn fault(&self) -> Option<&AppFault> {
        self.fault.as_ref()
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
        let update_stage_count = self.schedule.update_stage_indices().len();
        for stage_order_index in 0..update_stage_count {
            let stage_index = self.schedule.update_stage_indices()[stage_order_index];
            let stage_label = self.schedule.stage_label_at(stage_index);
            if stage_label == stage::FIXED_UPDATE {
                if let Some(config) = fixed_config {
                    for _ in 0..substeps {
                        let step = self.schedule.fixed_accumulator_mut().next_step(&config);
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
        let observed = self.observe_with_cleanup(StageOperation::Update, observe);
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
            let faulted = &mut self.faulted;
            let fault = &mut self.fault;
            catch_schedule_panic(|| {
                with_terminal_unwind_cleanup(
                    world,
                    context,
                    faulted,
                    fault,
                    StageOperation::Render,
                    |world, context| {
                        schedule.run_operation(
                            world,
                            StageOperation::Render,
                            context,
                            delta_seconds,
                            observer,
                        )
                    },
                )
            })
        };
        handle_guarded_run(self, run_result)?;
        let observed = self.observe_with_cleanup(StageOperation::Render, observe);
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
            let faulted = &mut self.faulted;
            let fault = &mut self.fault;
            catch_schedule_panic(|| {
                with_terminal_unwind_cleanup(
                    world,
                    context,
                    faulted,
                    fault,
                    StageOperation::Update,
                    |world, context| schedule.run_stage(world, stage_index, context, dt, observer),
                )
            })
        };
        handle_guarded_run(self, run_result)
    }

    fn observe_with_cleanup<R>(
        &mut self,
        operation: StageOperation,
        observe: impl FnOnce(&World) -> R,
    ) -> R {
        let run_result = {
            let world = &mut self.world;
            let context = &mut self.run_context;
            let faulted = &mut self.faulted;
            let fault = &mut self.fault;
            catch_schedule_panic(|| {
                with_terminal_unwind_cleanup(
                    world,
                    context,
                    faulted,
                    fault,
                    operation,
                    |world, _context| observe(world),
                )
            })
        };
        handle_guarded_value(self, run_result)
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
        if self.fault.is_none() {
            self.fault = Some(AppFault {
                stage: None,
                system: None,
                detail: Some(String::from(detail)),
            });
        }
        self.world.set_fixed_step(None);
        self.run_context.fixed_step = None;
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
            let context = &mut self.run_context;
            let faulted = &mut self.faulted;
            let fault = &mut self.fault;
            catch_schedule_panic(|| {
                with_terminal_unwind_cleanup(
                    world,
                    context,
                    faulted,
                    fault,
                    StageOperation::Update,
                    |world, _context| schedule.run_final_update_flush(world, observer),
                )
            })
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
        if self.fault.is_none() {
            self.fault = Some(AppFault {
                stage: outcome.fault_stage.clone(),
                system: outcome.fault_system.clone(),
                detail: outcome.fault_detail.clone(),
            });
        }
        let _ = self.world.discard_commands();
        self.world.set_fixed_step(None);
        self.run_context.fixed_step = None;
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
        if self.fault.is_none() {
            self.fault = Some(AppFault {
                stage: None,
                system: None,
                detail: Some(String::from("panic during execution")),
            });
        }
        let _ = self.world.discard_commands();
        self.world.set_fixed_step(None);
        self.run_context.fixed_step = None;
        self.world.end_run();
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

    /// Registers and seeds a resource before schedule validation.
    pub fn insert_resource<R: 'static>(&mut self, value: R) -> &mut Self {
        self.world_builder.insert_resource(value);
        self
    }

    /// Registers and seeds state before schedule validation.
    pub fn insert_state<S: Eq + 'static>(&mut self, initial: S) -> &mut Self {
        self.world_builder.insert_state(initial);
        self
    }

    pub fn fixed(&mut self, config: FixedConfig) -> &mut Self {
        self.schedule_builder.fixed(config);
        self
    }

    pub fn set_stage_flush_mode(
        &mut self,
        label: impl AsRef<str>,
        mode: FlushMode,
    ) -> Result<&mut Self, BuildError> {
        self.schedule_builder.set_stage_flush_mode(label, mode)?;
        Ok(self)
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

    pub fn order_set_before(
        &mut self,
        before: &SystemSet,
        after: &SystemSet,
    ) -> Result<&mut Self, BuildError> {
        self.schedule_builder.order_set_before(before, after)?;
        Ok(self)
    }

    pub fn order_set_after(
        &mut self,
        after: &SystemSet,
        before: &SystemSet,
    ) -> Result<&mut Self, BuildError> {
        self.schedule_builder.order_set_after(after, before)?;
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

struct TerminalUnwindGuard<'a> {
    world: &'a mut World,
    run_context: &'a mut RunContext,
    faulted: &'a mut bool,
    fault: &'a mut Option<AppFault>,
    operation: StageOperation,
    armed: bool,
}

impl Drop for TerminalUnwindGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        *self.faulted = true;
        if self.fault.is_none() {
            *self.fault = Some(AppFault {
                stage: None,
                system: None,
                detail: Some(String::from("panic during execution")),
            });
        }
        self.run_context.fixed_step = None;
        self.world.set_fixed_step(None);
        self.world.end_run();
        let _ = self.world.discard_commands();
        self.world.clear_frame_events(self.operation);
    }
}

fn with_terminal_unwind_cleanup<R>(
    world: &mut World,
    run_context: &mut RunContext,
    faulted: &mut bool,
    fault: &mut Option<AppFault>,
    operation: StageOperation,
    run: impl FnOnce(&mut World, &mut RunContext) -> R,
) -> R {
    let mut guard = TerminalUnwindGuard {
        world,
        run_context,
        faulted,
        fault,
        operation,
        armed: true,
    };
    let result = run(&mut *guard.world, &mut *guard.run_context);
    guard.armed = false;
    result
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

fn handle_guarded_value<T>(_app: &mut App, run: GuardedRun<T>) -> T {
    match run {
        GuardedRun::Completed(value) => value,
        #[cfg(feature = "std")]
        GuardedRun::Panicked(payload) => {
            _app.record_panic_fault();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::schedule::{stage, ScheduleBuilder, System};
    use crate::time::{ChangeTick, FixedConfig};
    use crate::world::WorldBuilder;
    use alloc::vec::Vec;
    use core::time::Duration;

    #[derive(Clone, Copy)]
    struct PoisonedComponent;

    fn poison_world(world: &mut World) {
        let entity = world.spawn().expect("entity");
        world.insert(entity, PoisonedComponent).expect("seed");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        world
            .insert(entity, PoisonedComponent)
            .expect("consume last tick");
        assert!(matches!(
            world.insert(entity, PoisonedComponent),
            Err(crate::world::WorldError::ChangeTickExhausted)
        ));
    }

    #[test]
    fn from_parts_rejects_poisoned_world() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<PoisonedComponent>(ComponentOptions::sparse())
            .expect("component");
        let mut world = builder.build().expect("world");
        let schedule = ScheduleBuilder::standard()
            .build(&mut world)
            .expect("schedule");
        poison_world(&mut world);

        assert!(matches!(
            App::from_parts(world, schedule),
            Err(BuildError::WorldMutationPoisoned)
        ));
    }

    #[test]
    fn update_rejects_poisoned_world() {
        let mut builder = AppBuilder::new();
        builder
            .world_builder()
            .register_component::<PoisonedComponent>(ComponentOptions::sparse())
            .expect("component");
        builder
            .add_system(System::new("noop", stage::UPDATE, |_world, _dt| {}))
            .expect("system");
        let mut app = builder.build().expect("app");
        poison_world(app.world_mut());

        assert!(matches!(
            app.update(1.0 / 60.0),
            Err(AppError::WorldMutationPoisoned)
        ));
    }

    #[test]
    fn world_tick_exhaustion_faults_app() {
        let mut app = AppBuilder::new().build().expect("app");
        app.world_mut().set_world_tick_for_test(u64::MAX);

        assert!(matches!(
            app.update(1.0 / 60.0),
            Err(AppError::WorldTickExhausted)
        ));
        assert!(app.is_faulted());
        assert_eq!(
            app.fault().and_then(|fault| fault.detail.as_deref()),
            Some("world tick exhausted")
        );
    }

    #[test]
    fn caught_tick_exhaustion_faults_before_next_system() {
        use core::sync::atomic::{AtomicU32, Ordering};

        static LATER_RUNS: AtomicU32 = AtomicU32::new(0);
        LATER_RUNS.store(0, Ordering::SeqCst);

        #[derive(Clone, Copy)]
        struct Counter;

        let mut builder = AppBuilder::new();
        builder.insert_resource(Counter);
        builder
            .add_system(System::new("poison", stage::UPDATE, |world, _dt| {
                let _ = world.resource_mut::<Counter>();
            }))
            .expect("poison system");
        builder
            .add_system(System::new("later", stage::UPDATE, |_world, _dt| {
                LATER_RUNS.fetch_add(1, Ordering::SeqCst);
            }))
            .expect("later system");
        let mut app = builder.build().expect("app");
        app.world_mut()
            .set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));

        assert!(matches!(app.update(0.0), Err(AppError::Fault(_))));
        assert_eq!(LATER_RUNS.load(Ordering::SeqCst), 0);
        assert_eq!(
            app.fault().and_then(|fault| fault.system.as_deref()),
            Some("poison")
        );
    }

    #[test]
    fn fixed_step_exhaustion_records_fault() {
        let fixed = FixedConfig::new(Duration::from_millis(16))
            .expect("fixed")
            .with_max_substeps(1)
            .expect("cap");
        let mut world = WorldBuilder::new().build().expect("world");
        let mut schedule_builder = ScheduleBuilder::standard();
        schedule_builder.fixed(fixed);
        schedule_builder
            .add_system(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {}))
            .expect("fixed");
        let mut schedule = schedule_builder.build(&mut world).expect("schedule");
        schedule
            .fixed_accumulator_mut()
            .set_next_index_for_test(u64::MAX);
        let mut app = App::from_parts(world, schedule).expect("app");
        assert!(matches!(app.update(1.0), Err(AppError::FixedStepExhausted)));
        assert!(app.is_faulted());
    }

    #[test]
    fn later_faults_preserve_the_first_terminal_fault() {
        let mut app = AppBuilder::default().build().expect("app");
        let first = AppFault {
            stage: Some(String::from("first-stage")),
            system: Some(String::from("first-system")),
            detail: Some(String::from("first-detail")),
        };
        app.fault = Some(first.clone());

        app.record_exhaustion_fault("later exhaustion");
        assert_eq!(app.fault(), Some(&first));

        app.record_fault(&RunOutcome {
            fault_stage: Some(String::from("later-stage")),
            fault_system: Some(String::from("later-system")),
            fault_detail: Some(String::from("later-detail")),
        });
        assert_eq!(app.fault(), Some(&first));
    }

    #[test]
    #[cfg(feature = "std")]
    fn unwind_cleanup_preserves_an_existing_fault() {
        let mut world = WorldBuilder::new().build().expect("world");
        let mut context = RunContext::new();
        let mut faulted = false;
        let first = AppFault {
            stage: Some(String::from("first-stage")),
            system: Some(String::from("first-system")),
            detail: Some(String::from("first-detail")),
        };
        let mut fault = Some(first.clone());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_terminal_unwind_cleanup(
                &mut world,
                &mut context,
                &mut faulted,
                &mut fault,
                StageOperation::Update,
                |_world, _context| panic!("test panic"),
            );
        }));

        assert!(result.is_err());
        assert!(faulted);
        assert_eq!(fault, Some(first));
        assert!(world.run_guard_is_idle());
    }

    #[test]
    #[cfg(feature = "std")]
    fn fixed_system_panic_clears_world_and_run_context_steps() {
        let fixed = FixedConfig::new(Duration::from_millis(16)).expect("fixed");
        let mut builder = AppBuilder::new();
        builder.fixed(fixed);
        builder
            .add_system(System::new("panic", stage::FIXED_UPDATE, |world, _dt| {
                assert!(world.fixed_step().is_some());
                world
                    .commands()
                    .expect("commands")
                    .spawn()
                    .expect("reserve");
                panic!("fixed panic");
            }))
            .expect("system");
        let mut app = builder.build().expect("app");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = app.update(0.016);
        }));

        assert!(result.is_err());
        assert!(app.world.fixed_step().is_none());
        assert!(app.run_context.fixed_step.is_none());
        assert!(app.world.run_guard_is_idle());
        assert!(!app.world.has_pending_commands());
    }

    #[cfg(feature = "std")]
    #[test]
    fn panic_fault_can_be_recorded_directly_without_prior_fault() {
        let mut app = AppBuilder::default().build().expect("app");
        app.record_panic_fault();
        assert!(app.is_faulted());
        assert_eq!(
            app.fault().and_then(|fault| fault.detail.as_deref()),
            Some("panic during execution")
        );
    }

    #[test]
    fn builder_set_order_after_delegates_and_default_constructs() {
        let before = SystemSet::new("before");
        let after = SystemSet::new("after");
        let mut builder = AppBuilder::default();
        builder.insert_resource(Vec::<&'static str>::new());
        builder.register_set(before.clone()).expect("before set");
        builder.register_set(after.clone()).expect("after set");
        builder
            .add_system(
                System::new("after", stage::UPDATE, |world, _dt| {
                    world
                        .resource_mut::<Vec<&'static str>>()
                        .expect("trace access")
                        .expect("trace resource")
                        .push("after");
                })
                .in_set(&after),
            )
            .expect("after system");
        builder
            .add_system(
                System::new("before", stage::UPDATE, |world, _dt| {
                    world
                        .resource_mut::<Vec<&'static str>>()
                        .expect("trace access")
                        .expect("trace resource")
                        .push("before");
                })
                .in_set(&before),
            )
            .expect("before system");
        builder
            .order_set_after(&after, &before)
            .expect("order after");

        let mut app = builder.build().expect("app");
        app.update(0.0).expect("update");
        assert_eq!(
            app.world()
                .resource::<Vec<&'static str>>()
                .expect("trace access")
                .expect("trace resource")
                .as_slice(),
            ["before", "after"]
        );
    }
}
