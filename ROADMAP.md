# moirai — Roadmap

**Status:** Phase 0 accepted; Phase 1 ready for dispatch · 2026-07-12
**Architecture contract:** [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)
**Behavioral source:** [`pd-asteroids/game-core/src/ecs/`](../pd-asteroids/game-core/src/ecs/)
**Partners:** [wyrd](../wyrd/) and [anapao](../anpao/)
**Consumers:** [pd-asteroids](../pd-asteroids/) and [sea-of-grass](../sea-of-grass/)

Each phase is a delegation-ready requirements/design/task document. Mark work done only when its
stated proof exists; a source test or planning paragraph is not implementation evidence.

## What Moirai is

Moirai is one engine-neutral, `no_std + alloc` ECS crate. It owns entities, typed components and
resources, dual storage, deferred structural commands, validated deterministic schedules, typed
events, queries, and a small fixed-point helper.

It is not a game framework, graph engine, serializer, reflection system, or test simulator. Host
games own domain components/stages/rendering; Wyrd owns signal graphs; Anapao owns scenario
assertions/artifacts.

Moirai 1.0 is deliberately single-threaded. It does not promise `Send + Sync` App values or
parallel scheduling; downstream batch tools may impose stronger bounds on their own factories.

Safe public line:

> A small deterministic Rust ECS for constrained and headless games, with validated schedules,
> dual storage, typed resources/events, and optional downstream Wyrd/Anapao integration.

## Architecture at a glance

```text
Host game
  └─ moirai::App
       ├─ World       entities · components · resources · events · commands · queries
       └─ Schedule    validated systems · stages · conditions · fixed steps

Wyrd-owned Moirai adapter ──uses──> App/World/Schedule public seams
Anapao-owned adapter ────────uses──> moirai::testkit replay reports
```

The critical ownership rule is `App { World, Schedule }`. World does not embed Schedule. This makes
`App::update` safe while systems retain mutable World access and keeps `#![forbid(unsafe_code)]`
viable.

Every stage declares `StageOperation::Update` or `Render`. Update owns topology changes and command
flushes; Render is topology-read-only. Frame event channels declare the same operation owner, so
prequeued host input, observation, and clearing have one deterministic boundary.

## Public API philosophy

- Private implementation modules hide allocator, registries, sparse/table storage, raw command
  variants, event queues, and query plans.
- Stable semantic namespaces are `component`, `event`, `query`, `schedule`, `world`, `math`, and
  `diagnostics`, plus additive `testkit`.
- The crate root re-exports common happy-path types.
- `prelude` is smaller and contains only system-authoring essentials.
- Public fields are private behind constructors/builders; growing errors are non-exhaustive.
- README/doctest/public-api tests freeze intended import paths before implementation spreads.

