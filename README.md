# Moirai

[![Crates.io Version](https://img.shields.io/crates/v/moirai-for-games)](https://crates.io/crates/moirai-for-games)
[![Crates.io Downloads](https://img.shields.io/crates/d/moirai-for-games)](https://crates.io/crates/moirai-for-games)
[![CI](https://img.shields.io/github/actions/workflow/status/bnomei/moirai/ci.yml?branch=main&label=CI)](https://github.com/bnomei/moirai/actions/workflows/ci.yml)
[![docs.rs](https://img.shields.io/docsrs/moirai-for-games)](https://docs.rs/moirai-for-games)
[![MSRV](https://img.shields.io/badge/MSRV-1.75-blue)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

Moirai is a small, deterministic, single-threaded entity-component-system library for constrained
and headless games. It combines checked world construction, validated schedules, prepared queries,
typed events, fixed-step execution, diagnostics, and replay support in a `no_std + alloc` crate.

**Status:** Moirai 0.1 is pre-1.0. The core API is usable, but minor `0.x` releases may make
breaking public-API changes. CI checks the feature matrix, Rust 1.75 support, Rustdoc, dependency
policy, source coverage, and benchmark compilation.

**Single-threaded** · **`no_std + alloc` by default** · **zero runtime dependencies** ·
**checked schedules** · **deterministic replay**

## When Moirai fits

Use Moirai when you want an ECS that is:

- deterministic across entity allocation, schedule order, queries, events, and replay;
- explicit about component storage, system dependencies, event retention, and fixed-step policy;
- validated before execution for invalid schemas, missing dependencies, and schedule cycles;
- explicit about owner-scoped handles that reject cross-world or cross-schedule use;
- usable without `std`, threads, reflection, or an engine runtime; and
- designed for constrained devices while still supporting headless verification.

Moirai owns ECS data and execution. Your host owns input, rendering, audio, networking,
persistence, and platform integration. The crate does not require a particular game engine or
async runtime.

## Quickstart

### Prerequisites

- Rust 1.75 or later

Add Moirai to your project:

```bash
cargo add moirai-for-games --rename moirai
```

This writes a dependency entry like:

```toml
[dependencies]
moirai = { package = "moirai-for-games", version = "0.1" }
```

### Run your first app

Create an application with one resource and one update system:

```rust
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
            .expect("registered resource")
            .expect("seeded resource")
            .0 += 1;
    }))
    .expect("valid system");

let mut app = builder.build().expect("valid app");
app.update(1.0 / 60.0).expect("update");

assert_eq!(app.world().world_tick().raw(), 1);
assert_eq!(app.world().resource::<Counter>().unwrap(), Some(&Counter(1)));
```

`AppBuilder` validates the world and schedule together. `App::update` advances the world tick and
runs the built-in update stages in their checked order.

## How execution works

The standard builder separates application construction from execution:

```text
AppBuilder
├─ WorldBuilder: components, resources, events, initial data
└─ ScheduleBuilder: stages, systems, sets, conditions, ordering
          │
          └─ build validates ownership and execution contracts
                         │
                         ▼
                        App
        ┌────────────────┴────────────────┐
        ▼                                 ▼
App::update(delta)                 App::render(delta)
Startup once                       Render stages
FixedUpdate 0..n
Update stages
        │                                 │
        └──── checked flush and frame-event cleanup ────┘
```

The standard schedule contains `Startup`, `FixedUpdate`, `Update`, and `Render`. Startup runs once;
fixed steps run only when configured; update and render are separate host operations. Deferred
commands apply at configured flush boundaries, so structural changes never invalidate a running
system body.

## Core capabilities

| Area | What Moirai provides |
| --- | --- |
| World model | World-owned generational entity IDs, resources, sparse and table components, tags, bundles, and deferred commands |
| Scheduling | Checked stages, system sets, explicit ordering, conditions, fixed-step policy, update plans, and runtime enable/disable handles |
| Queries | Typed queries, exact-ID policy, prepared membership/result policies, change windows, cursors, and deferred query effects |
| Events | Typed broadcast events with explicit readers and frame, manual, or bounded retention |
| State and time | Checked state transitions, world/change ticks, fixed-step metadata, and debt policy |
| Constrained hosts | `no_std + alloc`, Q16.16 fixed-point values, dense entity scratch storage, and no runtime dependencies |
| Verification | Structured diagnostics plus optional deterministic replay reports and exact host snapshots |

Opaque entity, component, event, stage, system, and query handles remain scoped to the
world or schedule that created them. Cross-owner use returns an error instead of indexing unrelated
state.

## Learn through tiered examples

[`moirai::examples`](https://docs.rs/moirai-for-games/latest/moirai/examples/index.html) is the canonical
learning path. Its 17 lessons are stable-Rust doctests that use the same public API available to
applications.

| Tier | Focus | Lessons |
| --- | --- | ---: |
| **A** | World and application foundations: resources, components, bundles, tags, deferred commands | 4 |
| **B** | Scheduled behavior: ordering, state transitions, typed events, fixed timestep | 4 |
| **C** | Prepared queries, filters, change cursors, and controlled side effects | 4 |
| **D** | System locals, diagnostics, dense scratch data, Q16 values, deterministic replay | 5 |

Start with [A01: Run your first app](https://docs.rs/moirai-for-games/latest/moirai/examples/tier_a/a01_first_app/index.html)
and follow each lesson's **Next** link. The final replay lesson requires the `testkit` feature and is
included in docs.rs builds.

## Features

Moirai has no default Cargo features.

| Feature | Purpose |
| --- | --- |
| default feature set | The dependency-free `no_std + alloc` ECS, schedule, query, event, diagnostics, Q16, and example APIs |
| `std` | Additive standard-library integration; the core execution model remains unchanged |
| `testkit` | Deterministic finite replay, step records, exact host snapshots, metrics, partial failure reports, and report comparison |
| `bench-internals` | Repository benchmark seams; not a stable host-facing API |

Build the default constrained surface:

```bash
cargo check --no-default-features
```

Run the replay lesson and testkit tests:

```bash
cargo test --features testkit
```

## Stability and boundaries

The public Rust API follows [Cargo's SemVer compatibility rules](https://doc.rust-lang.org/cargo/reference/semver.html).
Before 1.0, minor releases may revise public contracts; patch releases should preserve them unless
the changelog documents a required correction.

Moirai deliberately remains single-threaded and forbids unsafe code. It does not expose allocator,
registry, storage-engine, command-queue, or schedule-runner internals. Host snapshots used by
`testkit` are explicit application types—Moirai does not reflect or serialize a type-erased world.

## Development

Run the standard public checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --package moirai-for-games --no-deps --all-features --locked
```

The complete repository gate also checks the feature matrix, production source coverage, MSRV,
benchmark compilation, and the publishable package:

```bash
scripts/verify_all.sh
```

The complete gate requires a current Rust toolchain plus the Rust 1.75 toolchain,
`cargo-llvm-cov`, and [UV](https://docs.astral.sh/uv/).

## Reference and next steps

- [API reference](https://docs.rs/moirai-for-games)
- [Tiered executable examples](https://docs.rs/moirai-for-games/latest/moirai/examples/index.html)
- [Crates.io package](https://crates.io/crates/moirai-for-games)
- [Changelog](CHANGELOG.md)

## License

MIT — see [LICENSE](./LICENSE).
