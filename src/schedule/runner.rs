use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::diagnostics::{DiagnosticEvent, Observer};
use crate::operation::StageOperation;
use crate::schedule::compiled::CompiledSchedule;
use crate::schedule::stage;
use crate::schedule::system::FlushMode;
use crate::schedule::RunContext;
use crate::world::World;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunOutcome {
    pub fault_stage: Option<String>,
    pub fault_system: Option<String>,
    pub fault_detail: Option<String>,
}

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

pub(crate) fn run_operation(
    schedule: &mut CompiledSchedule,
    world: &mut World,
    operation: StageOperation,
    context: &mut RunContext,
    dt: f32,
    observer: &mut Option<Box<dyn Observer>>,
) -> Result<(), RunOutcome> {
    let stage_order: Vec<usize> = schedule.operation_stages(operation).to_vec();
    for stage_index in stage_order {
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
    let stage_label = schedule.stage_label(stage_index).to_string();
    if operation == StageOperation::Update
        && stage_label == stage::STARTUP
        && schedule.startup_complete
    {
        return Ok(());
    }

    context.clear_set_cache();
    emit(observer, DiagnosticEvent::StageStart { name: &stage_label });

    let system_order = schedule.stages[stage_index].system_order.clone();
    for system_index in system_order {
        if !schedule.system_enabled[system_index] {
            continue;
        }
        if !evaluate_system_conditions(schedule, system_index, world, context) {
            continue;
        }

        let system_name = schedule.system_name(system_index).to_string();
        emit(
            observer,
            DiagnosticEvent::SystemStart { name: &system_name },
        );

        world.begin_run(operation).map_err(|error| RunOutcome {
            fault_stage: Some(stage_label.clone()),
            fault_system: Some(system_name.clone()),
            fault_detail: Some(alloc::format!("{error:?}")),
        })?;

        let result = (schedule.systems[system_index].body)(world, dt);
        world.end_run();

        if let Err(detail) = result {
            return Err(RunOutcome {
                fault_stage: Some(stage_label),
                fault_system: Some(system_name),
                fault_detail: Some(detail),
            });
        }

        for condition in &schedule.systems[system_index].conditions {
            condition.advance_cursors(world, system_index, context);
        }

        emit(
            observer,
            DiagnosticEvent::SystemFinish { name: &system_name },
        );

        if schedule.systems[system_index].flush_mode == FlushMode::AfterSystem
            && operation == StageOperation::Update
        {
            flush_or_fault(world, &stage_label, &system_name, observer)?;
        }
    }

    if operation == StageOperation::Update
        && schedule.stage_flush_mode(stage_index) == FlushMode::Stage
    {
        flush_or_fault(world, &stage_label, "<stage>", observer)?;
    }

    let evaluated_sets: Vec<String> = context.evaluated_set_labels().map(str::to_string).collect();
    for set_label in evaluated_sets {
        if let Some(condition) = schedule.set_conditions.get(&set_label).cloned() {
            condition.advance_set_cursors(world, &set_label, context);
        }
    }

    if operation == StageOperation::Update && stage_label == stage::STARTUP {
        schedule.startup_complete = true;
    }

    emit(
        observer,
        DiagnosticEvent::StageFinish { name: &stage_label },
    );
    Ok(())
}

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
    if let Some(set_label) = &schedule.systems[system_index].in_set {
        let allowed = match context.set_gate_cached(set_label) {
            Some(value) => value,
            None => {
                let condition = schedule
                    .set_conditions
                    .get(set_label)
                    .cloned()
                    .unwrap_or_else(Condition::always);
                let value = condition.evaluate_for_set(world, set_label, context);
                context.set_gate(set_label, value);
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

use crate::schedule::condition::Condition;
