# Phase 0 — Architecture, inventory, and sign-off

**Status:** complete — accepted 2026-07-12; Phase 1 may begin
**Depends on:** source access to pd-asteroids, sea-of-grass, Wyrd, and Anapao (local checkout
`../anpao`)
**Produces:** a frozen public shape, a classified compatibility corpus, and phase-ready research

## Goal

Freeze the choices that would be expensive to reverse after Moirai publishes an API: crate
topology, module visibility, runtime ownership, structural mutation rules, numeric semantics,
adapter ownership, persistence requirements, and the meaning of parity.

Phase 0 does not freeze pd-asteroids implementation accidents. It separates behavior worth
preserving from behavior that must be adapted or rejected for safety, generality, or API
evolution.

The cross-phase contract lives in [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md). This phase is
complete only when that document and D1–D16 below are accepted.

## Acceptance record

**Accepted 2026-07-12:** the project owner accepted D1–D16, the curated root/prelude/namespace
surface, and research packets 001–006 in this workspace. Implementation may begin with Phase 1.
There are no architecture amendments at acceptance.

Any later deviation requires an explicit architecture amendment naming the affected public paths,
tests, downstream migrations, and Phase documents. The canonical plans and research packets are no
longer hidden by broad ignore rules; the next source-control handoff must include this Phase 0
bundle before implementation results are merged.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R001 | WHEN Phase 0 is signed off THE PROJECT SHALL lock D1–D16 and the architecture contract before implementation. |
| R002 | WHEN source parity is claimed `docs/parity.md` SHALL account for all 151 live pd-asteroids ECS tests as preserve/adapt/reject. |
| R003 | WHEN a source behavior is rejected THE LEDGER SHALL name its replacement invariant or downstream owner. |
| R004 | WHEN Moirai's public surface is frozen THE ROOT, PRELUDE, and semantic namespace lists SHALL be explicit. |
| R005 | WHEN runtime ownership is frozen App SHALL own sibling World/Schedule and World SHALL have no Schedule execution edge. |
| R006 | WHEN numeric behavior is frozen f32, integers, and conventional Q16 SHALL coexist without numeric features. |
| R007 | WHEN Wyrd replaces Sea of Grass wiring THE GATE SHALL include 16 behavior traces and versioned restore continuation. |
| R008 | WHEN Anapao integration is planned IT SHALL use neutral testkit reports rather than a fake ScenarioSpec. |
| R009 | WHEN host migrations delete old code PATH-DEPENDENCY/DUAL-RUN evidence SHALL exist first. |
| R010 | WHEN Phase 0 exits THE ONLY OPEN ARCHITECTURE ACTION SHALL be explicit human acceptance or named amendments. |
| R011 | WHEN custom stages/frame events are authored THEIR Update/Render operation SHALL determine execution, topology authority, and clearing. |
| R012 | WHEN Wyrd state restores RuntimeState/last-completed/next tick SHALL validate together before mutation. |

## Required evidence

Phase 0 owns three evidence sets:

1. **pd-asteroids ECS:** its 151 tests form a characterization inventory. Each case is marked
   `preserve`, `adapt`, or `reject` in [`docs/parity.md`](./docs/parity.md) before being assigned
   to a later phase.
2. **Sea of Grass wiring:** the 16 behavioral cases are the minimum semantic replacement corpus,
   source-audited in [`docs/wyrd-parity.md`](./docs/wyrd-parity.md). Save/restore continuation is an
   additional deletion gate, not part of the original 16.
3. **Sibling API precedent:** Wyrd and Anapao provide local evidence for private implementation
   modules, curated roots, small preludes, checked authoring-to-runtime transitions, and explicit
   public API tests.

External Rust research validates language and ecosystem constraints; it does not replace local
behavioral evidence.

## Architecture to sign off

### One published crate

Moirai remains a single published crate through 1.0:

```text
moirai
├── stable semantic modules
├── curated crate-root conveniences
├── deliberately smaller prelude
└── private storage, registry, queue, query-plan, and runner modules
```

There is no proc-macro companion and no `moirai-wyrd` or `moirai-anapao` workspace member.
Physical source modules are used aggressively, but they are not automatically public API.

### Safe runtime ownership

`App` owns `World` and `Schedule` as sibling fields:

```text
App
├── World       data, registries, resources, events, command queue
└── Schedule    systems, conditions, compiled order, fixed-step state
```

