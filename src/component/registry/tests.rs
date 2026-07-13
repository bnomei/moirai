use core::any::TypeId;
use core::mem::{align_of, size_of};

use super::*;
use crate::component::{ComponentOptions, StorageKind};
use crate::world::WorldOwner;
use rstest::rstest;

struct Marker;
struct Position {
    _x: i32,
    _y: i32,
}
struct DroppingTag(#[allow(dead_code)] String);

#[rstest]
fn first_registration_succeeds() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let id = registry
        .register_typed::<Position>(&owner, None, ComponentOptions::sparse())
        .expect("first registration");
    assert_eq!(id.index(), 0);
}

#[rstest]
fn find_conflict_exact_repeat_returns_none() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("pos"), ComponentOptions::sparse())
        .expect("first");
    assert!(!registry.find_conflict_for_test(
        Some(TypeId::of::<Position>()),
        "pos",
        ComponentOptions::sparse(),
        size_of::<Position>(),
        align_of::<Position>(),
    ));
}

#[rstest]
fn exact_repeat_is_idempotent() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let first = registry
        .register_typed::<Position>(&owner, None, ComponentOptions::sparse())
        .expect("first");
    let second = registry
        .register_typed::<Position>(&owner, None, ComponentOptions::sparse())
        .expect("repeat");
    assert_eq!(first, second);
    assert_eq!(registry.len(), 1);
}

#[rstest]
fn conflicting_options_fail_without_mutation() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, None, ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_typed::<Position>(&owner, None, ComponentOptions::table())
        .expect_err("conflict");
    assert!(matches!(err, RegistrationError::TypeConflict { .. }));
    assert_eq!(registry.len(), 1);
}

#[rstest]
fn same_name_different_type_fails() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("shared"), ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_typed::<Marker>(&owner, Some("shared"), ComponentOptions::tag())
        .expect_err("name conflict");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn typed_non_zst_tag_fails() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let err = registry
        .register_typed::<Position>(&owner, None, ComponentOptions::tag())
        .expect_err("invalid tag");
    assert!(matches!(err, RegistrationError::InvalidTag { .. }));
}

#[rstest]
fn untyped_registration_requires_tag_options() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let err = registry
        .register_untyped(&owner, "marker", ComponentOptions::sparse())
        .expect_err("untyped sparse");
    assert!(matches!(err, RegistrationError::LayoutConflict { .. }));
}

#[rstest]
fn dropping_tag_fails() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let err = registry
        .register_typed::<DroppingTag>(&owner, None, ComponentOptions::tag())
        .expect_err("dropping tag");
    assert!(matches!(err, RegistrationError::InvalidTag { .. }));
}

#[rstest]
fn component_id_rejects_foreign_owner() {
    let owner_a = WorldOwner::new();
    let owner_b = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let id = registry
        .register_typed::<Marker>(&owner_a, None, ComponentOptions::tag())
        .expect("register");
    assert!(matches!(
        id.validate_owner(&owner_b),
        Err(RegistrationError::LayoutConflict { .. })
    ));
}

#[rstest]
fn storage_kind_and_lookup_helpers() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let sparse = registry
        .register_typed::<Position>(&owner, Some("pos"), ComponentOptions::sparse())
        .expect("sparse");
    let tag = registry
        .register_typed::<Marker>(&owner, Some("marker"), ComponentOptions::tag())
        .expect("tag");

    assert_eq!(registry.storage_kind(&sparse), Some(StorageKind::Sparse));
    assert_eq!(registry.storage_kind(&tag), Some(StorageKind::Sparse));
    assert_eq!(registry.is_tag(&tag), Some(true));
    assert!(registry.entry_is_tag(tag.index()));
    assert_eq!(
        registry.index_of_type(core::any::TypeId::of::<Position>()),
        Some(sparse.index())
    );
    assert_eq!(registry.id_of::<Position>(&owner), Some(sparse.clone()));
    assert_eq!(registry.component_name(&sparse), "pos");
}

#[rstest]
fn tag_table_storage_is_rejected() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let err = registry
        .register_typed::<Marker>(&owner, None, ComponentOptions::test_tag_table())
        .expect_err("tag table");
    assert!(matches!(err, RegistrationError::UnsupportedStorage { .. }));
}

