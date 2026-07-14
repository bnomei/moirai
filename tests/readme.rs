use std::fs;

#[test]
fn readme_advertises_the_landed_quickstart_without_cutover_claims() {
    let readme = fs::read_to_string("README.md").expect("README.md should exist");
    assert!(
        readme.contains("```rust") && readme.contains("AppBuilder::new()"),
        "README must include the landed runnable Rust quickstart"
    );
    assert!(
        readme.contains("docs/ARCHITECTURE.md"),
        "README should point readers at the architecture contract"
    );
    assert!(
        readme.contains("moirai/examples/index.html"),
        "README should point readers at the canonical Rustdoc examples"
    );
    assert!(readme.contains("no downstream migration or\nperformance result is claimed"));
}

#[test]
fn integration_contract_docs_track_the_landed_owner_and_authoring_paths() {
    let architecture =
        fs::read_to_string("docs/ARCHITECTURE.md").expect("architecture doc should exist");
    let scratch_task = fs::read_to_string("specs/002-moirai-integration-readiness/tasks/T008.md")
        .expect("completed scratch task should exist");

    assert!(architecture.contains("AppBuilder::world_builder().register_component::<T>(options)"));
    assert!(architecture.contains("Condition::in_state(value)"));
    assert!(architecture.contains("Condition::state_changed::<S>()"));
    assert!(architecture.contains("opaque 16-byte Copy handle"));
    assert!(architecture.contains("private `u32` World owner token"));
    assert!(!architecture.contains("`System::in_state`"));
    assert!(!architecture.contains("opaque 8-byte Copy handle"));
    assert!(!architecture.contains("`Rc<WorldOwner>`"));

    assert!(scratch_task.contains("carry a private owner token"));
    assert!(!scratch_task.contains("do not carry an owner token"));
}
