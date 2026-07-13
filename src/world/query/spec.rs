use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{type_name, TypeId};
use core::hash::{Hash, Hasher};

use crate::component::ComponentId;
use crate::query::{ExactIdPolicy, QueryError, QuerySpec};
use crate::world::World;

use super::plan::{ResolvedPlan, TraversalSource};
use super::plan_cache::QueryResolveScratch;

struct PreparedQuery1 {
    fingerprint: u64,
    primary_index: usize,
    primary_is_table: bool,
    traversal: TraversalSource,
    added_indices: Vec<usize>,
    changed_indices: Vec<usize>,
    exact_id_policy: Option<ExactIdPolicy>,
}

pub(crate) fn peek_query1_fingerprint<T: 'static>(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<u64, QueryError> {
    Ok(prepare_query1::<T>(world, spec, scratch)?.fingerprint)
}

pub(crate) fn resolve_query1<T: 'static>(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<ResolvedPlan, QueryError> {
    let prepared = prepare_query1::<T>(world, spec, scratch)?;
    Ok(ResolvedPlan {
        fingerprint: prepared.fingerprint,
        primary_index: prepared.primary_index,
        primary_is_table: prepared.primary_is_table,
        traversal: prepared.traversal,
        required_indices: scratch.required.clone(),
        without_indices: scratch.without.clone(),
        with_tag_indices: scratch.with_tags.clone(),
        without_tag_indices: scratch.without_tags.clone(),
        added_indices: prepared.added_indices,
        changed_indices: prepared.changed_indices,
        exact_id_policy: prepared.exact_id_policy,
    })
}

fn prepare_query1<T: 'static>(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<PreparedQuery1, QueryError> {
    let primary = resolve_component::<T>(world)?;
    let primary_index = primary.index();
    let primary_is_table = world.registry_is_table(&primary);

    let (added_indices, changed_indices) = fill_spec_indices(world, spec, scratch)?;
    if !scratch.required.contains(&primary_index) {
        scratch.required.push(primary_index);
    }
    normalize(&mut scratch.required);
    validate_overlaps(scratch)?;

    let traversal = if let Some(ids) = &spec.exact_ids {
        TraversalSource::Exact { ids: ids.clone() }
    } else if primary_is_table {
        TraversalSource::Table {
            component_index: primary_index,
        }
    } else {
        TraversalSource::Sparse {
            component_index: primary_index,
        }
    };

    let fingerprint = fingerprint_plan(
        &scratch.required,
        &scratch.without,
        &scratch.with_tags,
        &scratch.without_tags,
        &added_indices,
        &changed_indices,
        &traversal,
        Some(primary_index),
        spec.exact_id_policy,
    );

    Ok(PreparedQuery1 {
        fingerprint,
        primary_index,
        primary_is_table,
        traversal,
        added_indices,
        changed_indices,
        exact_id_policy: spec.exact_id_policy,
    })
}

