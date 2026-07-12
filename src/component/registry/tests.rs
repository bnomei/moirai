use super::*;
use crate::component::ComponentOptions;
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
fn dropping_tag_fails() {
    let owner = WorldOwner::new();
    let mut registry = ComponentRegistry::new();
    let err = registry
        .register_typed::<DroppingTag>(&owner, None, ComponentOptions::tag())
        .expect_err("dropping tag");
    assert!(matches!(err, RegistrationError::InvalidTag { .. }));
}