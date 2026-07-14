use crate::event::EventRetention;
use crate::operation::StageOperation;
use crate::schedule::compiled::{CompiledSchedule, CompiledStage, CompiledSystem};
use crate::schedule::condition::Condition;
use crate::schedule::error::BuildError;
use crate::schedule::owner::{ExecutionLease, ScheduleOwner};
use crate::schedule::stage::{self, StageDescriptor};
use crate::schedule::system::{
    EventRoleKind, FlushMode, System, SystemBodySource, SystemInitContext, SystemSet,
};
use crate::schedule::Schedule;
use crate::time::{FixedAccumulator, FixedConfig};
use crate::world::World;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Authoring graph for a validated compiled schedule.
pub struct ScheduleBuilder {
    owner: ScheduleOwner,
    stages: Vec<StageDescriptor>,
    stage_index: BTreeMap<String, usize>,
    systems: Vec<System>,
    sets: BTreeMap<String, Condition>,
    set_ordering: Vec<(String, String)>,
    fixed_config: Option<FixedConfig>,
    default_update_flush: FlushMode,
}

impl ScheduleBuilder {
    pub fn new() -> Self {
        Self {
            owner: ScheduleOwner::new(),
            stages: Vec::new(),
            stage_index: BTreeMap::new(),
            systems: Vec::new(),
            sets: BTreeMap::new(),
            set_ordering: Vec::new(),
            fixed_config: None,
            default_update_flush: FlushMode::Final,
        }
    }

    pub fn standard() -> Self {
        let mut builder = Self::new();
        builder.default_update_flush = FlushMode::Stage;
        builder
            .add_stage(stage::STARTUP, StageOperation::Update)
            .expect("builtin");
        builder
            .add_stage(stage::FIXED_UPDATE, StageOperation::Update)
            .expect("builtin");
        builder
            .add_stage(stage::UPDATE, StageOperation::Update)
            .expect("builtin");
        builder
            .add_stage(stage::RENDER, StageOperation::Render)
            .expect("builtin");
        builder
    }

    pub fn add_stage(
        &mut self,
        label: impl Into<String>,
        operation: StageOperation,
    ) -> Result<(), BuildError> {
        let label = label.into();
        if let Some(index) = self.stage_index.get(&label).copied() {
            let existing = &self.stages[index];
            if existing.operation != operation {
                return Err(BuildError::StageOperationMismatch { label });
            }
            return Ok(());
        }
        let flush_mode = match operation {
            StageOperation::Update => self.default_update_flush,
            StageOperation::Render => FlushMode::Final,
        };
        let index = self.stages.len();
        self.stages.push(StageDescriptor {
            label: label.clone(),
            operation,
            flush_mode,
        });
        self.stage_index.insert(label, index);
        Ok(())
    }

    pub fn register_set(&mut self, set: SystemSet) -> Result<&mut Self, BuildError> {
        if self
            .sets
            .insert(set.label().into(), Condition::always())
            .is_some()
        {
            return Err(BuildError::DuplicateSystemSet {
                label: set.label().into(),
            });
        }
        Ok(self)
    }

    pub fn set_stage_flush_mode(
        &mut self,
        label: impl AsRef<str>,
        mode: FlushMode,
    ) -> Result<&mut Self, BuildError> {
        let label = label.as_ref();
        let index =
            self.stage_index
                .get(label)
                .copied()
                .ok_or_else(|| BuildError::UnknownStage {
                    label: label.into(),
                })?;
        let operation = self.stages[index].operation;
        let valid = matches!(
            (operation, mode),
            (StageOperation::Update, FlushMode::Final | FlushMode::Stage)
                | (StageOperation::Render, FlushMode::Final)
        );
        if !valid {
            return Err(BuildError::InvalidStageFlushMode {
                label: label.into(),
                mode,
            });
        }
        self.stages[index].flush_mode = mode;
        Ok(self)
    }

    pub fn set_run_if(
        &mut self,
        set: &SystemSet,
        condition: Condition,
    ) -> Result<&mut Self, BuildError> {
        match self.sets.get_mut(set.label()) {
            Some(existing) => {
                *existing = condition;
                Ok(self)
            }
            None => Err(BuildError::UnknownSystemSet {
                label: set.label().into(),
            }),
        }
    }

