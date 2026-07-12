use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{type_name, TypeId};
use core::hash::{Hash, Hasher};

use crate::component::ComponentId;
use crate::query::{ExactIdPolicy, QueryError, QuerySpec};
use crate::world::World;

use super::plan::{ResolvedPlan, TraversalSource};

pub(crate) fn resolve_query1<T: 'static>(
    world: &World,
    spec: &QuerySpec,
) -> Result<ResolvedPlan, QueryError> {
    let primary = resolve_component::<T>(world)?;
    let primary_index = primary.index();
    let primary_is_table = world.registry_is_table(&primary);

    let mut required = resolve_type_ids(world, &spec.required)?;
    if !required.contains(&primary_index) {
        required.push(primary_index);
    }
    required.sort_unstable();
    required.dedup();

    let without = resolve_type_ids(world, &spec.without)?;
    let with_tags = resolve_tag_type_ids(world, &spec.with_tags)?;
    let without_tags = resolve_tag_type_ids(world, &spec.without_tags)?;

    if spec.exact_ids.is_some() && spec.exact_id_policy.is_none() {
        return Err(QueryError::WrongQuery {
            detail: String::from("exact-id queries require an explicit ExactIdPolicy"),
        });
    }

    validate_no_overlap(&required, &without, "required", "without")?;
    validate_no_overlap(&with_tags, &without_tags, "with_tag", "without_tag")?;

    let added_index = spec
        .added
        .map(|type_id| resolve_type_id(world, type_id))
        .transpose()?
        .map(|id| id.index());
    let changed_index = spec
        .changed
        .map(|type_id| resolve_type_id(world, type_id))
        .transpose()?
        .map(|id| id.index());

    if added_index.is_some() && changed_index.is_some() {
        return Err(QueryError::ConflictingFilters {
            detail: String::from("added and changed filters are mutually exclusive"),
        });
    }

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
        &required,
        &without,
        &with_tags,
        &without_tags,
        added_index,
        changed_index,
        &traversal,
        primary_index,
        spec.exact_id_policy,
    );

    Ok(ResolvedPlan {
        fingerprint,
        primary_index,
        primary_is_table,
        traversal,
        required_indices: required,
        without_indices: without,
        with_tag_indices: with_tags,
        without_tag_indices: without_tags,
        added_index,
        changed_index,
        exact_id_policy: spec.exact_id_policy,
    })
}

pub(crate) fn resolve_query2<A: 'static, B: 'static>(
    world: &World,
    spec: &QuerySpec,
) -> Result<(ResolvedPlan, usize, bool), QueryError> {
    let primary_a = resolve_component::<A>(world)?;
    let primary_b = resolve_component::<B>(world)?;
    let second_index = primary_b.index();
    let second_is_table = world.registry_is_table(&primary_b);

    let mut required = resolve_type_ids(world, &spec.required)?;
    for index in [primary_a.index(), primary_b.index()] {
        if !required.contains(&index) {
            required.push(index);
        }
    }
    required.sort_unstable();
    required.dedup();

    let without = resolve_type_ids(world, &spec.without)?;
    let with_tags = resolve_tag_type_ids(world, &spec.with_tags)?;
    let without_tags = resolve_tag_type_ids(world, &spec.without_tags)?;

    if spec.exact_ids.is_some() && spec.exact_id_policy.is_none() {
        return Err(QueryError::WrongQuery {
            detail: String::from("exact-id queries require an explicit ExactIdPolicy"),
        });
    }

    validate_no_overlap(&required, &without, "required", "without")?;
    validate_no_overlap(&with_tags, &without_tags, "with_tag", "without_tag")?;

    let added_index = spec
        .added
        .map(|type_id| resolve_type_id(world, type_id))
        .transpose()?
        .map(|id| id.index());
    let changed_index = spec
        .changed
        .map(|type_id| resolve_type_id(world, type_id))
        .transpose()?
        .map(|id| id.index());

    if added_index.is_some() && changed_index.is_some() {
        return Err(QueryError::ConflictingFilters {
            detail: String::from("added and changed filters are mutually exclusive"),
        });
    }

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
        &required,
        &without,
        &with_tags,
        &without_tags,
        added_index,
        changed_index,
        &traversal,
        primary_index,
        spec.exact_id_policy,
    );

    let plan = ResolvedPlan {
        fingerprint,
        primary_index,
        primary_is_table,
        traversal,
        required_indices: required,
        without_indices: without,
        with_tag_indices: with_tags,
        without_tag_indices: without_tags,
        added_index,
        changed_index,
        exact_id_policy: spec.exact_id_policy,
    };

    Ok((plan, second_index, second_is_table))
}

fn resolve_component<T: 'static>(world: &World) -> Result<ComponentId, QueryError> {
    world
        .registry_id_of::<T>()
        .ok_or_else(|| QueryError::UnregisteredComponent {
            name: String::from(type_name::<T>()),
        })
}

fn resolve_type_ids(world: &World, type_ids: &[TypeId]) -> Result<Vec<usize>, QueryError> {
    type_ids
        .iter()
        .map(|&type_id| resolve_type_id(world, type_id).map(|id| id.index()))
        .collect()
}

fn resolve_tag_type_ids(world: &World, type_ids: &[TypeId]) -> Result<Vec<usize>, QueryError> {
    type_ids
        .iter()
        .map(|&type_id| {
            let id = resolve_type_id(world, type_id)?;
            if !world.is_tag_component(&id) {
                return Err(QueryError::WrongStorageKind {
                    name: world.registry_component_name(&id),
                });
            }
            Ok(id.index())
        })
        .collect()
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
    added: Option<usize>,
    changed: Option<usize>,
    traversal: &TraversalSource,
    primary: usize,
    exact_id_policy: Option<ExactIdPolicy>,
) -> u64 {
    let mut hasher = FnvHasher::new();
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
        TraversalSource::Sparse { component_index } => {
            0u8.hash(&mut hasher);
            component_index.hash(&mut hasher);
        }
        TraversalSource::Table { component_index } => {
            1u8.hash(&mut hasher);
            component_index.hash(&mut hasher);
        }
        TraversalSource::Exact { ids } => {
            2u8.hash(&mut hasher);
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