Systems keep the migration-friendly body `FnMut(&mut World, f32)`. `World` never owns or exposes a
mutable `Schedule`. This removes the safe path to aliasing and pointer invalidation present in the
pd-asteroids implementation. Moirai is compiled with `#![forbid(unsafe_code)]`.

`AppBuilder::build` resolves registrations and schedule edges before the first update. Cycles,
missing declared resources, conflicting registrations, and invalid event roles are contextual
build errors.

### Structural mutation contract

- Setup/editor entity/component topology calls on an idle `World` mutate immediately.
- While a schedule is running, immediate entity/component structural methods return
  `WorldError::StructuralMutationDuringRun`.
- Systems in Update-owned stages request deferred structural changes through `world.commands()`;
  Render-owned stages are entity/component-topology-read-only.
- Update flush points are explicit schedule policy. App rejects an idle pending batch instead of
  adopting it; the host flushes or discards explicitly.
- `CommandOp` is private; source quirks such as Spawn-as-no-op and `RunSystem` as a command are not
  public semantics.

This is intentionally clearer than pd-asteroids, where several apparently immediate methods always
enqueue work.

### Numeric contract

Moirai has no global numeric feature. One world may store `f32`, integer, and fixed-point
components simultaneously.

`moirai::math::Q16` is an always-available conventional fixed-point newtype with private bits,
exact bit conversion, checked and saturating operations, and explicit fallible/saturating scalar
conversion. Division by zero does not silently become zero.

Wyrd's i32 `Signal` is domain-sensitive: Count values are raw integers while Level values are Q16
bits. It must not be copied as a universal Moirai scalar. Wyrd adapter conversion remains explicit
about Bool, Count, and Level domains.

### Ecosystem ownership

Moirai stays dependency-pure.

- Wyrd owns its Moirai adapter and atomic `begin_frame → sample → loom → apply` driver.
- Anapao owns its external-driver/report/assertion bridge.
- Moirai owns stable neutral seams: `App`, `World`, `Schedule`, `World::resource_scope`, the
  post-flush observation boundary, and the optional `testkit` replay vocabulary.
- Hosts own component mappings, stage graphs, save schema, and policy.

This keeps Moirai at Rust 1.75 and `no_std + alloc` without importing Wyrd's feature choices or
Anapao's higher-MSRV/std graph.

## Source inventory

The pd-asteroids source is an implementation oracle, not the target module layout. Inventory at
sign-off must include:

- generational entity allocation and stale-handle behavior;
- component registration, sparse storage, tables/archetypes, and row moves;
- bundles, structural commands, resources, events, readers, and retention;
- system metadata, stage ordering, conditions, state, fixed update, and flush behavior;
- Query1, Query2, id queries, membership caching, result caching, filters, and mutation;
- profiler hooks, platform assumptions, raw-pointer execution, and game-specific state.

The target module ownership is frozen in [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md), not copied
from `game-core/src/ecs/mod.rs`.

## Characterization classification

Every imported source test receives one of these labels:

| Label | Meaning | Typical examples |
| --- | --- | --- |
| `preserve` | Observable behavior is part of Moirai's intended contract | stale entity rejection, event retention, deterministic schedule order |
| `adapt` | Intent remains, but API or timing changes deliberately | deferred commands only during execution, build-time cycle errors, generic `State<S>` |
| `reject` | Host-specific, unsafe, misleading, or accidental behavior is not ported | Playdate FFI, magic `Inactive` name, public raw cache keys, Spawn no-op |

Rejected cases remain documented so a future contributor cannot accidentally reintroduce them in
the name of parity.

### Mandatory corrections discovered during research

1. The entity allocator must track occupancy separately from generation and retire a slot on
   generation overflow. A freed slot with a matching generation is not alive.
2. Component re-registration is idempotent only when type, name, storage kind, tag/layout, and
   relevant options match exactly. All conflicts are errors.
3. Schedule cycles are detected by `ScheduleBuilder::build`, never by a runtime panic.
4. `GameState { Menu, Game, Pause, Inventory }` becomes host code; Moirai provides `State<S>`.
5. Host stage graphs remain in host install functions. Moirai's standard Update operation contains
   generic Startup, FixedUpdate, and Update; its separate Render operation contains Render.
6. Typed resources and typed events are the common path. Named resources are not ported; named
   events remain an advanced authored/dynamic channel.