    pub fn order_set_before(
        &mut self,
        before: &SystemSet,
        after: &SystemSet,
    ) -> Result<&mut Self, BuildError> {
        self.ensure_registered_set(before)?;
        self.ensure_registered_set(after)?;
        self.set_ordering
            .push((before.label().into(), after.label().into()));
        Ok(self)
    }

    pub fn order_set_after(
        &mut self,
        after: &SystemSet,
        before: &SystemSet,
    ) -> Result<&mut Self, BuildError> {
        self.order_set_before(before, after)
    }

    fn ensure_registered_set(&self, set: &SystemSet) -> Result<(), BuildError> {
        if self.sets.contains_key(set.label()) {
            Ok(())
        } else {
            Err(BuildError::UnknownSystemSet {
                label: set.label().into(),
            })
        }
    }

    pub fn add_system(&mut self, system: System) -> Result<(), BuildError> {
        let stage_index = self
            .stage_index
            .get(&system.stage_label)
            .copied()
            .ok_or_else(|| BuildError::UnknownStage {
                label: system.stage_label.clone(),
            })?;
        if self
            .systems
            .iter()
            .any(|existing| existing.name == system.name)
        {
            return Err(BuildError::DuplicateSystemLabel {
                label: system.name.clone(),
            });
        }
        validate_system_flush_mode(&system, self.stages[stage_index].operation)?;
        if let Some(set_label) = &system.in_set {
            if !self.sets.contains_key(set_label) {
                return Err(BuildError::UnknownSystemSet {
                    label: set_label.clone(),
                });
            }
        }
        for set_label in system.before_sets.iter().chain(&system.after_sets) {
            if !self.sets.contains_key(set_label) {
                return Err(BuildError::UnknownSystemSet {
                    label: set_label.clone(),
                });
            }
        }
        self.systems.push(system);
        Ok(())
    }

    pub fn fixed(&mut self, config: FixedConfig) -> &mut Self {
        self.fixed_config = Some(config);
        self
    }

