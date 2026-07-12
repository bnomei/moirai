#[test]
fn crate_links_as_no_std_alloc_library() {}

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
fn semantic_namespaces_are_not_prematurely_public() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/premature_component_namespace.rs");
    cases.compile_fail("tests/ui/premature_event_namespace.rs");
    cases.compile_fail("tests/ui/premature_query_namespace.rs");
    cases.compile_fail("tests/ui/premature_schedule_namespace.rs");
    cases.compile_fail("tests/ui/premature_world_namespace.rs");
    cases.compile_fail("tests/ui/premature_math_namespace.rs");
    cases.compile_fail("tests/ui/premature_diagnostics_namespace.rs");
}

#[test]
fn root_and_prelude_vocabulary_is_not_prematurely_public() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/premature_root_world.rs");
    cases.compile_fail("tests/ui/premature_root_app.rs");
    cases.compile_fail("tests/ui/premature_root_system.rs");
    cases.compile_fail("tests/ui/premature_prelude.rs");
}

#[cfg(feature = "std")]
#[test]
fn std_feature_is_additive() {}

#[cfg(feature = "testkit")]
#[test]
fn testkit_feature_compiles_private_module() {}