See [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the exact tree and export lists.

## Ecosystem ownership

| Concern | Owner | Boundary |
| --- | --- | --- |
| ECS data/execution | Moirai | `App`, `World`, `Schedule` |
| Graph authoring/settle/state | Wyrd | downstream atomic `WyrdDriver` |
| Replay assertions/artifacts | Anapao | downstream report bridge |
| Exact host snapshots | Host + `moirai::testkit` | typed canonical snapshot |
| Rendering/input/assets/save schema | Host | never Moirai core |

Adapters live with the library that owns their semantics. Moirai therefore has no Wyrd or Anapao
runtime dependency and keeps one Rust 1.75 core/MSRV contract.

## Numeric model

Moirai is numeric-agnostic. A World may store `f32`, grid integers, and `math::Q16` components
together. There are no `numeric-f32`/`numeric-q16` crate features.

`Q16` is a conventional fixed-point newtype with private bits, checked/saturating conversions, and
checked division. It does not copy Wyrd's domain-tagged Signal semantics: Wyrd i32 counts are raw
integers while levels are Q16 bits. Wyrd adapters convert Bool/Count/Level explicitly.

Performance claims remain hypotheses until representative release benchmarks and device checks.

## Proven behavior and deliberate corrections

Preserve:

- generational stale-handle detection;
- sparse and table/archetype storage;
- deferred Commands with explicit system/stage/final flush policies;
- event retention/readers/operation lifetime and resource change ticks;
- deterministic stage/system order and fixed-step cap;
- Query1/Query2/spec behavior plus both cache use cases;
- steady-state allocation discipline after warmup.

Correct before publishing:

- track entity occupancy and retire generation-overflow slots;
- reject component name/type/layout conflicts;
- separate World from Schedule and remove source unsafe pointer execution;
- make order cycles setup errors instead of runtime panics;
- replace game-specific `GameState` with `State<S>`;
- remove Playdate FFI, magic `Inactive` names, unused named resources, raw cache keys, and public
  Spawn-no-op/RunSystem command quirks;
- use typed events/resources by default and contextual errors for misuse.

The 151 pd-asteroids tests are a characterization inventory classified as preserved, adapted, or
rejected—not 151 immutable API requirements.

## Decision locks for sign-off

| # | Decision | Recommended lock |
| --- | --- | --- |
| D1 | Package topology | One published Moirai crate through 1.0 |
| D2 | Runtime ownership | `App` owns sibling World + Schedule; systems receive `&mut World` |
| D3 | Public facade | Semantic modules + curated root + smaller prelude |
| D4 | Build envelope | unconditional `no_std + alloc`, single-threaded 1.0; additive `std`/`testkit` |
| D5 | Safety | `#![forbid(unsafe_code)]`; platform FFI downstream |
| D6 | Configuration | checked builders; cycles/conflicts fail before first update |
| D7 | Components/events/resources | typed defaults; private registries; no named resources |
| D8 | Entity/component topology | immediate setup; deferred Commands in Update; Render topology-read-only |
| D9 | Query surface | Query1/2/spec + ChangeTick cursor + owner-scoped dual caches |
| D10 | State/stages | generic `State<S>`; operation-owned stages; fixed-before-update; host graphs stay downstream |
| D11 | Numeric model | always-available conventional Q16; no global numeric feature |
| D12 | Quality | 100% executable source lines plus invariant/state-machine/API tests |
| D13 | Wyrd | Wyrd-owned atomic driver; independent settle tick |
| D14 | SoG deletion gate | 16 behavior cases plus versioned Wyrd restore continuation |
| D15 | Anapao | neutral Moirai testkit; Anapao-owned external report/assertion bridge |
| D16 | Migration | dual-run/path-dependency proof before deleting either host implementation |

## Phase index

| Phase | Theme | Main result |
| --- | --- | --- |
| [0](./PHASE_0_ANALYSIS.md) | Architecture and parity sign-off | D1–D16 locked; corpus classified |
| [1](./PHASE_1_SCAFFOLD.md) | Crate contract | Facade, visibility, features, CI, API tests |
| [2](./PHASE_2_CORE_STORAGE.md) | Checked core | Identity, registration, storage, Q16 |
| [3](./PHASE_3_WORLD_LIFECYCLE.md) | World data | Commands, tables, resources, typed events, flush |
| [4](./PHASE_4_SCHEDULE.md) | Safe execution | App, compiled Schedule, conditions, fixed steps, observers |
| [5](./PHASE_5_QUERIES.md) | Query completion | Mixed storage, filters, owner-scoped cache state |
| [6](./PHASE_6_QUALITY.md) | Proof closure | Neutral testkit, parity, safety, API, coverage, performance |
| [7](./PHASE_7_INTEGRATIONS.md) | Ecosystem migration | Downstream adapters, persistence, host cutovers |

Phase 2 should prove a small executable sparse-world slice instead of waiting for a horizontal
layer cake. Phase 3 completes structural storage. Phases 4 and 5 may overlap after World contracts
stabilize. Quality tests and benches land with their owning code; Phase 6 closes the full gate.

## Wyrd contract

The downstream default is one atomic resource-scoped driver system:

```text
begin_frame(SettleTick) → sample host World → loom → apply to host World
```

It is ordered by the host (SoG: after Actions, before portal travel). The driver's next
`SettleTick` advances only after the atomic step completes Apply and is independent from outer
`WorldTick` and fixed substeps. Any sample/loom/apply error leaves that tick unchanged and
sticky-faults driver plus App; a condition-skipped action never calls the driver.

Before SoG deletes `WiringState`, Wyrd must provide versioned, topology/numeric-bound
snapshot/restore for held senses, edge history, counters, flags, timers, OnStart, delays, RNG, and
tick. Its driver envelope records last-completed plus next settle tick and validates their
relationship with RuntimeState before mutation. Continuation tests must cover latch, mid-delay,
held sense, RNG, and mismatch/corruption rejection.

## Anapao contract

Anapao Simulator currently runs only Anapao CompiledScenario. Moirai will not wrap an App in a fake
ScenarioSpec.

`moirai::testkit` provides seeded steps, exact host-selected snapshots, scalar metrics, and a stable
post-flush observation seam. An Anapao-owned adapter maps scalar metrics/events into its public
reports, expectation evaluators, and artifacts. Exact ECS equality remains typed Moirai/host proof.

## Quality bar

- `cargo fmt --check`
- strict Clippy and tests on current stable
- Rust 1.75 library-only no_std check
- doctests, README snippets, `tests/public_api.rs`, and prelude checks
- additive feature matrix: core no_std, std, testkit
- source-line coverage on current stable
- allocation tests and Divan benchmarks for f32 and Q16 workloads in the same build
- scoped property/state-machine tests for allocator, registration, schedule, events, and caches
- `cargo semver-checks` after the first published API baseline

## Non-goals through 1.0

- crate split or proc-macro companion;
- Bevy compatibility/derive API;
- parallel scheduling;
- reflection, scripting, hot reload, serialization policy;
- rendering, physics, asset, or Playdate SDK types;
- host-specific state/stage presets;
- one entity per SoG voxel;
- common base traits shared by Moirai, Wyrd, and Anapao.

## Success signals

- [ ] A newcomer builds and runs a headless App from the root/prelude docs.
- [ ] No safe caller can reach overlapping mutable schedule/world state.
- [ ] pd-asteroids passes its classified migration corpus with local ECS removed.
- [ ] SoG preserves exact wiring save continuation and removes Bevy ECS/wiring evaluation.
- [ ] Wyrd adapter step and Anapao report bridge remain outside Moirai core dependencies.
- [ ] Public API, no_std/MSRV, coverage, allocation, and benchmark gates are reproducible.

## Related evidence

- [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)
- [`docs/parity.md`](./docs/parity.md)
- [`docs/wyrd-parity.md`](./docs/wyrd-parity.md)
- [single-crate research packet](./.orchid/spec-research/006-single-crate-api-architecture/)
- `pd-asteroids/game-core/src/ecs/`
- `wyrd/crates/wyrd-for-games/src/lib.rs`
- `wyrd/crates/wyrd-for-games-bevy/src/lib.rs`
- `anpao/src/lib.rs`, `anpao/src/prelude.rs`, `anpao/tests/public_api.rs`
- `sea-of-grass/src/wiring.rs`