    pub fn build(self, world: &mut World) -> Result<Schedule, BuildError> {
        if world.has_pending_commands() {
            return Err(BuildError::PendingCommands);
        }
        if !world.run_guard_is_idle() {
            return Err(BuildError::WorldRunning);
        }
        if world.is_mutation_poisoned() {
            return Err(BuildError::WorldMutationPoisoned);
        }
        world.prune_dead_execution_lease();
        if world.has_live_execution_lease() {
            return Err(BuildError::LiveLeaseAlreadyAttached);
        }

        let has_fixed_stage = self
            .stages
            .iter()
            .any(|stage| stage.label == stage::FIXED_UPDATE);
        let has_fixed_system = self
            .systems
            .iter()
            .any(|system| system.stage_label == stage::FIXED_UPDATE);
        if has_fixed_system && self.fixed_config.is_none() {
            return Err(BuildError::FixedUpdateWithoutConfig);
        }
        if self.fixed_config.is_some() && !has_fixed_stage {
            return Err(BuildError::FixedConfigWithoutFixedUpdate);
        }

        let mut stage_systems = vec![Vec::new(); self.stages.len()];
        let mut name_to_index = BTreeMap::<String, usize>::new();
        for (index, system) in self.systems.iter().enumerate() {
            if let Some(existing) = name_to_index.insert(system.name.clone(), index) {
                let _ = existing;
                return Err(BuildError::DuplicateSystemLabel {
                    label: system.name.clone(),
                });
            }
            let stage_index = *self
                .stage_index
                .get(&system.stage_label)
                .expect("validated stage");
            stage_systems[stage_index].push(index);
        }

        for system in &self.systems {
            let stage_index = *self
                .stage_index
                .get(&system.stage_label)
                .expect("validated stage");
            validate_system_flush_mode(system, self.stages[stage_index].operation)?;
            for before in &system.before {
                if !name_to_index.contains_key(before) {
                    return Err(BuildError::UnknownSystem {
                        label: before.clone(),
                    });
                }
            }
            for after in &system.after {
                if !name_to_index.contains_key(after) {
                    return Err(BuildError::UnknownSystem {
                        label: after.clone(),
                    });
                }
            }
            if system.before.contains(&system.name) || system.after.contains(&system.name) {
                return Err(BuildError::SelfEdge {
                    label: system.name.clone(),
                });
            }
            for set_label in system.before_sets.iter().chain(&system.after_sets) {
                if !self.sets.contains_key(set_label) {
                    return Err(BuildError::UnknownSystemSet {
                        label: set_label.clone(),
                    });
                }
            }
        }

        let mut required_resources = Vec::<core::any::TypeId>::new();
        for system in &self.systems {
            for type_id in &system.required_resources {
                if !world.resource_present(*type_id) {
                    return Err(BuildError::MissingRequiredResource {
                        name: world
                            .resource_type_name(*type_id)
                            .unwrap_or("<resource>")
                            .into(),
                    });
                }
                if !required_resources.contains(type_id) {
                    required_resources.push(*type_id);
                }
            }
        }

        let ordering_edges = collect_ordering_edges(
            &self.systems,
            &self.sets,
            &self.set_ordering,
            &name_to_index,
        )?;

        let mut compiled_stages = Vec::with_capacity(self.stages.len());
        let generation = 1u32;
        let lease = ExecutionLease::new();

        for (stage_index, descriptor) in self.stages.iter().enumerate() {
            let order =
                topological_sort(&stage_systems[stage_index], &self.systems, &ordering_edges)?;
            compiled_stages.push(CompiledStage {
                descriptor: descriptor.clone(),
                system_order: order,
            });
        }

        let compiled_event_access = validate_event_roles(
            &self.systems,
            &self.stages,
            &self.stage_index,
            &ordering_edges,
            world,
        )?;

        let mut set_conditions = Vec::with_capacity(self.sets.len());
        let mut set_index_map = BTreeMap::<String, usize>::new();
        for (index, (label, condition)) in self.sets.into_iter().enumerate() {
            set_index_map.insert(label, index);
            set_conditions.push(condition);
        }

        let mut compiled_systems = Vec::with_capacity(self.systems.len());
        for (index, system) in self.systems.into_iter().enumerate() {
            let stage_index = *self
                .stage_index
                .get(&system.stage_label)
                .expect("validated stage");
            let in_set_index = system
                .in_set
                .as_ref()
                .and_then(|label| set_index_map.get(label).copied());
            let system_name = system.name.clone();
            let body = match system.body {
                SystemBodySource::Ready(body) => body,
                SystemBodySource::Initialize(initializer) => {
                    let mut context = SystemInitContext::new(world);
                    initializer(&mut context).map_err(|detail| {
                        BuildError::SystemInitialization {
                            system: system_name.clone(),
                            detail,
                        }
                    })?
                }
            };
            compiled_systems.push(CompiledSystem {
                name: system_name,
                stage_index,
                body,
                enabled: system.enabled,
                flush_mode: system.flush_mode,
                conditions: system.conditions,
                in_set_index,
                id: crate::schedule::system::SystemId::new(
                    self.owner.clone(),
                    index as u32,
                    generation,
                ),
                event_access: Rc::new(compiled_event_access[index].clone()),
            });
        }

        let update_stage_order = self
            .stages
            .iter()
            .enumerate()
            .filter(|(_, stage)| stage.operation == StageOperation::Update)
            .map(|(index, _)| index)
            .collect();
        let render_stage_order = self
            .stages
            .iter()
            .enumerate()
            .filter(|(_, stage)| stage.operation == StageOperation::Render)
            .map(|(index, _)| index)
            .collect();

        let system_enabled = compiled_systems
            .iter()
            .map(|system| system.enabled)
            .collect();

        world.attach_execution_lease_with_locks(lease.downgrade(), &required_resources);

        Ok(Schedule {
            compiled: CompiledSchedule {
                owner: self.owner,
                lease,
                generation,
                stages: compiled_stages,
                systems: compiled_systems,
                update_stage_order,
                render_stage_order,
                fixed_config: self.fixed_config,
                fixed_accumulator: FixedAccumulator::new(),
                startup_complete: false,
                system_enabled,
                set_conditions,
            },
        })
    }
}

