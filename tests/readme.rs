use std::fs;

#[test]
fn readme_does_not_advertise_a_runnable_ecs_quickstart() {
    let readme = fs::read_to_string("README.md").expect("README.md should exist");
    assert!(
        !readme.contains("```rust"),
        "README must not include runnable Rust quickstart snippets before Phase 4"
    );
    assert!(
        readme.contains("docs/ARCHITECTURE.md"),
        "README should point readers at the architecture contract"
    );
}
