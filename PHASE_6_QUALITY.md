# Phase 6 — Proof closure, compatibility, and performance

**Status:** closes after Phases 2–5 land their owning tests and benches
**Depends on:** [Phase 4](./PHASE_4_SCHEDULE.md), [Phase 5](./PHASE_5_QUERIES.md)
**Research:** [packet 004](./.orchid/spec-research/004-queries-performance-proof/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)

## Goal

Turn the implemented crate into a defensible 1.0 candidate. Phase 6 closes—not invents—the evidence
for behavioral compatibility, safety invariants, public API shape, feature/MSRV support,
executable-line coverage, steady-state allocation, and representative performance.

Passing 151 translated tests is not sufficient if they preserve an unsafe or host-specific
accident. Reaching 100% line coverage is not sufficient if assertions do not prove state
transitions. Both are required and interpreted through the Phase 0 classification.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R601 | WHEN parity closes every row in `docs/parity.md` SHALL map to a passing replacement or documented rejection proof. |
| R602 | WHEN `testkit` builds IT SHALL remain `no_std + alloc` and depend on no sibling library. |
| R603 | WHEN seeded replay repeats exact host snapshots SHALL match step-for-step and failures SHALL retain partial reports. |
| R604 | WHEN hostile safety regressions run stale ids, conflicts, owner errors, aliases, and partial batches SHALL remain contained. |
| R605 | WHEN feature/MSRV quality runs core/std/testkit/all-features and Rust 1.75 lib checks SHALL pass. |
| R606 | WHEN coverage reports merge executable source-line coverage SHALL be 100% with meaningful assertions. |
| R607 | WHEN steady-state paths run within warmed capacities THE DOCUMENTED ZERO-ALLOCATION SET SHALL hold. |
| R608 | WHEN benchmarks are compared environment/statistics/source baselines SHALL be recorded reproducibly. |
| R609 | WHEN public docs/API tests run root/prelude/namespaces/README/compile-fail contracts SHALL pass. |
| R610 | WHEN a public baseline exists semver checks SHALL guard it and intentional source breaks SHALL be documented. |
| R611 | WHEN packaging is checked no path dependency, adapter, host asset, or generated report SHALL leak. |
| R612 | WHEN Phase 6 exits Phase 7 SHALL have no undefined core/testkit semantic left to discover. |

## Compatibility ledger

Phase 0's [`docs/parity.md`](./docs/parity.md) contains one row for every pd-asteroids
characterization test:

| Source test | Classification | Moirai owner/test | Rationale |
| --- | --- | --- | --- |
| test path/name | preserve / adapt / reject | phase + test path | exact observable contract or deliberate correction |

Rules:

- **Preserve:** equivalent observable trace and assertions pass through Moirai's intended API.
- **Adapt:** the source intention is retained but timing/API/error mode changes; both old behavior
  and new contract are documented.
- **Reject:** no port is written solely to keep an accidental behavior. The row names the
  replacement invariant or downstream owner.

Mandatory rejected/adapted surfaces include embedded-schedule raw pointers, runtime cycle panic,
public allocator/registry/storage/command/cache internals, game-specific `GameState`, Playdate
profiler FFI, Spawn no-op, `RunSystem` commands, magic `Inactive` naming, silent registration
aliasing, and raw caller cache keys.

The ledger total must reconcile with the discovered source corpus. A changed source count triggers
inventory review rather than silently preserving “151” as folklore.

Sea of Grass/Wyrd behavior and persistence are tracked separately in Phase 7 because they gate a
downstream deletion, not core source parity.

## Test layers

### Contract and example tests

- downstream-style root, namespace, and prelude compile tests;
- doctests for App authoring, schedule errors, commands, events, queries, Q16, and observation;
- README snippet drift test;
- compile-fail tests for representative private/sealed internals and invalid extension attempts;
- `cargo semver-checks` after the first explicit public baseline.

### Invariant and model tests

- allocator state machine including Reserved and Retired;
- registry transaction/conflict model;
- archetype/location and structural-batch reference model;
- event reader/retention model;
- schedule topological-order model and cycle path;
- fixed-step accumulator model;
- query/cache reference model;
- Q16 wide-integer reference arithmetic;
- sticky fault and recovery traces.

Randomized tests use recorded seeds on failure. CI runs a bounded deterministic seed set; a longer
local/nightly job expands it.

## Neutral testkit implementation

Phase 6 implements `moirai::testkit` before validating its feature matrix. It depends only on core
Moirai and `alloc` and provides:

- opaque checked `StepIndex`, first step 0;
- deterministic seeded driver/factory contracts;
- `ReplayConfig` with finite step/failure/capture policy;
- `StepSnapshot<S>` and `ReplayReport<S>` for host-defined exact `S: Eq` snapshots;
- selected scalar `MetricSample` values;
- an App driver/helper that captures through `App::update_with` after final flush and before
  frame-event clearing.