#[derive(Clone)]
struct ResolvedEventRole {
    event_id: crate::event::EventId,
    event_name: String,
    kind: EventRoleKind,
    external_source: bool,
}

fn validate_event_roles(
    systems: &[System],
    stages: &[StageDescriptor],
    stage_indices: &BTreeMap<String, usize>,
    ordering_edges: &[(usize, usize)],
    world: &World,
) -> Result<Vec<crate::world::guard::EventAccess>, BuildError> {
    let mut resolved = Vec::with_capacity(systems.len());
    for system in systems {
        let stage_index = *stage_indices
            .get(&system.stage_label)
            .expect("system stage validated");
        let system_operation = stages[stage_index].operation;
        let mut roles = Vec::with_capacity(system.event_roles.len());
        for role in &system.event_roles {
            let event_id = match role.kind {
                EventRoleKind::Emits | EventRoleKind::Consumes => {
                    world.event_id_of_type(role.type_id)
                }
                EventRoleKind::ConsumesOnAdd => world.lifecycle_event_id(role.type_id, true),
                EventRoleKind::ConsumesOnRemove => world.lifecycle_event_id(role.type_id, false),
            }
            .ok_or_else(|| BuildError::UnregisteredEventRole {
                system: system.name.clone(),
                event: String::from(role.type_name),
            })?;
            let options = world
                .event_options(&event_id)
                .expect("resolved event has registered options");
            let event_operation = match role.kind {
                EventRoleKind::ConsumesOnAdd | EventRoleKind::ConsumesOnRemove => {
                    Some(StageOperation::Update)
                }
                EventRoleKind::Emits | EventRoleKind::Consumes => match options.retention() {
                    EventRetention::Frame(operation) => Some(operation),
                    EventRetention::Manual | EventRetention::Bounded(_) => None,
                },
            };
            if let Some(event_operation) = event_operation {
                if event_operation != system_operation {
                    return Err(BuildError::EventOperationMismatch {
                        system: system.name.clone(),
                        event: String::from(role.type_name),
                        event_operation,
                        system_operation,
                    });
                }
            }
            roles.push(ResolvedEventRole {
                event_id,
                event_name: String::from(role.type_name),
                kind: role.kind,
                external_source: options.is_external_source(),
            });
        }
        resolved.push(roles);
    }

    for (consumer_index, roles) in resolved.iter().enumerate() {
        for consumer in roles {
            if consumer.kind != EventRoleKind::Consumes || consumer.external_source {
                continue;
            }
            let producers: Vec<usize> = resolved
                .iter()
                .enumerate()
                .filter(|(_, roles)| {
                    roles.iter().any(|role| {
                        role.kind == EventRoleKind::Emits && role.event_id == consumer.event_id
                    })
                })
                .map(|(index, _)| index)
                .collect();
            if producers.is_empty() {
                return Err(BuildError::MissingEventProducer {
                    system: systems[consumer_index].name.clone(),
                    event: consumer.event_name.clone(),
                });
            }
            for producer_index in producers {
                if system_precedes(
                    producer_index,
                    consumer_index,
                    systems,
                    stages,
                    stage_indices,
                    ordering_edges,
                ) {
                    continue;
                }
                return Err(BuildError::UnreachableEventProducer {
                    producer: systems[producer_index].name.clone(),
                    consumer: systems[consumer_index].name.clone(),
                    event: consumer.event_name.clone(),
                });
            }
        }
    }

    Ok(resolved
        .into_iter()
        .map(|roles| {
            let emitted = roles
                .iter()
                .filter(|role| role.kind == EventRoleKind::Emits)
                .map(|role| role.event_id.clone())
                .collect();
            let consumed = roles
                .iter()
                .filter(|role| role.kind != EventRoleKind::Emits)
                .map(|role| role.event_id.clone())
                .collect();
            crate::world::guard::EventAccess::new(emitted, consumed)
        })
        .collect())
}

