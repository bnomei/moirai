//! Stage and operation execution: conditions, system bodies, flush, and fault mapping.

use alloc::boxed::Box;
use alloc::string::{String, ToString};

use crate::diagnostics::{DiagnosticEvent, Observer};
use crate::operation::StageOperation;
use crate::schedule::compiled::CompiledSchedule;
use crate::schedule::stage;
use crate::schedule::system::FlushMode;
use crate::schedule::RunContext;
use crate::world::World;

/// Runtime fault location when a stage pass aborts before completion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunOutcome {
    /// Stage label active when the fault occurred.
    pub fault_stage: Option<String>,
    /// System label active when the fault occurred.
    pub fault_system: Option<String>,
    /// World guard, flush, poison, or system-returned detail.
    pub fault_detail: Option<String>,
}

/// Runs one compiled stage in deterministic system order.
pub(crate) fn run_stage(
    schedule: &mut CompiledSchedule,
    world: &mut World,
    stage_index: usize,
    context: &mut RunContext,
    dt: f32,
    observer: &mut Option<Box<dyn Observer>>,
) -> Result<(), RunOutcome> {
    let operation = schedule.stage_operation(stage_index);
    run_stage_inner(
        schedule,
        world,
        stage_index,
        operation,
        context,
        dt,
        observer,
    )
}

/// Runs every stage registered for an operation in compiled stage order.
pub(crate) fn run_operation(
    schedule: &mut CompiledSchedule,
    world: &mut World,
    operation: StageOperation,
    context: &mut RunContext,
    dt: f32,
    observer: &mut Option<Box<dyn Observer>>,
) -> Result<(), RunOutcome> {
    for i in 0..schedule.operation_stages(operation).len() {
        let stage_index = schedule.operation_stages(operation)[i];
        run_stage_inner(
            schedule,
            world,
            stage_index,
            operation,
            context,
            dt,
            observer,
        )?;
    }
    Ok(())
}

fn run_stage_inner(
    schedule: &mut CompiledSchedule,
    world: &mut World,
    stage_index: usize,
    operation: StageOperation,
    context: &mut RunContext,
    dt: f32,
    observer: &mut Option<Box<dyn Observer>>,
) -> Result<(), RunOutcome> {
    let stage_label = schedule.stages[stage_index].descriptor.label.as_str();
    // Startup runs at most once per schedule lifetime on the first Update pass.
    if operation == StageOperation::Update
        && stage_label == stage::STARTUP
        && schedule.startup_complete
    {
        return Ok(());
    }

    context.clear_set_cache();
    emit(observer, DiagnosticEvent::StageStart { name: stage_label });

    let system_order = &schedule.stages[stage_index].system_order;
    for &system_index in system_order {
        if !schedule.system_enabled[system_index] {
            continue;
        }
        if !evaluate_system_conditions(schedule, system_index, world, context) {
            continue;
        }

        emit(
            observer,
            DiagnosticEvent::SystemStart {
                name: &schedule.systems[system_index].name,
            },
        );

        world
            .begin_system_run(
                operation,
                alloc::rc::Rc::clone(&schedule.systems[system_index].event_access),
            )
            .map_err(|error| RunOutcome {
                fault_stage: Some(stage_label.to_string()),
                fault_system: Some(schedule.systems[system_index].name.clone()),
                fault_detail: Some(alloc::format!("{error:?}")),
            })?;

        let result = (schedule.systems[system_index].body)(world, dt);
        world.end_run();

        if world.is_mutation_poisoned() {
            return Err(RunOutcome {
                fault_stage: Some(stage_label.to_string()),
                fault_system: Some(schedule.systems[system_index].name.clone()),
                fault_detail: Some(String::from("world mutation is poisoned")),
            });
        }

        if let Err(detail) = result {
            return Err(RunOutcome {
                fault_stage: Some(stage_label.to_string()),
                fault_system: Some(schedule.systems[system_index].name.clone()),
                fault_detail: Some(detail),
            });
        }

        for condition in &schedule.systems[system_index].conditions {
            condition.advance_cursors(world, system_index, context);
        }

        emit(
            observer,
            DiagnosticEvent::SystemFinish {
                name: &schedule.systems[system_index].name,
            },
        );

        if schedule.systems[system_index].flush_mode == FlushMode::AfterSystem
            && operation == StageOperation::Update
        {
            flush_or_fault(
                world,
                stage_label,
                schedule.systems[system_index].name.as_str(),
                observer,
            )?;
        }
    }

    if operation == StageOperation::Update
        && schedule.stage_flush_mode(stage_index) == FlushMode::Stage
    {
        flush_or_fault(world, stage_label, "<stage>", observer)?;
    }

    // Advance set cursors only for sets that were evaluated this stage pass.
    let set_count = context
        .set_gate_cache
        .len()
        .min(schedule.set_conditions.len());
    for set_index in 0..set_count {
        if context.set_gate_cached(set_index).is_some() {
            schedule.set_conditions[set_index].advance_set_cursors(world, set_index, context);
        }
    }

    if operation == StageOperation::Update && stage_label == stage::STARTUP {
        schedule.startup_complete = true;
    }

    emit(observer, DiagnosticEvent::StageFinish { name: stage_label });
    Ok(())
}

