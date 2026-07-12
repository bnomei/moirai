# Phase 1 — Single-crate contract and quality scaffold

**Status:** complete · 2026-07-12
**Depends on:** [Phase 0](./PHASE_0_ANALYSIS.md)
**Research:** [packet 001](./.orchid/spec-research/001-scaffold-quality-baseline/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)

## Goal

Create the permanent one-crate envelope before storage code makes accidental paths public. This
phase freezes crate attributes, feature semantics, semantic namespaces, root/prelude policy,
documentation tests, and CI. It does not publish empty implementation modules as API.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R101 | WHEN Moirai builds without defaults THE LIBRARY SHALL be `no_std + alloc` on Rust 1.75. |
| R102 | WHEN `std`, `testkit`, or all features build THE FEATURE SET SHALL remain additive and coherent. |
| R103 | WHEN rustdoc is generated IT SHALL expose no accidental public namespace; a semantic namespace SHALL first publish with its owning real surface. |
| R104 | WHEN Phase 1 visibility/README policy drifts DOWNSTREAM COMPILE TESTS SHALL prove internals remain inaccessible without inventing runtime behavior. |
| R105 | WHEN internal modules are created allocator/storage/registry/queue/runner types SHALL remain private. |
| R106 | WHEN CI runs IT SHALL separate current-stable quality/tooling from library-only MSRV proof. |
| R107 | WHEN dependencies are inspected core SHALL contain no Wyrd, Anapao, Bevy, Playdate, serde, or proc-macro edge. |
| R108 | WHEN the public envelope lands IT SHALL contain no panic/no-op behavior placeholder advertised as implementation. |

## Deliverable

One package named `moirai`:

```toml
[package]
name = "moirai"
edition = "2021"
rust-version = "1.75"

[features]
default = []
std = []
testkit = []

[dependencies]

[dev-dependencies]
divan = "..."
rstest = "..."
```

Exact dependency versions are selected during the spec after checking their MSRV. No runtime
dependency is admitted merely for convenience.

The library root begins with:

```rust
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
```

`alloc` is an unconditional platform assumption, not an empty Cargo feature. `std` and `testkit`
are additive. `cargo check --all-features` must always make sense.

## Module policy

### Final public namespaces

Only concept-level namespaces may become public, with each first published by its owning phase:

- `moirai::component`
- `moirai::event`
- `moirai::math`
- `moirai::query`
- `moirai::schedule`
- `moirai::world`
- `moirai::diagnostics`
- `moirai::prelude`

`moirai::testkit` is a final Phase 6 namespace behind the `testkit` feature. Phase 1 deliberately
does not publish any empty semantic module or test a future path: adding a namespace with its real
surface is additive, while a placeholder would violate R108. Component/math names begin in Phase 2,
World/event names in Phase 3, App/schedule/diagnostics names in Phase 4, query names in Phase 5, and
testkit in Phase 6.

`app`, `command`, `entity`, `operation`, `resource`, `state`, `storage`, and `time` may be private
modules whose stable types are deliberately re-exported. `operation` contains only the shared
`StageOperation` classification used by schedule and event policy; it contains no Schedule state.
A private file can still contain a public type reached through the root; file visibility is not API
design.

Implementation children stay private from their first commit:

```text
entity/allocator
component/registry
storage/{erased,sparse,table,archetype}
command/queue
resource/store
event/{registry,queue,component}
world/{access,spawn,flush,resources,events,query/*}
schedule/{condition,builder,compiled,runner,error}
```

There are no blanket declarations such as `pub mod storage`, `pub mod archetype`, or
`pub mod components`. The source crate's public allocator, registry, columns, queues, command
variants, and query plans are visibility accidents Moirai deliberately corrects.

### Curated crate root

The target root vocabulary is frozen now even though owning phases publish it incrementally:

```rust
pub use app::{App, AppBuilder, AppError, AppFault, BuildError};
pub use command::Commands;
pub use component::{ComponentId, ComponentOptions, StorageKind};
pub use entity::EntityId;
pub use event::{EventId, EventOptions, EventReader, EventRetention};
pub use query::{
    Query1, Query2, QueryCache, QueryCursor, QueryError, QueryParams, QueryResultCache, QuerySpec,
};
pub use operation::StageOperation;
pub use schedule::{
    stage, FlushMode, Schedule, ScheduleBuilder, ScheduleError, StageId, System, SystemId, SystemSet,
};
pub use state::State;
pub use time::{ChangeTick, FixedConfig, FixedStep, WorldTick};
pub use world::{Bundle, DynamicBundle, World, WorldBuilder};
```

This is a final-path contract, not a Phase 1 stub list. `Bundle`/`BundleWriter` and
`diagnostics::Observer` are real downstream extension traits whose signatures belong to Phases 3
and 4; Phase 1 must not expose empty traits or guess their method contracts. The same rule defers
future enums and builders until their owner can publish their actual invariants.

