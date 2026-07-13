//! `std` Display and `Error::source` coverage for public error types.
#![cfg(feature = "std")]

use std::error::Error;

use moirai::component::ComponentOptions;
use moirai::component::RegistrationError;
use moirai::event::EventRegistrationError;
use moirai::math::Q16Error;
use moirai::query::QueryError;
use moirai::schedule::{BuildError, ScheduleError};
use moirai::world::WorldBuilder;
use moirai::world::{EventReadError, FlushError, WorldAllocatorError, WorldError};

fn assert_display<T: core::fmt::Display>(value: &T, expected: impl AsRef<str>) {
    assert_eq!(value.to_string(), expected.as_ref());
}

#[test]
fn q16_error_display_variants() {
    assert_display(&Q16Error::Overflow, "Q16 overflow");
    assert_display(&Q16Error::Underflow, "Q16 underflow");
    assert_display(&Q16Error::DivisionByZero, "Q16 division by zero");
    assert_display(&Q16Error::NotFinite, "Q16 input is not finite");
    assert_display(&Q16Error::OutOfRange, "Q16 input is out of range");
}

#[test]
fn world_allocator_error_display_variants() {
    assert_display(
        &WorldAllocatorError::GenerationOverflow,
        "entity generation overflow",
    );
    assert_display(&WorldAllocatorError::SlotRetired, "entity slot retired");
}

#[test]
fn flush_error_display_variants() {
    assert_display(
        &FlushError::CommandValidation {
            index: 2,
            detail: "stale entity".into(),
        },
        "command 2 failed validation: stale entity",
    );
    assert_display(
        &FlushError::ChangeTickExhausted,
        "change tick exhausted during flush",
    );
}

#[test]
fn world_error_display_variants() {
    let mut world = WorldBuilder::new().build().expect("world");
    let entity = world.spawn().expect("spawn");
    world.despawn(entity).expect("despawn");
    assert_display(&WorldError::StaleEntity { entity }, "stale entity 0:1");
    assert_display(
        &WorldError::EntityNotLive { entity },
        "entity 0:1 is not live",
    );
    assert_display(
        &WorldError::UnregisteredComponent { name: "Pos".into() },
        "unregistered component Pos",
    );
    assert_display(
        &WorldError::WrongStorageKind { name: "Pos".into() },
        "wrong storage kind for Pos",
    );
    assert_display(
        &WorldError::Registration(RegistrationError::LayoutConflict {
            name: "Pos".into(),
            detail: "size mismatch".into(),
        }),
        "component registration failed: component layout conflict for Pos: size mismatch",
    );
    assert_display(
        &WorldError::Allocator(WorldAllocatorError::SlotRetired),
        "entity allocator failed: entity slot retired",
    );
    assert_display(&WorldError::ChangeTickExhausted, "change tick exhausted");
    assert_display(
        &WorldError::StructuralMutationDuringRun,
        "structural mutation is deferred while the world is running",
    );
    assert_display(
        &WorldError::StructuralCommandsDuringRender,
        "structural commands are unavailable during render",
    );
    assert_display(&WorldError::FlushDuringRun, "flush is idle-only");
    assert_display(
        &WorldError::DiscardDuringRun,
        "discard_commands is idle-only",
    );
    assert_display(
        &WorldError::Flush(FlushError::ChangeTickExhausted),
        "flush failed: change tick exhausted during flush",
    );
    assert_display(
        &WorldError::UnregisteredResource {
            name: "Score".into(),
        },
        "unregistered resource Score",
    );
    assert_display(
        &WorldError::ResourceInUse {
            name: "Score".into(),
        },
        "resource Score is in use",
    );
    assert_display(
        &WorldError::ResourceScoped {
            name: "Score".into(),
        },
        "resource Score is scoped",
    );
    assert_display(
        &WorldError::UnregisteredEvent {
            name: "Damage".into(),
        },
        "unregistered event Damage",
    );
    assert_display(&WorldError::EventChannelClosed, "event channel is closed");
    assert_display(
        &WorldError::NestedRun,
        "nested world execution is not supported",
    );
}