fn system_precedes(
    producer: usize,
    consumer: usize,
    systems: &[System],
    stages: &[StageDescriptor],
    stage_indices: &BTreeMap<String, usize>,
    ordering_edges: &[(usize, usize)],
) -> bool {
    let producer_stage = stage_indices[&systems[producer].stage_label];
    let consumer_stage = stage_indices[&systems[consumer].stage_label];
    if producer_stage != consumer_stage {
        return stages[producer_stage].operation == stages[consumer_stage].operation
            && producer_stage < consumer_stage;
    }
    has_explicit_path(producer, consumer, systems.len(), ordering_edges)
}

fn has_explicit_path(
    from: usize,
    to: usize,
    system_count: usize,
    ordering_edges: &[(usize, usize)],
) -> bool {
    let mut visited = vec![false; system_count];
    let mut pending = vec![from];
    while let Some(current) = pending.pop() {
        if visited[current] {
            continue;
        }
        visited[current] = true;
        for &(edge_from, edge_to) in ordering_edges {
            if edge_from == current {
                if edge_to == to {
                    return true;
                }
                pending.push(edge_to);
            }
        }
    }
    false
}

fn validate_system_flush_mode(
    system: &System,
    operation: StageOperation,
) -> Result<(), BuildError> {
    let valid = matches!(system.flush_mode, FlushMode::Final)
        || matches!(
            (operation, system.flush_mode),
            (StageOperation::Update, FlushMode::AfterSystem)
        );
    if valid {
        Ok(())
    } else {
        Err(BuildError::InvalidSystemFlushMode {
            label: system.name.clone(),
            mode: system.flush_mode,
        })
    }
}

fn collect_ordering_edges(
    systems: &[System],
    sets: &BTreeMap<String, Condition>,
    set_ordering: &[(String, String)],
    names: &BTreeMap<String, usize>,
) -> Result<Vec<(usize, usize)>, BuildError> {
    let mut edges = Vec::new();

    for (system_index, system) in systems.iter().enumerate() {
        for before in &system.before {
            let target = *names.get(before).ok_or_else(|| BuildError::UnknownSystem {
                label: before.clone(),
            })?;
            push_ordering_edge(&mut edges, system_index, target, systems)?;
        }
        for after in &system.after {
            let source = *names.get(after).ok_or_else(|| BuildError::UnknownSystem {
                label: after.clone(),
            })?;
            push_ordering_edge(&mut edges, source, system_index, systems)?;
        }
        for before_set in &system.before_sets {
            ensure_set_label(sets, before_set)?;
            for (target, candidate) in systems.iter().enumerate() {
                if candidate.in_set.as_ref() == Some(before_set) {
                    push_ordering_edge(&mut edges, system_index, target, systems)?;
                }
            }
        }
        for after_set in &system.after_sets {
            ensure_set_label(sets, after_set)?;
            for (source, candidate) in systems.iter().enumerate() {
                if candidate.in_set.as_ref() == Some(after_set) {
                    push_ordering_edge(&mut edges, source, system_index, systems)?;
                }
            }
        }
    }

    for (before_set, after_set) in set_ordering {
        ensure_set_label(sets, before_set)?;
        ensure_set_label(sets, after_set)?;
        for (source, source_system) in systems.iter().enumerate() {
            if source_system.in_set.as_ref() != Some(before_set) {
                continue;
            }
            for (target, target_system) in systems.iter().enumerate() {
                if target_system.in_set.as_ref() == Some(after_set) {
                    push_ordering_edge(&mut edges, source, target, systems)?;
                }
            }
        }
    }

    edges.sort_unstable();
    edges.dedup();
    Ok(edges)
}

fn ensure_set_label(sets: &BTreeMap<String, Condition>, label: &str) -> Result<(), BuildError> {
    if sets.contains_key(label) {
        Ok(())
    } else {
        Err(BuildError::UnknownSystemSet {
            label: label.into(),
        })
    }
}