pub(crate) fn resolve_query2<A: 'static, B: 'static>(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<(ResolvedPlan, usize, bool), QueryError> {
    let primary_a = resolve_component::<A>(world)?;
    let primary_b = resolve_component::<B>(world)?;
    let second_index = primary_b.index();
    let second_is_table = world.registry_is_table(&primary_b);

    let (added_indices, changed_indices) = fill_spec_indices(world, spec, scratch)?;
    for index in [primary_a.index(), primary_b.index()] {
        if !scratch.required.contains(&index) {
            scratch.required.push(index);
        }
    }
    normalize(&mut scratch.required);
    validate_overlaps(scratch)?;

    let primary_index = primary_a.index();
    let primary_is_table = world.registry_is_table(&primary_a);

    let traversal = if let Some(ids) = &spec.exact_ids {
        TraversalSource::Exact { ids: ids.clone() }
    } else if primary_is_table {
        TraversalSource::Table {
            component_index: primary_index,
        }
    } else {
        TraversalSource::Sparse {
            component_index: primary_index,
        }
    };

    let fingerprint = fingerprint_plan(
        &scratch.required,
        &scratch.without,
        &scratch.with_tags,
        &scratch.without_tags,
        &added_indices,
        &changed_indices,
        &traversal,
        Some(primary_index),
        spec.exact_id_policy,
    );

    let plan = ResolvedPlan {
        fingerprint,
        primary_index,
        primary_is_table,
        traversal,
        required_indices: scratch.required.clone(),
        without_indices: scratch.without.clone(),
        with_tag_indices: scratch.with_tags.clone(),
        without_tag_indices: scratch.without_tags.clone(),
        added_indices,
        changed_indices,
        exact_id_policy: spec.exact_id_policy,
    };

    Ok((plan, second_index, second_is_table))
}

pub(crate) fn peek_entities_fingerprint(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<u64, QueryError> {
    Ok(resolve_entities(world, spec, scratch)?.fingerprint)
}

pub(crate) fn resolve_entities(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<ResolvedPlan, QueryError> {
    let (added_indices, changed_indices) = fill_spec_indices(world, spec, scratch)?;
    validate_overlaps(scratch)?;

    let traversal = if let Some(ids) = &spec.exact_ids {
        TraversalSource::Exact { ids: ids.clone() }
    } else if let Some(&index) = scratch.required.first() {
        traversal_for_index(world, index)
    } else if let Some(&index) = scratch.with_tags.first() {
        TraversalSource::Sparse {
            component_index: index,
        }
    } else {
        TraversalSource::All
    };
    let fingerprint = fingerprint_plan(
        &scratch.required,
        &scratch.without,
        &scratch.with_tags,
        &scratch.without_tags,
        &added_indices,
        &changed_indices,
        &traversal,
        None,
        spec.exact_id_policy,
    );
    Ok(ResolvedPlan {
        fingerprint,
        primary_index: usize::MAX,
        primary_is_table: false,
        traversal,
        required_indices: scratch.required.clone(),
        without_indices: scratch.without.clone(),
        with_tag_indices: scratch.with_tags.clone(),
        without_tag_indices: scratch.without_tags.clone(),
        added_indices,
        changed_indices,
        exact_id_policy: spec.exact_id_policy,
    })
}

fn traversal_for_index(world: &World, index: usize) -> TraversalSource {
    let id = ComponentId::new(world.owner_token(), index as u32);
    if world.registry_is_table(&id) {
        TraversalSource::Table {
            component_index: index,
        }
    } else {
        TraversalSource::Sparse {
            component_index: index,
        }
    }
}

fn fill_spec_indices(
    world: &World,
    spec: &QuerySpec,
    scratch: &mut QueryResolveScratch,
) -> Result<(Vec<usize>, Vec<usize>), QueryError> {
    fill_type_and_component_ids(
        world,
        &spec.required,
        &spec.required_ids,
        false,
        &mut scratch.required,
    )?;
    fill_type_and_component_ids(
        world,
        &spec.without,
        &spec.without_ids,
        false,
        &mut scratch.without,
    )?;
    fill_type_and_component_ids(
        world,
        &spec.with_tags,
        &spec.with_tag_ids,
        true,
        &mut scratch.with_tags,
    )?;
    fill_type_and_component_ids(
        world,
        &spec.without_tags,
        &spec.without_tag_ids,
        true,
        &mut scratch.without_tags,
    )?;

    if spec.exact_ids.is_some() && spec.exact_id_policy.is_none() {
        return Err(QueryError::WrongQuery {
            detail: String::from("exact-id queries require an explicit ExactIdPolicy"),
        });
    }

    let mut added = resolve_selector_group(world, &spec.added, &spec.added_ids)?;
    let mut changed = resolve_selector_group(world, &spec.changed, &spec.changed_ids)?;
    normalize(&mut added);
    normalize(&mut changed);
    if !added.is_empty() && !changed.is_empty() {
        return Err(QueryError::ConflictingFilters {
            detail: String::from("added and changed filters are mutually exclusive"),
        });
    }
    Ok((added, changed))
}

fn fill_type_and_component_ids(
    world: &World,
    type_ids: &[TypeId],
    component_ids: &[ComponentId],
    require_tag: bool,
    out: &mut Vec<usize>,
) -> Result<(), QueryError> {
    out.clear();
    for &type_id in type_ids {
        let id = resolve_type_id(world, type_id)?;
        validate_tag_kind(world, &id, require_tag)?;
        out.push(id.index());
    }
    for id in component_ids {
        validate_component_id(world, id)?;
        validate_tag_kind(world, id, require_tag)?;
        out.push(id.index());
    }
    normalize(out);
    Ok(())
}

fn resolve_selector_group(
    world: &World,
    type_ids: &[TypeId],
    component_ids: &[ComponentId],
) -> Result<Vec<usize>, QueryError> {
    let mut out = Vec::with_capacity(type_ids.len() + component_ids.len());
    for &type_id in type_ids {
        out.push(resolve_type_id(world, type_id)?.index());
    }
    for id in component_ids {
        validate_component_id(world, id)?;
        out.push(id.index());
    }
    Ok(out)
}

fn validate_component_id(world: &World, id: &ComponentId) -> Result<(), QueryError> {
    if id.validate_owner(world.owner()).is_err() {
        return Err(QueryError::WrongOwner);
    }
    if !world.registry_contains(id) {
        return Err(QueryError::UnregisteredComponent {
            name: String::from("<stale component id>"),
        });
    }
    Ok(())
}

fn validate_tag_kind(world: &World, id: &ComponentId, require_tag: bool) -> Result<(), QueryError> {
    if require_tag && !world.is_tag_component(id) {
        return Err(QueryError::WrongStorageKind {
            name: world.registry_component_name(id),
        });
    }
    Ok(())
}

fn normalize(indices: &mut Vec<usize>) {
    indices.sort_unstable();
    indices.dedup();
}

fn validate_overlaps(scratch: &QueryResolveScratch) -> Result<(), QueryError> {
    validate_no_overlap(&scratch.required, &scratch.without, "required", "without")?;
    validate_no_overlap(
        &scratch.required,
        &scratch.without_tags,
        "required",
        "without_tag",
    )?;
    validate_no_overlap(&scratch.with_tags, &scratch.without, "with_tag", "without")?;
    validate_no_overlap(
        &scratch.with_tags,
        &scratch.without_tags,
        "with_tag",
        "without_tag",
    )
}

fn resolve_component<T: 'static>(world: &World) -> Result<ComponentId, QueryError> {
    world
        .registry_id_of::<T>()
        .ok_or_else(|| QueryError::UnregisteredComponent {
            name: String::from(type_name::<T>()),
        })
}

fn resolve_type_id(world: &World, type_id: TypeId) -> Result<ComponentId, QueryError> {
    world
        .registry_id_of_type(type_id)
        .ok_or_else(|| QueryError::UnregisteredComponent {
            name: String::from("<unregistered component>"),
        })
}

fn validate_no_overlap(
    left: &[usize],
    right: &[usize],
    left_name: &str,
    right_name: &str,
) -> Result<(), QueryError> {
    for index in left {
        if right.contains(index) {
            return Err(QueryError::ConflictingFilters {
                detail: format!("{left_name} and {right_name} both reference index {index}"),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fingerprint_plan(
    required: &[usize],
    without: &[usize],
    with_tags: &[usize],
    without_tags: &[usize],
    added: &[usize],
    changed: &[usize],
    traversal: &TraversalSource,
    primary: Option<usize>,
    exact_id_policy: Option<ExactIdPolicy>,
) -> u64 {
    let mut hasher = FnvHasher::new();
    primary.is_none().hash(&mut hasher);
    primary.hash(&mut hasher);
    for index in required {
        index.hash(&mut hasher);
    }
    for index in without {
        index.hash(&mut hasher);
        1u8.hash(&mut hasher);
    }
    for index in with_tags {
        index.hash(&mut hasher);
        2u8.hash(&mut hasher);
    }
    for index in without_tags {
        index.hash(&mut hasher);
        3u8.hash(&mut hasher);
    }
    added.hash(&mut hasher);
    changed.hash(&mut hasher);
    exact_id_policy.hash(&mut hasher);
    match traversal {
        TraversalSource::All => {
            0u8.hash(&mut hasher);
        }
        TraversalSource::Sparse { component_index } => {
            1u8.hash(&mut hasher);
            component_index.hash(&mut hasher);
        }
        TraversalSource::Table { component_index } => {
            2u8.hash(&mut hasher);
            component_index.hash(&mut hasher);
        }
        TraversalSource::Exact { ids } => {
            3u8.hash(&mut hasher);
            ids.len().hash(&mut hasher);
            for id in ids {
                id.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

struct FnvHasher(u64);

impl FnvHasher {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= *byte as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn write_u64(&mut self, i: u64) {
        self.0 ^= i;
        self.0 = self.0.wrapping_mul(0x100000001b3);
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::query::{ExactIdPolicy, QueryError, QuerySpec};
    use crate::world::WorldBuilder;
    use alloc::vec;

    #[derive(Clone, Copy)]
    struct Position(#[allow(dead_code)] i32);

    #[derive(Clone, Copy)]
    struct Velocity(#[allow(dead_code)] i32);

    #[derive(Clone, Copy)]
    struct Player;

    #[derive(Clone, Copy)]
    struct Ghost;

    fn world() -> World {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("pos");
        builder
            .register_component::<Velocity>(ComponentOptions::sparse())
            .expect("vel");
        builder
            .register_component::<Player>(ComponentOptions::tag())
            .expect("tag");
        builder.build().expect("build")
    }

    #[test]
    fn exact_ids_without_policy_is_rejected() {
        let mut world = world();
        let mut spec = QuerySpec::new();
        spec.exact_ids = Some(vec![]);
        assert!(matches!(
            world.resolve_query1_plan::<Position>(&spec),
            Err(QueryError::WrongQuery { .. })
        ));
        assert!(matches!(
            world.resolve_query2_plan::<Position, Velocity>(&spec),
            Err(QueryError::WrongQuery { .. })
        ));
    }

    #[test]
    fn added_and_changed_filters_conflict() {
        let mut world = world();
        let spec = QuerySpec::new().added::<Position>().changed::<Velocity>();
        assert!(matches!(
            world.resolve_query1_plan::<Position>(&spec),
            Err(QueryError::ConflictingFilters { .. })
        ));
        assert!(matches!(
            world.resolve_query2_plan::<Position, Velocity>(&spec),
            Err(QueryError::ConflictingFilters { .. })
        ));
    }

    #[test]
    fn overlapping_required_and_without_conflict() {
        let mut world = world();
        let spec = QuerySpec::new().with::<Position>().without::<Position>();
        assert!(matches!(
            world.resolve_query1_plan::<Position>(&spec),
            Err(QueryError::ConflictingFilters { .. })
        ));
    }

    #[test]
    fn query2_overlapping_tag_filters_conflict() {
        let mut world = world();
        let spec = QuerySpec::new()
            .with_tag::<Player>()
            .without_tag::<Player>();
        assert!(matches!(
            world.resolve_query2_plan::<Position, Velocity>(&spec),
            Err(QueryError::ConflictingFilters { .. })
        ));
    }

    #[test]
    fn unregistered_filter_type_is_rejected() {
        let mut world = world();
        let spec = QuerySpec::new().without::<Ghost>();
        assert!(matches!(
            world.resolve_query1_plan::<Position>(&spec),
            Err(QueryError::UnregisteredComponent { .. })
        ));
    }

    #[test]
    fn non_tag_with_tag_filter_is_wrong_storage_kind() {
        let mut world = world();
        let spec = QuerySpec::new().with_tag::<Position>();
        assert!(matches!(
            world.resolve_query1_plan::<Position>(&spec),
            Err(QueryError::WrongStorageKind { .. })
        ));
    }

    #[test]
    fn query2_exact_ids_use_exact_traversal() {
        let mut world = world();
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Position(1)).expect("insert");
        let spec = QuerySpec::new().exact_ids(vec![entity], ExactIdPolicy::SkipUnavailable);
        let (plan, _, _) = world
            .resolve_query2_plan::<Position, Velocity>(&spec)
            .expect("plan");
        assert!(matches!(plan.traversal, TraversalSource::Exact { .. }));
    }
}