#[test]
fn world_error_source_chains() {
    let registration: &dyn Error = &WorldError::Registration(RegistrationError::LayoutConflict {
        name: "Pos".into(),
        detail: "size".into(),
    });
    assert!(registration.source().is_some());

    let allocator: &dyn Error = &WorldError::Allocator(WorldAllocatorError::GenerationOverflow);
    assert!(allocator.source().is_some());

    let flush: &dyn Error = &WorldError::Flush(FlushError::ChangeTickExhausted);
    assert!(flush.source().is_some());

    let leaf: &dyn Error = &WorldError::NestedRun;
    assert!(leaf.source().is_none());
}

#[test]
fn event_read_error_display_variants() {
    assert_display(
        &EventReadError::Lagged { dropped: 3 },
        "event reader lagged by 3 events",
    );
    assert_display(&EventReadError::ChannelClosed, "event channel is closed");
    assert_display(
        &EventReadError::UnregisteredEvent {
            name: "Damage".into(),
        },
        "unregistered event Damage",
    );
    assert_display(
        &EventReadError::OwnerMismatch {
            name: "reader".into(),
        },
        "event reader owner mismatch for reader",
    );
}

#[derive(Clone, Copy)]
struct Tag;

#[test]
fn query_error_display_variants() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Tag>(ComponentOptions::tag())
        .expect("register");
    let mut world = builder.build().expect("world");
    let entity = world.spawn().expect("spawn");
    assert_display(
        &QueryError::UnregisteredComponent { name: "Pos".into() },
        "unregistered component 'Pos'",
    );
    assert_display(
        &QueryError::WrongStorageKind { name: "Pos".into() },
        "wrong storage kind for 'Pos'",
    );
    assert_display(
        &QueryError::ConflictingFilters {
            detail: "added+changed".into(),
        },
        "conflicting filters: added+changed",
    );
    assert_display(
        &QueryError::DuplicateMutableComponent { name: "Pos".into() },
        "duplicate mutable component 'Pos'",
    );
    assert_display(
        &QueryError::WrongOwner,
        "query handle belongs to another world",
    );
    assert_display(&QueryError::StaleCache, "stale query cache handle");
    assert_display(
        &QueryError::WrongQuery {
            detail: "cursor".into(),
        },
        "wrong query cursor: cursor",
    );
    assert_display(
        &QueryError::MovingChangeWindow,
        "added/changed filters require QueryCache, not QueryResultCache",
    );
    assert_display(
        &QueryError::UnsupportedCachePolicy {
            detail: "exact".into(),
        },
        "unsupported cache policy: exact",
    );
    assert_display(
        &QueryError::ExactIdOrderConflict,
        "exact-id order conflicts with result cache",
    );
    assert_display(
        &QueryError::MissingExactId { entity },
        format!("exact-id query missing unavailable entity {entity:?}"),
    );
    assert_display(
        &QueryError::BorrowConflict {
            detail: "mutable".into(),
        },
        "query borrow conflict: mutable",
    );
    assert_display(&QueryError::OwnerMismatch, "query handle owner mismatch");
    assert_display(
        &QueryError::TraversalAborted {
            entity,
            detail: "poison".into(),
        },
        format!("query traversal aborted at {entity:?}: poison"),
    );
}