fn push_ordering_edge(
    edges: &mut Vec<(usize, usize)>,
    from: usize,
    to: usize,
    systems: &[System],
) -> Result<(), BuildError> {
    if from == to {
        return Err(BuildError::SelfEdge {
            label: systems[from].name.clone(),
        });
    }
    if systems[from].stage_label != systems[to].stage_label {
        return Err(BuildError::CrossStageSystemEdge {
            from: systems[from].name.clone(),
            to: systems[to].name.clone(),
        });
    }
    edges.push((from, to));
    Ok(())
}

impl Default for ScheduleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn topological_sort(
    stage_members: &[usize],
    systems: &[System],
    ordering_edges: &[(usize, usize)],
) -> Result<Vec<usize>, BuildError> {
    if stage_members.is_empty() {
        return Ok(Vec::new());
    }

    let mut indegree = BTreeMap::<usize, usize>::new();
    for &index in stage_members {
        indegree.insert(index, 0);
    }
    for (_, to) in ordering_edges {
        if !indegree.contains_key(to) {
            continue;
        }
        *indegree.get_mut(to).expect("member") += 1;
    }

    let mut ready: Vec<usize> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(index, _)| *index)
        .collect();
    ready.sort_unstable();

    let mut order = Vec::with_capacity(stage_members.len());
    while let Some(index) = ready.first().copied() {
        ready.remove(0);
        order.push(index);
        for (from, to) in ordering_edges {
            if *from != index {
                continue;
            }
            let entry = indegree.get_mut(to).expect("member");
            *entry -= 1;
            if *entry == 0 {
                ready.push(*to);
            }
        }
        ready.sort_unstable();
    }

    if order.len() != stage_members.len() {
        let path = stage_members
            .iter()
            .map(|index| systems[*index].name.clone())
            .collect();
        return Err(BuildError::Cycle { path });
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::time::ChangeTick;
    use crate::world::WorldBuilder;
    use crate::StageOperation;

    #[test]
    fn build_rejects_running_world() {
        let mut world = WorldBuilder::new().build().expect("world");
        world.begin_run(StageOperation::Update).expect("begin");
        assert!(matches!(
            ScheduleBuilder::standard().build(&mut world),
            Err(BuildError::WorldRunning)
        ));
        world.end_run();
    }

    #[test]
    fn build_rejects_poisoned_world() {
        #[derive(Clone, Copy)]
        struct Health;

        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Health>(ComponentOptions::sparse())
            .expect("component");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("entity");
        world.insert(entity, Health).expect("seed");
        world.set_change_tick_for_test(ChangeTick::from_raw(u64::MAX - 1));
        world.insert(entity, Health).expect("consume last tick");
        let _ = world.insert(entity, Health);

        assert!(world.is_mutation_poisoned());
        assert!(matches!(
            ScheduleBuilder::standard().build(&mut world),
            Err(BuildError::WorldMutationPoisoned)
        ));
    }

    #[test]
    fn build_detects_duplicate_labels_when_add_system_validation_bypassed() {
        let mut world = WorldBuilder::new().build().expect("world");
        let mut builder = ScheduleBuilder::standard();
        builder
            .systems
            .push(System::new("dup", stage::UPDATE, |_world, _dt| {}));
        builder
            .systems
            .push(System::new("dup", stage::UPDATE, |_world, _dt| {}));
        assert!(matches!(
            builder.build(&mut world),
            Err(BuildError::DuplicateSystemLabel { label })
                if label == "dup"
        ));
    }

    #[test]
    fn build_detects_fixed_update_without_config_when_add_system_validation_bypassed() {
        let mut world = WorldBuilder::new().build().expect("world");
        let mut builder = ScheduleBuilder::new();
        builder
            .add_stage(stage::FIXED_UPDATE, StageOperation::Update)
            .expect("stage");
        builder
            .systems
            .push(System::new("fixed", stage::FIXED_UPDATE, |_world, _dt| {}));
        assert!(matches!(
            builder.build(&mut world),
            Err(BuildError::FixedUpdateWithoutConfig)
        ));
    }

    #[test]
    fn topological_sort_rejects_unknown_before_and_after_edges() {
        let mut names = BTreeMap::new();
        names.insert(String::from("leaf"), 0);
        let sets = BTreeMap::new();
        let systems = vec![System::new("leaf", stage::UPDATE, |_world, _dt| {}).before("ghost")];
        assert!(matches!(
            collect_ordering_edges(&systems, &sets, &[], &names),
            Err(BuildError::UnknownSystem { label })
                if label == "ghost"
        ));

        let systems = vec![System::new("leaf", stage::UPDATE, |_world, _dt| {}).after("missing")];
        assert!(matches!(
            collect_ordering_edges(&systems, &sets, &[], &names),
            Err(BuildError::UnknownSystem { label })
                if label == "missing"
        ));
    }

    #[test]
    fn builder_reports_unknown_sets_and_invalid_flush_modes() {
        let mut builder = ScheduleBuilder::standard();
        assert!(matches!(
            builder.set_stage_flush_mode("missing", FlushMode::Final),
            Err(BuildError::UnknownStage { .. })
        ));
        assert!(matches!(
            builder.set_stage_flush_mode(stage::RENDER, FlushMode::Stage),
            Err(BuildError::InvalidStageFlushMode { .. })
        ));

        let missing = SystemSet::new("missing");
        assert!(matches!(
            builder.add_system(
                System::new("member", stage::UPDATE, |_world, _dt| {}).in_set(&missing)
            ),
            Err(BuildError::UnknownSystemSet { .. })
        ));
        assert!(matches!(
            builder.add_system(
                System::new("before", stage::UPDATE, |_world, _dt| {}).before_set(&missing)
            ),
            Err(BuildError::UnknownSystemSet { .. })
        ));
    }

    #[test]
    fn internal_ordering_helpers_cover_cycles_unknown_sets_and_invalid_edges() {
        assert!(!has_explicit_path(0, 2, 3, &[(0, 1), (1, 0)]));

        let sets = BTreeMap::new();
        assert!(matches!(
            ensure_set_label(&sets, "missing"),
            Err(BuildError::UnknownSystemSet { .. })
        ));

        let systems = vec![
            System::new("update", stage::UPDATE, |_world, _dt| {}),
            System::new("render", stage::RENDER, |_world, _dt| {}),
        ];
        assert!(matches!(
            push_ordering_edge(&mut Vec::new(), 0, 0, &systems),
            Err(BuildError::SelfEdge { .. })
        ));
        assert!(matches!(
            push_ordering_edge(&mut Vec::new(), 0, 1, &systems),
            Err(BuildError::CrossStageSystemEdge { .. })
        ));
        assert!(matches!(
            validate_system_flush_mode(
                &System::new("render", stage::RENDER, |_world, _dt| {}).flush_after(),
                StageOperation::Render,
            ),
            Err(BuildError::InvalidSystemFlushMode { .. })
        ));
    }

    #[test]
    fn build_rechecks_unknown_before_set_when_validation_is_bypassed() {
        let mut world = WorldBuilder::new().build().expect("world");
        let missing = SystemSet::new("missing");
        let mut builder = ScheduleBuilder::standard();
        builder
            .systems
            .push(System::new("bypassed", stage::UPDATE, |_world, _dt| {}).before_set(&missing));
        assert!(matches!(
            builder.build(&mut world),
            Err(BuildError::UnknownSystemSet { .. })
        ));
    }

    #[test]
    fn build_deduplicates_present_required_resource_locks() {
        #[derive(Clone)]
        struct SharedResource;

        let mut world_builder = WorldBuilder::new();
        world_builder.register_resource::<SharedResource>();
        let mut world = world_builder.build().expect("world");
        world
            .insert_resource(SharedResource)
            .expect("insert resource");

        let mut builder = ScheduleBuilder::standard();
        builder
            .add_system(
                System::new("first", stage::UPDATE, |_world, _dt| {})
                    .requires_resource::<SharedResource>(),
            )
            .expect("first");
        builder
            .add_system(
                System::new("second", stage::UPDATE, |_world, _dt| {})
                    .requires_resource::<SharedResource>(),
            )
            .expect("second");

        builder.build(&mut world).expect("schedule");
    }
}
