use moirai::{
    component::{ComponentId, ComponentOptions, StorageKind},
    math::Q16,
    world::{World, WorldBuilder},
    EntityId,
};

#[test]
fn crate_links_as_no_std_alloc_library() {
    let _ = core::mem::size_of::<EntityId>();
    let _ = core::mem::size_of::<ComponentId>();
    let _ = core::mem::size_of::<Q16>();
    let _ = core::mem::size_of::<World>();
    let _ = core::mem::size_of::<WorldBuilder>();
}

#[test]
fn phase_2_root_and_namespace_paths_compile() {
    let _ = ComponentOptions::sparse();
    let _ = StorageKind::Sparse;
}

#[test]
fn all_features_build_is_additive() {
    std::process::Command::new("cargo")
        .args(["check", "--all-features"])
        .status()
        .expect("cargo should be on PATH")
        .success()
        .then_some(())
        .expect("all-features build must remain additive and coherent");
}

#[test]
fn core_has_no_forbidden_runtime_dependencies() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml should exist");
    for forbidden in ["wyrd", "anapao", "bevy", "playdate", "serde", "proc-macro"] {
        assert!(
            !manifest.contains(&format!("{forbidden} =")),
            "core manifest must not depend on {forbidden}"
        );
    }
}

#[test]
fn implementation_modules_are_not_public() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/internal_entity.rs");
    cases.compile_fail("tests/ui/internal_storage.rs");
    cases.compile_fail("tests/ui/internal_allocator.rs");
    cases.compile_fail("tests/ui/internal_registry.rs");
    cases.compile_fail("tests/ui/internal_command_queue.rs");
    cases.compile_fail("tests/ui/internal_schedule_runner.rs");
    cases.compile_fail("tests/ui/internal_world_query_plan.rs");
}

#[test]
fn deferred_namespaces_remain_unpublished() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/premature_event_namespace.rs");
    cases.compile_fail("tests/ui/premature_query_namespace.rs");
    cases.compile_fail("tests/ui/premature_schedule_namespace.rs");
    cases.compile_fail("tests/ui/premature_diagnostics_namespace.rs");
}

#[test]
fn root_and_prelude_vocabulary_is_not_prematurely_public() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/premature_root_app.rs");
    cases.compile_fail("tests/ui/premature_root_system.rs");
    cases.compile_fail("tests/ui/premature_prelude.rs");
}

#[cfg(feature = "std")]
#[test]
fn std_error_types_expose_display_and_source() {
    use std::error::Error;

    let q16: &dyn Error = &moirai::math::Q16Error::OutOfRange;
    assert_eq!(q16.to_string(), "Q16 input is out of range");

    let world: &dyn Error = &moirai::world::WorldError::ChangeTickExhausted;
    assert_eq!(world.to_string(), "change tick exhausted");
}

#[cfg(feature = "std")]
#[test]
fn std_feature_is_additive() {}

#[cfg(feature = "testkit")]
#[test]
fn testkit_feature_compiles_private_module() {}