#[rstest]
fn untyped_tag_table_storage_is_rejected() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let err = registry
        .register_untyped(&owner, "marker", ComponentOptions::test_tag_table())
        .expect_err("untyped tag table");
    assert!(matches!(err, RegistrationError::UnsupportedStorage { .. }));
}

#[rstest]
fn layout_conflict_reports_detail() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("pos"), ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_typed::<Position>(&owner, Some("other"), ComponentOptions::sparse())
        .expect_err("layout");
    assert!(matches!(err, RegistrationError::TypeConflict { .. }));
}

#[rstest]
fn layout_conflict_reports_existing_and_requested_layout() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("pos"), ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_inner(
            &owner,
            "pos-alias".into(),
            Some(TypeId::of::<Position>()),
            ComponentOptions::sparse(),
            size_of::<Position>() + 1,
            align_of::<Position>(),
        )
        .expect_err("layout");
    assert!(matches!(err, RegistrationError::LayoutConflict { .. }));
}

#[rstest]
fn type_conflict_detects_same_type_different_name() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("first"), ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_typed::<Position>(&owner, Some("second"), ComponentOptions::sparse())
        .expect_err("type");
    assert!(matches!(err, RegistrationError::TypeConflict { .. }));
}

#[rstest]
fn name_conflict_detects_same_name_different_type() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("shared"), ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_typed::<Marker>(&owner, Some("shared"), ComponentOptions::tag())
        .expect_err("name");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn type_conflict_with_renamed_repeat() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("a"), ComponentOptions::sparse())
        .expect("first");
    let err = registry
        .register_typed::<Position>(&owner, Some("b"), ComponentOptions::sparse())
        .expect_err("rename");
    assert!(matches!(err, RegistrationError::TypeConflict { .. }));
}

#[rstest]
fn name_conflict_detects_matching_name_with_different_tag_kind() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("shared"), ComponentOptions::sparse())
        .expect("sparse");
    let err = registry
        .register_inner(
            &owner,
            "shared".into(),
            Some(TypeId::of::<Marker>()),
            ComponentOptions::tag(),
            0,
            1,
        )
        .expect_err("tag");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn type_conflict_same_type_different_storage_kind() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("velocity"), ComponentOptions::sparse())
        .expect("sparse");
    let err = registry
        .register_inner(
            &owner,
            "velocity".into(),
            Some(TypeId::of::<Position>()),
            ComponentOptions::table(),
            size_of::<Position>(),
            align_of::<Position>(),
        )
        .expect_err("storage");
    assert!(matches!(err, RegistrationError::TypeConflict { .. }));
}

#[rstest]
fn name_conflict_untyped_same_tag_kind_different_size() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_inner(&owner, "shared".into(), None, ComponentOptions::tag(), 0, 1)
        .expect("tag");
    let err = registry
        .register_inner(&owner, "shared".into(), None, ComponentOptions::tag(), 8, 8)
        .expect_err("size");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn name_conflict_untyped_same_tag_kind_different_align() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_inner(&owner, "shared".into(), None, ComponentOptions::tag(), 0, 1)
        .expect("tag");
    let err = registry
        .register_inner(&owner, "shared".into(), None, ComponentOptions::tag(), 0, 4)
        .expect_err("align");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn name_conflict_untyped_sparse_and_tag_share_name() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_inner(&owner, "shared".into(), None, ComponentOptions::tag(), 0, 1)
        .expect("tag");
    let err = registry
        .register_inner(
            &owner,
            "shared".into(),
            None,
            ComponentOptions::sparse(),
            0,
            1,
        )
        .expect_err("sparse");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn name_conflict_same_name_different_storage_on_name_branch() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("shared"), ComponentOptions::sparse())
        .expect("sparse");
    let err = registry
        .register_inner(
            &owner,
            "shared".into(),
            Some(TypeId::of::<Marker>()),
            ComponentOptions::table(),
            size_of::<Marker>(),
            align_of::<Marker>(),
        )
        .expect_err("storage");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}

#[rstest]
fn name_conflict_detects_matching_name_with_different_layout() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    registry
        .register_typed::<Position>(&owner, Some("shared"), ComponentOptions::sparse())
        .expect("sparse");
    let err = registry
        .register_inner(
            &owner,
            "shared".into(),
            Some(TypeId::of::<Marker>()),
            ComponentOptions::sparse(),
            size_of::<Marker>(),
            align_of::<Marker>(),
        )
        .expect_err("layout");
    assert!(matches!(err, RegistrationError::NameConflict { .. }));
}