The core extension point is:

```rust
pub trait ReplayDriver {
    type Snapshot: Eq;
    type Error;

    fn step(
        &mut self,
        step: StepIndex,
    ) -> Result<StepRecord<Self::Snapshot>, Self::Error>;
}
```

`ReplayConfig` contains a seed, finite non-zero step count, and every-step/final-only capture
policy. The runner passes the seed to a caller factory; Moirai does not own an RNG. `StepRecord`
contains the step, optional WorldTick, exact snapshot, and scalar metrics. A failed step returns
`ReplayFailure { step, partial_report, source }` rather than discarding evidence. StepIndex uses
checked increment and never wraps.

The App helper owns/calls host snapshot and metric closures. Metric keys are owned strings and
values are f64 solely for report interoperability; they are not the exact state proof.

The host supplies snapshot construction and canonical ordering; testkit never reflects or
serializes an arbitrary type-erased World. Its own proof uses a small seeded Moirai App without
Wyrd or Anapao, repeats the run, and compares every typed snapshot. Phase 7 reuses this surface for
the Wyrd-controlled door and Anapao report bridge.

`testkit` remains usable under `no_std + alloc`. It does not imply `std`, add a sibling dependency,
invent f64-only ECS snapshots, or duplicate Anapao assertions/artifacts.

### Hostile regression tests

Keep named tests for the failures research identified:

- freed current-generation slot is not alive;
- forged/raw bits cannot bypass World validation;
- conflicting registration leaves registry unchanged;
- system cannot obtain/mutate the running schedule through World;
- immediate structural mutation during run is rejected;
- failed command batch applies no structural operation;
- same-type mutable Query2 is rejected;
- query cache from another World is rejected;
- event clearing occurs after observation;
- runtime cycle is a build error;
- scoped resource cannot be reborrowed/replaced as the same type;
- generation/cache slot overflow does not revive stale handles.

## Coverage contract

The release target is 100% executable source-line coverage across the supported feature union,
excluding test/benchmark code and mechanically non-executable lines. Coverage is measured on
current stable with `cargo-llvm-cov`; it is not an MSRV job.

Run at least:

1. core `--no-default-features`;
2. `--features std`;
3. `--features testkit`;
4. doctests/compile examples where the coverage tool supports them.

Merge reports so cfg-specific paths are not hidden by an `--all-features`-only build. Any justified
tooling exclusion is line-specific, reviewed, and recorded in `docs/coverage.md`. “Unreachable in
tests” is normally evidence that the code or API should be removed.

Coverage assertions must inspect observable state. A test that only executes a branch to paint it
green does not satisfy the quality gate.

## Feature and toolchain matrix

Core has exactly:

```toml
[features]
default = []
std = []
testkit = []
```

Required matrix:

| Build | Purpose |
| --- | --- |
| Rust 1.75, no defaults, lib check | MSRV and unconditional `no_std + alloc` contract |
| current stable, no defaults | core tests/lints/docs |
| current stable, `std` | std error/source conveniences |
| current stable, `testkit` | replay API without hidden adapter dependencies |
| current stable, all features | Cargo feature additivity |

There is no f32-versus-Q16 feature matrix. Both scalar types are benchmarked and tested in the same
build. Wyrd's unavoidable numeric feature choice belongs to its downstream adapter matrix.

The crate must contain no `unsafe` block, `unsafe fn`, or `unsafe impl` and continues to compile
under `#![forbid(unsafe_code)]`.

## Performance methodology

Benchmark evidence is reproducible, not a single attractive number. `docs/perf.md` records:

- commit and Rust/tool versions;
- CPU/target, OS, power mode, and benchmark command;
- warmup/sample configuration;
- median and dispersion across repeated runs;
- source pd-asteroids baseline where workloads can be made equivalent;
- allocation count/capacity assumptions;
- accepted regression thresholds and rationale.

### Required Divan families

- allocate/spawn/despawn and stale-heavy churn;
- sparse insert/remove/iterate;
- table/archetype spawn and one-/multi-component moves;
- command queue preflight/commit;
- typed events with one/many readers and retention;
- schedule run with conditions, flush modes, and fixed substeps;
- Query1/Query2 cold, warm, cached, mixed, and closure-mutable paths;
- Q16 and f32 component workloads in one binary;
- representative pd-asteroids and Sea of Grass host-shaped traces.

Setup/schema construction is measured separately from hot execution. Bench inputs are kept alive
and outputs observed so optimization cannot remove the work.

A regression gate uses same-machine repeated comparison and confidence/noise rules chosen in the
implementation spec. Do not claim an absolute nanosecond budget portable across machines. Any
accepted slowdown must buy a recorded safety/correctness benefit and stay within host frame
budgets.

## Allocation contract

After explicit representative warmup/reservation, these paths allocate zero times in steady state:

