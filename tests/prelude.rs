#[test]
fn prelude_namespace_is_not_published_in_phase_1() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/premature_prelude.rs");
}