/// Flushes deferred commands after the last Update stage when flush mode is Final.
pub(crate) fn final_update_flush(
    world: &mut World,
    observer: &mut Option<Box<dyn Observer>>,
) -> Result<(), RunOutcome> {
    flush_or_fault(world, "Update", "<final>", observer)
}

fn flush_or_fault(
    world: &mut World,
    stage: &str,
    system: &str,
    observer: &mut Option<Box<dyn Observer>>,
) -> Result<(), RunOutcome> {
    if let Err(error) = world.flush_commands() {
        return Err(RunOutcome {
            fault_stage: Some(stage.to_string()),
            fault_system: Some(system.to_string()),
            fault_detail: Some(alloc::format!("{error:?}")),
        });
    }
    emit(observer, DiagnosticEvent::FlushComplete);
    Ok(())
}

fn evaluate_system_conditions(
    schedule: &CompiledSchedule,
    system_index: usize,
    world: &World,
    context: &mut RunContext,
) -> bool {
    if let Some(set_index) = schedule.systems[system_index].in_set_index {
        let allowed = match context.set_gate_cached(set_index) {
            Some(value) => value,
            None => {
                let value = schedule
                    .set_conditions
                    .get(set_index)
                    .map(|condition| condition.evaluate_for_set(world, set_index, context))
                    .unwrap_or(true);
                context.set_gate(set_index, value);
                value
            }
        };
        if !allowed {
            return false;
        }
    }

    let conditions = &schedule.systems[system_index].conditions;
    if conditions.is_empty() {
        return true;
    }
    for condition in conditions {
        if !condition.evaluate(world, system_index, context) {
            return false;
        }
    }
    true
}