7. Query state is owner-scoped. Callers cannot provide arbitrary raw cache keys.
8. Update observation is after its final command flush; Render observation is after its systems.
   Both occur before clearing their operation-owned frame channels.
9. Per-system interval buffering and untyped system pipes are rejected; fixed steps, host timers,
   and ordinary Rust composition cover the demonstrated needs.
10. Registered event sends are never silently disabled; readers are explicit owner-scoped handles.
11. `State<S>` retains current/previous/pending only, not a host navigation stack.
12. FixedUpdate is disabled until configured with a positive Duration. No counter wraps: App clocks
    fault App, ChangeTick poisons World mutation, entity/cache slots retire, and an exhausted event
    sequence closes only its channel.
13. Every stage and frame-retained event channel declares its Update/Render operation. Update owns
    structural flush; Render is topology-read-only; prequeued external frame input clears only at
    its matching operation boundary.

## Tick domains

The ecosystem has four independent counters:

| Domain | Advances when | First executed value |
| --- | --- | ---: |
| `WorldTick` | each outer `App::update` | 1 for pd compatibility |
| `FixedStep` | each fixed substep | 1 |
| Wyrd `SettleTick` | one atomic driver step completes Apply | 0 |
| Anapao `StepIndex` | one replay step executes | 0 |

No counter is derived from another. A skipped or failed Sea of Grass driver step must not advance
its next Wyrd settle tick; several successfully completed fixed driver steps in one update advance
the fixed and settle domains several times.

## Persistence deletion gate

The 16 wiring cases prove immediate behavior but not save compatibility. Sea of Grass currently
persists latch, edge, delay, and completed-step state. Wyrd additionally owns held senses,
previous-input/decrement state, counters, flags, timers, OnStart state, delay rings, RNG, and its
runtime tick.

Before the old wiring evaluator can be deleted, Wyrd must provide:

- a versioned `RuntimeState`;
- a stable topology/layout fingerprint and numeric-path tag;
- complete atomic validation before mutation;
- snapshot/restore of all continuation state;
- exclusion of the ephemeral outbox;
- tests for latch, mid-delay, held sense, timer, RNG, and mismatch rejection;
- uninterrupted versus save/rebind/restore equivalence.

The Wyrd-owned adapter adds independent last-completed and next `SettleTick` fields. Restore checks
their initial/post-Apply relationship against RuntimeState before any mutation. Host binding/domain
state remains host-owned.

## Decision locks D1–D16

| # | Decision | Lock |
| --- | --- | --- |
| D1 | Package topology | One published Moirai crate through 1.0 |
| D2 | Runtime ownership | `App` owns sibling `World` and `Schedule` |
| D3 | Public facade | Semantic namespaces, curated root, smaller prelude |
| D4 | Build envelope | Unconditional `no_std + alloc`, single-threaded 1.0; additive features |
| D5 | Safety | `#![forbid(unsafe_code)]`; platform FFI downstream |
| D6 | Configuration | Checked builders; conflicts/cycles fail before execution |
| D7 | Data APIs | Typed defaults, private registries, no named resources |
| D8 | Entity/component topology | Immediate idle; deferred `Commands` in Update; Render topology-read-only |
| D9 | Queries | Query1/2/spec, ChangeTick cursor, and owner-scoped dual cache modes |
| D10 | State/stages | Generic `State<S>`; operation-owned stages; fixed-before-update; host graphs downstream |
| D11 | Numeric | Always-available conventional `Q16`; no global numeric mode |
| D12 | Quality | Executable-line coverage plus invariant, API, state-machine, and performance proof |
| D13 | Wyrd | Wyrd-owned atomic driver with independent `SettleTick` |
| D14 | SoG deletion | 16 behavior cases plus versioned restore continuation |
| D15 | Anapao | Neutral Moirai testkit; Anapao-owned report/assertion bridge |
| D16 | Migration | Path-dependency/dual-run proof before either host implementation is deleted |

Changing a lock after Phase 1 requires an explicit architecture amendment that identifies affected
public paths, tests, and downstream migrations.

## Phase dependency graph

```text
Phase 0: architecture and classified evidence
  └─ Phase 1: crate contract and public facade
      └─ Phase 2: checked identity, registration, sparse storage, Q16
          └─ Phase 3: complete World data lifecycle
              ├─ Phase 4: compiled Schedule and App
              └─ Phase 5: complete query facade and caches
                    ╲   ╱
                 Phase 6: testkit and proof closure
                    └─ Phase 7: adapters and host migrations
```

