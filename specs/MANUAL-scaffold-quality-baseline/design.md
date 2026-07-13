# Design — Moirai single-crate scaffold and quality baseline

## Objective

Create the permanent Cargo, visibility, and validation envelope for the one-crate Moirai ECS. This
spec establishes names and boundaries only; storage, World behavior, scheduling, queries, testkit,
and downstream adapters remain with their owning phases.

## Scope and non-goals

In scope: `Cargo.toml`, no-std crate attributes, the approved private module tree, crate-boundary
privacy tests, README/doctest truthfulness, benchmark harness wiring, and CI checks.

Out of scope: any entity allocation, component/resource/event storage, command behavior, schedule
execution, query behavior, numeric implementation, Wyrd/Anapao adapter, or fake runnable example.

## Frozen contract

Moirai is one Rust 2021 package with `rust-version = "1.75"` and these exact features:

```toml
[features]
default = []
std = []
testkit = []
```

The crate root is permanently `#![no_std]`, imports `alloc`, and forbids unsafe code. `std` and
`testkit` are additive; no feature selects a global numeric representation.

Phase 1 publishes no semantic namespace, root re-export, prelude, or extension trait. It creates the
private physical module tree and documents the final target paths. Component/math names begin in
Phase 2, World/event/Bundle in Phase 3, App/schedule/diagnostics/Observer in Phase 4, query in
Phase 5, and `testkit` in Phase 6. `app`, `command`, `entity`, `operation`, `resource`, `state`,
`storage`, and `time` remain private physical modules; implementation children such as allocator,
registry, erased storage, queues, runner, and query plans remain private.

The root vocabulary is `App`, `AppBuilder`, `AppError`, `AppFault`, `BuildError`, `Commands`, typed
component/event/query/schedule/time names, `StageOperation`, `World`, `WorldBuilder`, `Bundle`, and
`DynamicBundle`. The prelude contains only system-authoring essentials: App/AppBuilder, World,
WorldTick, EntityId, Commands, DynamicBundle, component options/storage kind, System/SystemSet,
FlushMode, query spec/params, and State.

Phase 1 does not declare future-owned public types or traits. `Bundle`/`BundleWriter` and
`diagnostics::Observer` are downstream extension contracts and must arrive with their exact owning
signatures, not an empty trait or guessed method. API tests prove the public boundary has no leaks;
runnable authoring, root/prelude imports, and doctests wait for the owning phase. Later phases add
real behavior behind the documented final-path contract.

## Delivery sequence

```text
manifest + crate attributes
  → private physical module tree + crate-boundary docs
  → privacy/README compile-fail tests
  → CI, docs, benchmark harness, package visibility review
```

## Verification plan

Run format, strict all-feature Clippy, core/std/testkit/all-feature tests, all-feature check,
warnings-denied docs, benchmark-harness compilation, and the Rust 1.75 no-default library check.
Inspect rustdoc to confirm storage and runner internals remain absent. An independent review
validates the public paths and dependency graph before completion.

| Requirement | Task(s) | Validation | Risk control |
| --- | --- | --- | --- |
| R001–R002 | T001 | core/all-feature/MSRV checks | exact manifest and crate attributes |
| R003–R004 | T002 | rustdoc/privacy review | no premature public declarations |
| R005 | T003 | compile-fail docs/tests | no leak or fake quickstart |
| R006 | T004 | workflow syntax and local commands | toolchain separation |
| R001–R007 | T005 | independent full review | no accidental API/dependency leak |