- an update with no new topology;
- schedule condition/order traversal;
- hot Query1/Query2 traversal;
- membership/result cache hits;
- event send/read within retained capacity;
- typed command queueing/flush within reserved command, bundle, and archetype capacity;
- no-op diagnostics observer.

Topology growth, first-seen archetype creation, dynamic/authored bundle payload boxing, user payload
allocation, diagnostic error formatting, and replay snapshot collection are allowed allocations
and are documented separately.

Allocation tests use a counting allocator under `std` and assert both operation counts and bytes.
No production API silently depends on that allocator.

## Documentation and API evolution

Before 1.0 candidate:

- every root export and public module has a runnable example or focused reference documentation;
- all public `Result` error timing and structural visibility are documented;
- every public struct has private fields unless transparent layout is the contract (`Q16`);
- growing public enums are `#[non_exhaustive]`;
- rustdoc has no implementation-container modules;
- the changelog identifies intentional pd compatibility breaks;
- the first semver baseline is captured only after Phase 0 locks are implemented.

Moving a public item to “clean up modules” is breaking unless its old path remains re-exported.
That is why semantic paths are reviewed now.

## Dependency and artifact hygiene

- audit licenses/advisories with the selected repository tool;
- run `cargo machete` (or equivalent evidence) before claiming a dependency is unused;
- check the packaged crate contents and build the package, not only the working tree;
- keep benchmark/coverage output out of the crate package and version control;
- verify no path dependencies, sibling adapters, or host assets leak into published Moirai.

## Tasks

- [x] **T601** Reconcile the Phase 0 preserve/adapt/reject ledger against implemented tests.
- [x] **T602** Port or replace every preserved/adapted source case in its owning module.
- [x] **T603** Add all hostile regressions and reference-model/state-machine suites.
- [x] **T604** Implement/document neutral `moirai::testkit` replay/snapshot/metric primitives.
- [x] **T605** Prove deterministic exact replay through the post-flush/pre-clear observation seam.
- [x] **T606** Close merged executable-line coverage to 100% with meaningful assertions (merged 4-flavor union **100.00%** / 10,652 lines; `--all-features` alone **97.58%** / 262 missed as of 2026-07-13).
- [x] **T607** Run the complete feature/MSRV/docs/lint matrix.
- [x] **T608** Record stable same-machine benchmark baselines and host-shaped comparisons (`docs/perf.md`, commit `ab93dbb`).
- [x] **T609** Prove steady-state allocation contracts (**17/17** release tests pass via `tests/allocation.rs` + `--test-threads=1`).
- [x] **T610** Complete root/prelude/namespace/README/compile-fail API tests.
- [x] **T611** Review rustdoc visibility and non-exhaustive/private-field posture (`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` passes; public structs keep private fields).
- [x] **T612** Capture the first semver baseline at the agreed release-candidate point (`CHANGELOG.md` **0.1.0-rc.1**; run `cargo semver-checks` before publish).
- [x] **T613** Audit dependencies, license/advisories, and packaged contents (zero non-dev dependencies; `cargo package --allow-dirty` verifies 223 files, no path deps or host assets).

## Verification

Representative commands (final scripts own exact flags):

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
cargo test --features std
cargo test --features testkit
cargo test --all-features
cargo +1.75 check --lib --no-default-features
cargo doc --no-deps --all-features
cargo llvm-cov --no-default-features
cargo llvm-cov --features std
cargo llvm-cov --features testkit
cargo bench
cargo package --allow-dirty
```

Use a clean package verification for the actual release; `--allow-dirty` is only useful while
developing the plan.

## Risks and controls

| Risk | Control |
| --- | --- |
| 151 tests are treated as 151 good contracts | preserve/adapt/reject ledger with rationale |
| Coverage rewards assertion-free execution | state/invariant assertion review |
| Bench noise becomes a false gate | repeated same-machine statistics and recorded environment |
| Numeric modes double the matrix | no numeric features; f32/Q16 coexist |
| Allocation promise ignores topology growth | explicit warmup/capacity and allowed-allocation table |
| MSRV drifts through dev tools | library-only 1.75 job separated from stable tooling |

## Exit criteria

- [x] Every source characterization case is accounted for.
- [x] All deliberate corrections have named regression tests.
- [x] Merged executable source-line coverage is 100% (merged 4-flavor union **100.00%** / 10,652 lines as of 2026-07-13; `--all-features` alone **97.58%**).
- [x] Supported features, MSRV, docs, lint, package, and no-unsafe gates pass.
- [x] Allocation and performance claims are reproducible and documented (`tests/allocation.rs`, `docs/perf.md`).
- [x] Public API shape is ready to baseline (`CHANGELOG.md` **0.1.0-rc.1**).
- [x] Phase 7 can migrate hosts without discovering an undefined core semantic (parity ledger + hostile + testkit replay closed).

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Cargo SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