Phase 6 joins completed Phases 4 and 5. Tests and benchmarks land with the phase that owns the
behavior; Phase 6 closes accumulated proof and is not the first time performance or invariants are
tested.

## Tasks

- [x] **T001** Recount the live pd-asteroids ECS test corpus and reconcile every row/key in
  `docs/parity.md`.
- [x] **T002** Review and accept the one-crate module tree, stable namespaces, root exports, and
  deliberately smaller prelude.
- [x] **T003** Review and accept App/World/Schedule ownership, StageOperation, structural mutation,
  frame-event, query-window, and exhaustion contracts as one lifecycle.
- [x] **T004** Review and accept feature/MSRV/single-threaded/Q16 decisions and rejected alternatives.
- [x] **T005** Review all sixteen source-audited Sea of Grass traces and the complete Wyrd
  continuation/corruption matrix.
- [x] **T006** Review Wyrd/Anapao/host repository ownership and the independent deletion gates.
- [x] **T007** Record explicit human acceptance or named amendments for D1–D16 and packet 006; update
  every affected phase before marking approval.
- [x] **T008** Remove broad ignore rules so the accepted planning docs and research packets are
  visible for the required source-control handoff before implementation results merge.

## Verification

```sh
rg -n '#\[(rstest|test)\]' ../pd-asteroids/game-core/src/ecs | wc -l
rg -n '^\| [0-9]+ \|' docs/parity.md | wc -l
rg -n '^### [0-9]+\. ' docs/wyrd-parity.md | wc -l
UV_CACHE_DIR=/private/tmp/uv-cache uv run python /Users/bnomei/.codex/skills/make-research/scripts/validate_research_packet.py .orchid/spec-research/006-single-crate-api-architecture
git diff --check
git status --short --ignored
```

The first two counts must both be 151; the Wyrd case count must be 16. Commands validate evidence
shape, not decision approval. The Acceptance record above supplies the human sign-off; any future
amendment must identify its affected public paths/phases.

## Risks and controls

| Risk | Control |
| --- | --- |
| A legacy test name is mistaken for actual behavior | source-audited ordered traces and explicit evidence limitations |
| Phase-local wording silently contradicts the architecture | requirement IDs, cross-links, packet validation, and coherence audit |
| Public conveniences expose implementation modules | exact root/prelude lists plus downstream compile tests in Phase 1 |
| Counter or frame lifetime uses one vague global policy | operation owners and counter-specific failure taxonomy |
| Wyrd behavior parity hides broken continuation | uninterrupted-vs-restored matrix plus cross-field corruption cases |
| Human approval is implied rather than recorded | explicit dated Acceptance record and amendment rule |
| Planning evidence remains ignored/local-only | canonical plans/packets are unignored and required in the next handoff |

## Exit criteria

- [x] D1–D16 are accepted without unresolved ownership conflicts.
- [x] The crate-root list, prelude list, and stable namespace list are accepted.
- [x] All 151 pd-asteroids characterization tests are inventoried and assigned
  preserve/adapt/reject.
- [x] The 16 Sea of Grass wiring cases have named fixtures and expected traces in
  [`docs/wyrd-parity.md`](./docs/wyrd-parity.md).
- [x] Wyrd persistence continuation is recorded as a hard deletion gate.
- [x] App/World/Schedule ownership and structural mutation timing are unambiguous.
- [x] Core features are exactly `default = []`, `std = []`, and `testkit = []`.
- [x] Host-owned stages, state enums, platform FFI, and adapter repositories are named explicitly.
- [x] [Research packet 006](./.orchid/spec-research/006-single-crate-api-architecture/) is reviewed.
- [x] Every later phase links back to the architecture contract instead of redefining it.

## References

- [Moirai architecture](./docs/ARCHITECTURE.md)
- [pd-asteroids characterization ledger](./docs/parity.md)
- [Sea of Grass → Wyrd behavior and continuation parity](./docs/wyrd-parity.md)
- [Roadmap](./ROADMAP.md)
- [Research packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)
- [Rust visibility and privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
- [rustdoc re-exports](https://doc.rust-lang.org/rustdoc/write-documentation/re-exports.html)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Rust API naming guidelines](https://rust-lang.github.io/api-guidelines/naming.html)
- [Non-exhaustive APIs](https://rust-lang.github.io/rfcs/2008-non-exhaustive.html)