#[test]
fn schedule_error_display_variants() {
    assert_display(&BuildError::PendingCommands, "world has pending commands");
    assert_display(&BuildError::WorldRunning, "world is running");
    assert_display(
        &BuildError::WorldMutationPoisoned,
        "world mutation is poisoned",
    );
    assert_display(
        &BuildError::LeaseMismatch,
        "world and schedule execution lease mismatch",
    );
    assert_display(
        &BuildError::LiveLeaseAlreadyAttached,
        "world already has a live compiled schedule lease",
    );
    assert_display(
        &BuildError::UnknownStage {
            label: "Foo".into(),
        },
        "unknown stage 'Foo'",
    );
    assert_display(
        &BuildError::UnknownSystem {
            label: "bar".into(),
        },
        "unknown system 'bar'",
    );
    assert_display(
        &BuildError::UnknownSystemSet {
            label: "set".into(),
        },
        "unknown system set 'set'",
    );
    assert_display(
        &BuildError::DuplicateSystemSet {
            label: "set".into(),
        },
        "duplicate system set 'set'",
    );
    assert_display(
        &BuildError::DuplicateSystemLabel {
            label: "dup".into(),
        },
        "duplicate system label 'dup'",
    );
    assert_display(
        &BuildError::CrossStageSystemEdge {
            from: "a".into(),
            to: "b".into(),
        },
        "cross-stage system edge: a -> b",
    );
    assert_display(
        &BuildError::MissingRequiredResource {
            name: "State".into(),
        },
        "missing required resource 'State'",
    );
    assert_display(
        &BuildError::CrossOperationEdge {
            from: "update".into(),
            to: "render".into(),
        },
        "ordering edge crosses operations: update -> render",
    );
    assert_display(
        &BuildError::SelfEdge {
            label: "loop".into(),
        },
        "system cannot depend on itself: loop",
    );
    assert_display(
        &BuildError::Cycle {
            path: vec!["a".into(), "b".into(), "a".into()],
        },
        "schedule cycle: a -> b -> a",
    );
    assert_display(
        &BuildError::FixedUpdateWithoutConfig,
        "FixedUpdate systems require fixed configuration",
    );
    assert_display(
        &BuildError::FixedConfigWithoutFixedUpdate,
        "fixed configuration requires a FixedUpdate stage",
    );
    assert_display(
        &BuildError::StageOperationMismatch {
            label: "Render".into(),
        },
        "stage operation mismatch for 'Render'",
    );
    assert_display(
        &BuildError::WorldBuild(WorldError::NestedRun),
        "world build failed: nested world execution is not supported",
    );

    assert_display(
        &ScheduleError::OwnerMismatch,
        "schedule handle belongs to a different schedule",
    );
    assert_display(&ScheduleError::StaleHandle, "stale schedule handle");
    assert_display(
        &ScheduleError::SystemNotFound {
            label: "missing".into(),
        },
        "system not found: missing",
    );
}

#[test]
fn app_error_display_variants() {
    use moirai::AppError;

    assert_display(&AppError::InvalidDelta, "invalid delta");
    assert_display(&AppError::PendingIdleCommands, "pending idle commands");
    assert_display(&AppError::WorldMutationPoisoned, "world mutation poisoned");
    assert_display(&AppError::TerminalFault, "terminal app fault");
    assert_display(&AppError::WorldTickExhausted, "world tick exhausted");
    assert_display(&AppError::FixedStepExhausted, "fixed step exhausted");
    assert_display(
        &AppError::Fault(moirai::AppFault {
            stage: None,
            system: None,
            detail: Some("boom".into()),
        }),
        "app execution fault",
    );
}

#[test]
fn registration_error_display_variants() {
    assert_display(
        &RegistrationError::TypeConflict {
            name: "Pos".into(),
            existing: "pos".into(),
            requested: "position".into(),
        },
        "component type conflict for Pos: existing=pos, requested=position",
    );
    assert_display(
        &RegistrationError::NameConflict {
            name: "shared".into(),
            existing: "a".into(),
            requested: "b".into(),
        },
        "component name conflict for shared: existing=a, requested=b",
    );
    assert_display(
        &RegistrationError::LayoutConflict {
            name: "Pos".into(),
            detail: "size mismatch".into(),
        },
        "component layout conflict for Pos: size mismatch",
    );
    assert_display(
        &RegistrationError::InvalidTag {
            name: "Tag".into(),
            detail: "not zst".into(),
        },
        "invalid tag component Tag: not zst",
    );
    assert_display(
        &RegistrationError::UnsupportedStorage {
            name: "Tag".into(),
            detail: "no table".into(),
        },
        "unsupported storage for Tag: no table",
    );
}

#[test]
fn event_registration_error_display_variants() {
    assert_display(
        &EventRegistrationError::TypeConflict {
            name: "Damage".into(),
            existing: "Old".into(),
            requested: "New".into(),
        },
        "event registration conflict for Damage: existing=Old, requested=New",
    );
    assert_display(
        &EventRegistrationError::InvalidCapacity,
        "event retention capacity must be nonzero",
    );
}
