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
    assert!(readme.contains("no downstream migration or\nperformance result is claimed"));
}

#[test]
fn readme_quickstart_contract_executes() {
    use moirai::prelude::*;
    use moirai::stage;

    #[derive(Debug, PartialEq)]
    struct Counter(u32);

    let mut builder = AppBuilder::new();
    builder.insert_resource(Counter(0));
    builder
        .add_system(System::new("increment", stage::UPDATE, |world, _dt| {
            world
                .resource_mut::<Counter>()
                .expect("registered")
                .expect("seeded")
                .0 += 1;
        }))
        .expect("system");
    let mut app = builder.build().expect("app");
    app.update(1.0 / 60.0).expect("update");
    assert_eq!(
        app.world().resource::<Counter>().unwrap(),
        Some(&Counter(1))
    );
}
