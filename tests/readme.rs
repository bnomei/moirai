use std::fs;

#[test]
fn readme_presents_the_public_crate_without_local_planning_links() {
    let readme = fs::read_to_string("README.md").expect("README.md should exist");
    assert!(
        readme.contains("```rust") && readme.contains("AppBuilder::new()"),
        "README must include the landed runnable Rust quickstart"
    );
    assert!(
        readme.contains("no_std + alloc"),
        "README should state the default runtime environment"
    );
    assert!(
        readme.contains("docs.rs/moirai-for-games/latest/moirai/examples/index.html"),
        "README should point readers at the canonical Rustdoc examples"
    );
    assert!(
        readme.contains("cargo add moirai-for-games --rename moirai"),
        "README should install the published package under the public moirai crate name"
    );
    for local_path in ["docs/", "specs/", ".orchid/", "PHASE_", "ROADMAP.md"] {
        assert!(
            !readme.contains(local_path),
            "README must not link to local-only planning path {local_path}"
        );
    }
}