Returned error types that are not re-exported at the root remain nameable in their semantic
namespace, for example `moirai::world::WorldError` once that namespace exists.

Do not add root exports opportunistically. A root addition requires a public-API test and a short
reason that it belongs on the common path.

### Deliberately smaller prelude

The final prelude is exact:

```rust
pub use crate::{
    App, AppBuilder, Commands, ComponentOptions, DynamicBundle, EntityId, FlushMode,
    QueryParams, QuerySpec, State, StorageKind, System, SystemSet, World, WorldTick,
};
```

The prelude excludes errors, cache types, event infrastructure, diagnostics, `Q16`, adapter names,
and host types. It first publishes with the first real system-facing surface; Phase 1 must not ship
an empty prelude. This keeps `use moirai::prelude::*` compatible beside Wyrd and Anapao preludes.

## Public API proof

Add tests from the first phase so visibility drift fails immediately:

- Phase 1 proves its crate boundary through downstream `compile_fail` checks: implementation modules
  cannot be named and no future root/namespace/prelude path is accidentally published.
- Each owning phase adds downstream import/type-position tests when it introduces its real public
  surface; Phase 6 runs the complete root/namespace/prelude matrix.
- crate doctests cover only executable behavior that the owning phase has implemented.
- README snippets are doctests or are copied verbatim into compile tests only when their owning
  behavior exists; Phase 1 has no runnable ECS quickstart.
- `compile_fail` rustdoc examples in `lib.rs` prove representative internals cannot be named
  downstream once those internals exist, without adding a placeholder test framework.

Public structs keep fields private. Error enums expected to grow are `#[non_exhaustive]`.
Constructors and builders enforce invariants; consumers do not construct raw ids or configuration
struct literals.

Do not add `cargo semver-checks` until there is a published/baselined public API to compare. Once
there is one, the baseline becomes a required release check.

## CI contract

The initial workflow has separate, legible jobs:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --no-default-features`
4. `cargo test --features std`
5. `cargo test --features testkit`
6. `cargo test --all-features`
7. `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
8. `cargo bench --no-run`
9. `cargo +1.75 check --lib --no-default-features`

The MSRV job checks the library contract only. Coverage and developer tooling run on current
stable; they do not redefine the MSRV. `std`-specific tests live behind a clear cfg and never leak
std types into unconditional signatures.

The benchmark harness may be wired in this phase, but placeholder performance numbers are not
published. Each later hot-path change adds its own Divan case.

## Tasks

- [x] **T101** Create the single package manifest with Rust 1.75 and the exact feature set.
- [x] **T102** Add `#![no_std]`, unconditional `extern crate alloc`, and
  `#![forbid(unsafe_code)]`.
- [x] **T103** Create the private physical module tree and crate documentation; publish no semantic
  facade before its owning phase has a real surface.
- [x] **T104** Document the root/prelude admission rule in `lib.rs`.
- [x] **T105** Add downstream `compile_fail` privacy and README truthfulness checks without fake
  runtime authoring; owner phases add availability tests with their real APIs.
- [x] **T106** Add CI for format, lint, feature tests, warnings-denied docs, benchmark-harness
  compilation, and MSRV.
- [x] **T107** Add dependency/MSRV policy and `cargo deny` or equivalent license/advisory
  configuration if
  selected by the implementation spec.
- [x] **T108** Confirm `cargo check --all-features` is additive and contains no numeric exclusivity.
- [x] **T109** Confirm no Wyrd, Anapao, Bevy, Playdate, serde, proc-macro, or platform dependency
  enters core.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
cargo test --features std
cargo test --features testkit
cargo test --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo bench --no-run
cargo +1.75 check --lib --no-default-features
```

In addition, review generated rustdoc's module index. It must show concepts, not storage machinery.

## Risks and controls

| Risk | Control |
| --- | --- |
| Empty public modules accidentally become commitments | only semantic facades are public; implementation children start private |
| Prelude grows into a second root | exact compile test and explicit admission rule |
| MSRV is confused with coverage toolchain | separate jobs and documented scopes |
| Cargo feature unification creates impossible builds | only additive `std` and `testkit` |
| A derive macro is requested for convenience | defer; a proc macro would violate the one-crate lock |
| Scaffold types become fake implementations | implement only final types/invariants; do not expose panic/no-op stubs |

## Exit criteria

- [x] One dependency-pure package builds as unconditional `no_std + alloc`.
- [x] All three supported feature combinations and `--all-features` pass.
- [x] The final root/prelude/namespace contract is documented; Phase 1 proves no premature public
  path leaks, and owner phases add availability tests with real APIs.
- [x] No implementation container appears in rustdoc.
- [x] CI distinguishes MSRV, stable quality, and future coverage work.
- [x] Phase 2 can add internals without changing module visibility.

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Rust visibility and privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
- [rustdoc re-exports](https://doc.rust-lang.org/rustdoc/write-documentation/re-exports.html)
- [Cargo SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)