fn emit(observer: &mut Option<Box<dyn Observer>>, event: DiagnosticEvent<'_>) {
    if let Some(observer) = observer.as_mut() {
        observer.observe(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::operation::StageOperation;
    use crate::schedule::compiled::{CompiledSchedule, CompiledSystem};
    use crate::schedule::owner::{ExecutionLease, ScheduleOwner};
    use crate::schedule::stage::StageDescriptor;
    use crate::schedule::system::{FlushMode, SystemId};
    use crate::schedule::RunContext;
    use crate::time::{ChangeTick, FixedAccumulator};
    use crate::world::WorldBuilder;
    use alloc::boxed::Box;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    fn schedule_with_system(
        name: &str,
        body: crate::schedule::system::SystemBody,
    ) -> CompiledSchedule {
        let owner = ScheduleOwner::new();
        CompiledSchedule {
            owner: owner.clone(),
            lease: ExecutionLease::new(),
            generation: 1,
            stages: vec![crate::schedule::compiled::CompiledStage {
                descriptor: StageDescriptor {
                    label: String::from(stage::UPDATE),
                    operation: StageOperation::Update,
                    flush_mode: FlushMode::Final,
                },
                system_order: vec![0],
            }],
            systems: vec![CompiledSystem {
                name: String::from(name),
                stage_index: 0,
                body,
                enabled: true,
                flush_mode: FlushMode::Final,
                in_set_index: None,
                conditions: Vec::new(),
                id: SystemId::new(owner, 0, 1),
                event_access: alloc::rc::Rc::new(crate::world::guard::EventAccess::default()),
            }],
            update_stage_order: vec![0],
            render_stage_order: Vec::new(),
            fixed_config: None,
            fixed_accumulator: FixedAccumulator::new(),
            startup_complete: false,
            system_enabled: vec![true],
            set_conditions: Vec::new(),
        }
    }

    #[test]
    fn run_stage_maps_begin_run_fault() {
        let mut world = WorldBuilder::new().build().expect("world");
        world.set_run_guard_running_for_test(StageOperation::Update);
        let mut schedule = schedule_with_system("noop", Box::new(|_world, _dt| Ok(())));
        let mut context = RunContext::new();
        let mut observer = None;
        let outcome = run_stage(
            &mut schedule,
            &mut world,
            0,
            &mut context,
            0.0,
            &mut observer,
        )
        .expect_err("fault");
        assert_eq!(outcome.fault_stage.as_deref(), Some(stage::UPDATE));
        assert_eq!(outcome.fault_system.as_deref(), Some("noop"));
        assert!(outcome.fault_detail.is_some());
    }

    #[test]
    fn run_operation_propagates_stage_fault() {
        let mut world = WorldBuilder::new().build().expect("world");
        world.set_run_guard_running_for_test(StageOperation::Update);
        let mut schedule = schedule_with_system("noop", Box::new(|_world, _dt| Ok(())));
        let mut context = RunContext::new();
        let mut observer = None;
        assert!(run_operation(
            &mut schedule,
            &mut world,
            StageOperation::Update,
            &mut context,
            0.0,
            &mut observer,
        )
        .is_err());
    }

    #[derive(Clone, Copy)]
    struct PoisonedComponent;

    #[derive(Clone, Copy)]
    struct PoisonedResource;

    #[test]
    fn caught_component_tick_exhaustion_faults_after_system() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<PoisonedComponent>(ComponentOptions::sparse())
            .expect("component");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("entity");
        world
            .insert(entity, PoisonedComponent)
            .expect("seed component");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));

        let mut schedule = schedule_with_system(
            "component",
            Box::new(move |world, _dt| {
                let _ = world.get_mut::<PoisonedComponent>(entity);
                Ok(())
            }),
        );
        let mut context = RunContext::new();
        let mut observer = None;
        let outcome = run_stage(
            &mut schedule,
            &mut world,
            0,
            &mut context,
            0.0,
            &mut observer,
        )
        .expect_err("poison must fault");

        assert_eq!(outcome.fault_system.as_deref(), Some("component"));
        assert!(world.is_mutation_poisoned());
        assert!(world.run_guard_is_idle());
    }

    #[test]
    fn caught_resource_tick_exhaustion_faults_after_system() {
        let mut builder = WorldBuilder::new();
        builder.register_resource::<PoisonedResource>();
        let mut world = builder.build().expect("world");
        world
            .insert_resource(PoisonedResource)
            .expect("seed resource");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));

        let mut schedule = schedule_with_system(
            "resource",
            Box::new(|world, _dt| {
                let _ = world.resource_mut::<PoisonedResource>();
                Ok(())
            }),
        );
        let mut context = RunContext::new();
        let mut observer = None;
        let outcome = run_stage(
            &mut schedule,
            &mut world,
            0,
            &mut context,
            0.0,
            &mut observer,
        )
        .expect_err("poison must fault");

        assert_eq!(outcome.fault_system.as_deref(), Some("resource"));
        assert!(world.is_mutation_poisoned());
    }

    #[test]
    fn caught_query_tick_exhaustion_faults_after_system() {
        let mut world = WorldBuilder::new().build().expect("world");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX));
        let mut schedule = schedule_with_system(
            "query",
            Box::new(|world, _dt| {
                let _ = world.issue_change_tick_query();
                Ok(())
            }),
        );
        let mut context = RunContext::new();
        let mut observer = None;
        let outcome = run_stage(
            &mut schedule,
            &mut world,
            0,
            &mut context,
            0.0,
            &mut observer,
        )
        .expect_err("poison must fault");

        assert_eq!(outcome.fault_system.as_deref(), Some("query"));
        assert!(world.is_mutation_poisoned());
    }
}
