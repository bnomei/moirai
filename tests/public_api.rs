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
fn phase_4_schedule_and_app_paths_compile() {
    use moirai::{
        stage, App, AppBuilder, Condition, FlushMode, Schedule, ScheduleBuilder, State, StateError,
        System, SystemSet, WorldTick,
    };
    let _ = stage::UPDATE;
    let _ = FlushMode::Final;
    let _ = core::mem::size_of::<System>();
    let _ = core::mem::size_of::<SystemSet>();
    let _ = core::mem::size_of::<Schedule>();
    let _ = core::mem::size_of::<ScheduleBuilder>();
    let _ = core::mem::size_of::<App>();
    let _ = core::mem::size_of::<AppBuilder>();
    let _ = core::mem::size_of::<State<u8>>();
    let _ = core::mem::size_of::<StateError>();
    let _ = WorldTick::ZERO;
    let condition = Condition::from_world(|_world| true);
    let set = SystemSet::new("set");
    let _ = System::new("system", stage::UPDATE, |_world, _dt| {})
        .before_set(&set)
        .after_set(&set)
        .run_if(condition);
    let mut app_builder = AppBuilder::new();
    app_builder.insert_resource(1u8).insert_state(2u16);
    app_builder
        .set_stage_flush_mode(stage::UPDATE, FlushMode::Stage)
        .expect("public stage flush authoring");
}

#[test]
fn phase_4_prelude_paths_compile() {
    use moirai::prelude::*;
    let _ = core::mem::size_of::<App>();
    let _ = core::mem::size_of::<System>();
}

#[test]
fn phase_5_query_namespace_paths_compile() {
    use moirai::query::{
        EntityRef, ExactIdPolicy, Query1, Query2, QueryCache, QueryCommands, QueryCursor,
        QueryEffects, QueryEntities, QueryError, QueryIds, QueryParams, QueryResultCache,
        QuerySpec,
    };
    let _ = core::mem::size_of::<ExactIdPolicy>();
    let _ = core::mem::size_of::<QuerySpec>();
    let _ = core::mem::size_of::<QueryParams<'_>>();
    let _ = core::mem::size_of::<Query1<'_, '_, ()>>();
    let _ = core::mem::size_of::<Query2<'_, '_, (), ()>>();
    let _ = core::mem::size_of::<QueryIds<'_, '_>>();
    let _ = core::mem::size_of::<QueryEntities<'_, '_>>();
    let _ = core::mem::size_of::<EntityRef<'_>>();
    let _ = core::mem::size_of::<QueryCache>();
    let _ = core::mem::size_of::<QueryResultCache>();
    let _ = core::mem::size_of::<QueryCursor>();
    let _ = core::mem::size_of::<QueryError>();
    let _ = core::mem::size_of::<QueryEffects<'_>>();
    let _ = core::mem::size_of::<QueryCommands<'_>>();
}

#[test]
fn entity_query_root_paths_compile() {
    use moirai::{EntityRef, QueryEntities, QueryIds};
    let _ = core::mem::size_of::<QueryIds<'_, '_>>();
    let _ = core::mem::size_of::<QueryEntities<'_, '_>>();
    let _ = core::mem::size_of::<EntityRef<'_>>();
}

#[test]
fn entity_scratch_root_and_world_paths_compile() {
    use moirai::world::{
        EntityScratch as WorldEntityScratch, EntityScratchError as WorldScratchError,
    };
    use moirai::{EntityScratch, EntityScratchError};

    let _ = core::mem::size_of::<EntityScratch<u8>>();
    let _ = core::mem::size_of::<EntityScratchError>();
    let _ = core::mem::size_of::<WorldEntityScratch<u8>>();
    let _ = core::mem::size_of::<WorldScratchError>();
}

#[test]
fn phase_3_event_namespace_paths_compile() {
    use moirai::event::{ComponentAdded, EventId, EventOptions, EventReader, EventReaderStart};
    let _ = EventOptions::manual();
    let _ = EventReaderStart::OldestRetained;
    let _ = core::mem::size_of::<EventId>();
    let _ = core::mem::size_of::<EventReader<ComponentAdded>>();
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
    cases.compile_fail("tests/ui/internal_event_storage.rs");
    cases.compile_fail("tests/ui/internal_schedule_runner.rs");
    cases.compile_fail("tests/ui/internal_world_query_plan.rs");
}

#[cfg(feature = "std")]
#[test]
fn std_error_types_expose_display_and_source() {
    use std::error::Error;

    let q16: &dyn Error = &moirai::math::Q16Error::OutOfRange;
    assert_eq!(q16.to_string(), "Q16 input is out of range");

    let world: &dyn Error = &moirai::world::WorldError::ChangeTickExhausted;
    assert_eq!(world.to_string(), "change tick exhausted");

    let scratch: &dyn Error = &moirai::EntityScratchError::WrongWorld;
    assert_eq!(
        scratch.to_string(),
        "entity scratch used with the wrong world"
    );
}

#[cfg(feature = "std")]
#[test]
fn std_feature_is_additive() {}

#[cfg(feature = "testkit")]
#[test]
fn testkit_namespace_paths_compile() {
    use moirai::testkit::{
        reports_match, CapturePolicy, MetricSample, ReplayConfig, ReplayFailure, ReplayReport,
        ReplayRunError, StepIndex, StepRecord, StepSnapshot,
    };
    let _ = core::mem::size_of::<CapturePolicy>();
    let _ = core::mem::size_of::<ReplayConfig>();
    let _ = core::mem::size_of::<MetricSample>();
    let _ = core::mem::size_of::<StepIndex>();
    let _ = core::mem::size_of::<ReplayReport<u8>>();
    let _ = core::mem::size_of::<ReplayFailure<ReplayRunError<()>, u8>>();
    let _ = core::mem::size_of::<ReplayRunError<()>>();
    let _ = core::mem::size_of::<StepRecord<u8>>();
    let _ = core::mem::size_of::<StepSnapshot<u8>>();
    let _: fn(&ReplayReport<u8>, &ReplayReport<u8>) -> bool = reports_match::<u8>;
}
