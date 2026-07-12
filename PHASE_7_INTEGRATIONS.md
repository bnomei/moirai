# Phase 7 — Downstream adapters, persistence, and host migrations

**Status:** starts after Phase 6 core contract is green; upstream Wyrd persistence is a hard gate
**Depends on:** [Phase 6](./PHASE_6_QUALITY.md)
**Research:** [packet 005](./.orchid/spec-research/005-ecosystem-adapters-migration/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)
**Behavior contract:** [Sea of Grass → Wyrd parity](./docs/wyrd-parity.md)

## Goal

Make Moirai, Wyrd, and Anapao compose without merging their responsibilities, then migrate
pd-asteroids and Sea of Grass using path dependencies, trace comparison, persistence proof, and
explicit deletion gates.

This phase does not create additional Moirai workspace crates. Adapter work lands in the repository
that owns the semantics it adapts.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R701 | WHEN Wyrd runs on Moirai its default driver SHALL execute begin-frame/sample/loom/apply atomically. |
| R702 | WHEN a Wyrd driver step fails its next SettleTick SHALL not advance and the driver/App SHALL fault. |
| R703 | WHEN Wyrd RuntimeState/driver envelope restores version/topology/numeric/phase/tick invariants SHALL validate before mutation. |
| R704 | WHEN Sea of Grass wiring is deleted all 16 named traces and restore-continuation cases SHALL pass. |
| R705 | WHEN Sea of Grass PostAction runs Wyrd Apply SHALL complete before portal travel. |
| R706 | WHEN Sea of Grass ECS cuts over WorldMap/render/save boundaries SHALL remain host-owned and full tests/benches SHALL pass. |
| R707 | WHEN pd-asteroids cuts over its classified corpus/platform build/benches/path proof SHALL pass first. |
| R708 | WHEN ecosystem replay runs Phase 6 testkit SHALL be reused without sibling dependencies entering Moirai. |
| R709 | WHEN Anapao consumes replay IT SHALL use supported report/build/assertion APIs, never fake ScenarioSpec/NodeSnapshot. |
| R710 | WHEN seed 42 repeats ten-step exact snapshots and scalar reports SHALL match. |
| R711 | WHEN save/rebind/restore occurs after step five steps six-ten SHALL equal uninterrupted execution. |
| R712 | WHEN Phase 7 exits adapters SHALL remain downstream-owned and every old-code deletion SHALL have archived evidence. |

## Repository ownership

```text
moirai
  owns App/World/Schedule, resource scope, observation, neutral testkit

wyrd
  depends optionally on moirai
  owns WyrdDriver, binding semantics, SettleTick, runtime persistence

anapao
  depends on moirai testkit in its adapter/test support
  owns conversion to RunReport/BatchReport, assertions, events, artifacts

pd-asteroids / sea-of-grass
  own components, resources, state enums, stages, binding mappings, save schema
```

Moirai never depends on Wyrd or Anapao. Wyrd and Anapao types are not re-exported from Moirai's root
or prelude.

## Track A — Wyrd-owned Moirai adapter

The adapter lives beside `wyrd-for-games-bevy`, either as a Wyrd feature/module or a package such
as `wyrd-for-games-moirai`. Its public abstraction is an atomic driver, not three Bevy-shaped
systems.

Conceptual API:

```rust
pub struct SettleTick(u64);

pub trait WyrdBinding<P> {
    type Error;

    fn sample(
        &mut self,
        world: &mut moirai::World,
        ports: &P,
        writer: &mut wyrd::PortWriter<'_>,
    ) -> Result<(), Self::Error>;

    fn apply(
        &mut self,
        world: &mut moirai::World,
        ports: &P,
        outbox: wyrd::Outbox<'_>,
        tick: SettleTick,
    ) -> Result<(), Self::Error>;
}

pub struct WyrdDriver<P, B> {
    // bound Runtime, resolved ports, binding, last/next tick, sticky fault
}
```

Required construction paths:

- bind a dynamically authored `Weave` and resolve ports once;
- bind a typed `Recipe`;
- inspect runtime/ports/binding and last/next settle tick through narrow accessors;
- snapshot/restore adapter state only between completed driver steps.

One driver step is indivisible:

```text
begin_frame(SettleTick)
  → binding.sample(World)
  → Runtime::loom
  → binding.apply(World, borrowed Outbox)
  → commit last_completed_tick and advance next SettleTick
```

The outbox is consumed during Apply before the next `begin_frame` clears it. Any sample, loom, or
apply error leaves `next_settle_tick` unchanged and sticky-faults the driver. Sampling receives
mutable binding and World access, and `begin_frame` has already changed runtime frame state, so even
a sample error is not safely retryable. The adapter propagates that fatal system result, which also
terminally faults the containing App; it cannot silently retry or continue.

Before beginning a step, the driver checks that next SettleTick can advance after success.
Exhaustion faults before `begin_frame` and never wraps Wyrd temporal state.

The default schedule installation stores the driver as a typed host resource and invokes it through
`World::resource_scope` as one fallible Moirai system. This prevents a run condition, flush, or
failure from separating Loom and Apply. Split Sample/Loom/Apply systems are an advanced API only
when a host proves it needs those scheduling boundaries.

The adapter has its own Wyrd numeric feature matrix if required by Wyrd. Conversions are named by
signal domain:

- Bool ↔ zero/non-zero;
- Count ↔ raw integer count;
- Level ↔ normalized/fixed representation.

It does not reinterpret Moirai `Q16` as Wyrd Count.

## Tick contract

| Counter | Owner | First value | Advances |
| --- | --- | ---: | --- |
| `WorldTick` | Moirai World/App | 1 | each outer update |
| `FixedStep` | Moirai Schedule | 1 | each fixed substep |
| `SettleTick` | one WyrdDriver | 0 | only after successful sample/loom/apply |
| `StepIndex` | Moirai testkit/Anapao run | 0 | each requested replay step |

The first replay step may therefore observe `StepIndex(0)`, `WorldTick(1)`, and
`SettleTick(0)`. This is intentional. Multiple Wyrd drivers have independent settle counters.

## Track B — Wyrd persistence prerequisite

Sea of Grass currently saves wiring continuation state. Before its evaluator is deleted, Wyrd must
expose a versioned runtime snapshot/restore contract:

```rust
Runtime::snapshot_state() -> RuntimeState
Runtime::restore_state(&RuntimeState) -> Result<(), RuntimeStateError>
```

`RuntimeState` includes:

- held sense values;
- prior input/decrement/edge state;
- counters and flags;
- timers and OnStart state;
- delay buffers and heads;
- RNG state;
- an optional begun-frame/runtime tick and any other continuation cursor.

It excludes the ephemeral outbox and borrowed binding data. It carries:

- format version;
- stable full topology/layout fingerprint, not only endpoint manifest;
- numeric-path tag;
- lengths/invariants needed for complete validation.

Restore validates the entire snapshot before mutating Runtime. A mismatch applies nothing. The
adapter envelope is explicit:

```text
WyrdDriverState
├── runtime_state
├── last_completed_tick: Option<SettleTick>
└── next_settle_tick: SettleTick
```

It validates the fields together before mutating Runtime, driver, or host state:

- a fresh driver has `last_completed_tick = None`, `next_settle_tick = 0`, and RuntimeState records
  no begun frame;
- after Apply completes tick `n`, RuntimeState's frame tick and `last_completed_tick` both equal
  `n`, and `next_settle_tick == n.checked_add(1)`;
- a snapshot with individually valid fields but a mismatched phase/tick relation is rejected
  atomically;
- a faulted or mid-sample/loom/apply driver cannot produce a restorable snapshot.

The host stores binding/domain state and its own schema version alongside this validated envelope.

Required continuation tests compare uninterrupted execution with save/rebind/restore for:

- a latched flag;
- a mid-delay pulse;
- held senses and edge history;
- timer/OnStart progression;
- RNG sequence;
- initial/last/next tick relationship and cross-field corruption;
- topology mismatch rejection;
- numeric-path mismatch rejection;
- corrupt length/version rejection.

Save capture is legal only after a completed Apply and before the next begin-frame.

## Track C — Sea of Grass wiring behavior gate

Before deleting `sea-of-grass/src/wiring.rs` evaluation, map all sixteen named behaviors to a Wyrd
fixture or host integration test. The table reflects what the current source tests actually prove;
the ordered traces and limitations are frozen in [`docs/wyrd-parity.md`](./docs/wyrd-parity.md):

| Sea of Grass behavior | Replacement evidence |
| --- | --- |
| `validate_rejects_cycle` | invalid Weave cycle diagnostic |
| `lever_opens_door_latch` | legacy test manually manufactures latch/overlay; replacement splits real rise/toggle/set graph from host walkability/save |
| `settle_and_requires_both_levers` | And fan-in |
| `settle_or_either_lever_opens_door` | Or fan-in |
| `settle_plate_weight_barrel_opens_door` | Level sensor |
| `settle_crystal_lowers_barrier` | use edge → toggled Flag level → ordered host barrier edits |
| `delayed_pulse_does_not_and_with_live_false` | delayed Pulse over one Pass link; add a separate delayed/live And case only if required |
| `delayed_pulse_roundtrip` | uninterrupted versus fresh-runtime restore continuation with a pulse inside Delay storage |
| `portal_arm_activates_on_settle_before_travel` | legacy proves activation only; replacement must execute travel and trace Apply before observation |
| `plate_graph_validates` | accepted authored graph |
| `combine_mismatch_rejected` | fan-in policy diagnostic |
| `dead_actuator_rejected` | missing inbound link diagnostic |
| `portal_id_missing_rejected` | target validation |
| `tile_walkable_for_save_honors_open_doors` | apply mutation reflected in save view |
| `expand_structure_wiring_prefixes_ids` | stable pattern-instance id expansion |
| `e2e_app_lever_opens_door_and_saves` | legacy final-state/serialization check; replacement continues a fresh restored driver against a control |

These immediate behavior cases and Track B continuation cases are both required. Neither substitutes
for the other.

## Track D — Sea of Grass ECS migration

Sea of Grass defines its own operation-local schedule installer:

```text
Update operation
Input
→ Sim(Exposure → Recovery → Status)
→ Actions
→ PostAction(Wyrd driver → portal travel → world time → gravity → ...)
→ Generation
→ AI
→ Visibility
→ RenderExtract

Render operation
optional ECS presentation only; the current shell may render RenderSnapshot externally
```

Core Moirai has no Sea of Grass preset.

Migration rules:

- `RuntimeStep`, dirty-lane gating, `SimClock`, and host state remain host resources/policy;
- `WorldMap` stays a resource rather than becoming one ECS entity per tile;
- every custom host stage declares `StageOperation`; `RenderExtract` is Update-owned because it
  finalizes the shell snapshot, while actual presentation is Render-owned;
- Wyrd settles only on actual action steps, so skipped/render-only updates do not advance it;
- one dynamic driver is bound/restored per active level as required by current game ownership;
- Apply completes before portal travel;
- `RenderSnapshot` remains the shell boundary and Ratatui/crossterm stay outside simulation;
- host save ids/schema never persist Moirai's registry-local ids.

Migrate by coherent domain groups rather than “player first” placeholder slices: host shell/stages,
resources/events, entity families and queries, Wyrd mount/persistence, render extraction, then Bevy
removal. Each group must leave a runnable path-dependency build and trace evidence.

The Bevy ECS dependency is removed only when no simulation module imports it and full tests/benches
pass against Moirai.

## Track E — pd-asteroids migration

Use a local path dependency and, if it reduces churn, a temporary host-owned `crate::ecs` re-export
shim. The shim preserves intended imports, not private Moirai internals.

Migration sequence:

1. freeze source test and benchmark baselines;
2. map host `GameState` and eight-stage policy into pd-owned modules;
3. switch identity/storage/world/event/query behavior by coherent ownership group;
4. dual-run deterministic traces where both engines can be driven from the same fixture;
5. pass `game-core` tests and compile benches with Moirai;
6. verify Playdate platform adapters own clocks/profiling;
7. delete in-tree `ecs/` only after path-dependency proof;
8. pin a released Moirai version after the public API is baselined.

Raw allocator, registry, command variants, and cache keys are not added to Moirai merely to make the
shim compile. Host tests expecting those internals are adapted to public behavior or rejected in
the parity ledger.

## Track F — consume the Phase 6 neutral testkit

`moirai::testkit` is already implemented and green before this phase. Phase 7 consumes these
contracts:

- `StepIndex`;
- deterministic seeded driver/factory contracts;
- `ReplayConfig` and completion/failure policy;
- `StepSnapshot<S>` and `ReplayReport<S>` for host-defined exact snapshots;
- selected scalar `MetricSample` values;
- capture through `App::update_with` after flush and before frame-event clearing.

Exact replay uses `S: Eq`. The host defines and canonicalizes `S`, including sorting unordered
domain collections. Moirai cannot generically serialize or compare a type-erased World.

This phase may add ecosystem fixtures and host adapters, but it does not reopen the testkit's
neutral types or add Wyrd/Anapao dependencies to them.

## Track G — Anapao-owned bridge

Anapao's `Simulator` executes `CompiledScenario` and cannot run an arbitrary Moirai App. The bridge
therefore:

1. drives/captures through `moirai::testkit`;
2. maps explicitly selected scalar metrics/events into Anapao's public report vocabulary;
3. constructs `RunReport`/`BatchReport` through a public Anapao builder;
4. calls existing expectation evaluators;
5. emits existing Anapao events/artifacts.

It does not call `Simulator::compile` with a fake ECS scenario and does not coerce exact ECS
snapshots into Anapao `NodeSnapshot` (`NodeId → f64`). Exact equality remains in the Moirai replay
report; scalar expectations/artifacts are the Anapao view.

If Anapao's batch/report construction or aggregation remains private, expose a supported builder
there with parity tests before writing the bridge. Duplicating private aggregation semantics in
Moirai is rejected.

Anapao remains std-based and on its own MSRV. It does not enter Moirai's dependency graph.

## Canonical three-library proof

The ecosystem test is a seeded World containing a lever and Wyrd-controlled door:

1. construct the App, dynamic/typed Wyrd driver, binding, and host stages;
2. run ten action steps with seed 42;
3. capture exact canonical `DoorSnapshot` values post-flush each step;
4. rebuild and repeat seed 42; assert every snapshot equals;
5. map scalar `door.open` observations to Anapao expectations/artifacts;
6. repeat with snapshot/save after step five, rebind/restore, and run six through ten;
7. assert continuation equals uninterrupted output and tick domains match their contracts.

This proves composition without a shared base trait and exercises the persistence boundary that the
original 16 immediate tests do not cover.

## Deletion gates

### Old Sea of Grass wiring evaluator

- [ ] all 16 named behavior cases pass;
- [ ] Wyrd versioned snapshot/restore and corruption/mismatch tests pass;
- [ ] driver initial/last/next/runtime-tick cross-field corruption rejects atomically;
- [ ] uninterrupted/save-restore continuation matches;
- [ ] Wyrd Apply precedes portal travel;
- [ ] existing saves have a migration/rejection policy.

### Sea of Grass Bevy ECS

- [ ] full tests and host-shaped benches pass;
- [ ] deterministic traces match accepted behavior;
- [ ] no Bevy ECS import remains in simulation;
- [ ] WorldMap/render shell boundaries are preserved;
- [ ] save/load continuation passes.

### pd-asteroids in-tree ECS

- [ ] parity ledger is fully reconciled;
- [ ] `game-core` tests and bench compilation pass;
- [ ] host stages/state/platform hooks are downstream;
- [ ] path-dependency/dual-run evidence is archived;
- [ ] no local source code still requires deleted private internals.

No deletion gate is waived merely because an adapter compiles.

## Tasks

- [ ] **T701** Add Wyrd `RuntimeState` snapshot/restore and topology/numeric/phase validation
  upstream.
- [ ] **T702** Implement the Wyrd-owned Moirai adapter, atomic driver, and validated
  last-completed/next-tick envelope.
- [ ] **T703** Prove adapter numeric configurations in the Wyrd repository.
- [ ] **T704** Implement all 16 Sea of Grass fixtures plus continuation suite.
- [ ] **T705** Add Sea of Grass host stages/resources/events/entities/queries as coherent groups.
- [ ] **T706** Preserve Wyrd-before-portal order, WorldMap resource, and render snapshot boundary.
- [ ] **T707** Migrate pd-asteroids through a host shim/dual traces and gate ECS removal.
- [ ] **T708** Reuse Phase 6 testkit unchanged for the Wyrd/Anapao ecosystem proof.
- [ ] **T709** Expose an Anapao report/batch builder upstream if needed.
- [ ] **T710** Implement the Anapao-owned bridge using existing assertions/events/artifacts.
- [ ] **T711** Run the ten-step deterministic replay and step-five restore proof.
- [ ] **T712** Archive commands, commits, and trace artifacts for every deletion decision.

## Verification

Verification runs in each owning repository with local path dependencies before publication:

- Moirai: all Phase 6 feature/API/coverage gates plus testkit replay.
- Wyrd: both numeric adapter configurations, atomic driver faults, runtime state round trips, and
  cross-field tick/phase corruption rejection.
- Sea of Grass: wiring fixtures, full suite, save continuation, hot-path benches, no Bevy ECS
  imports after cutover.
- pd-asteroids: `game-core` tests, bench compilation, platform build, no in-tree ECS after cutover.
- Anapao: report builder parity, external Moirai replay bridge, expectations and artifact tests.

Record exact commands in the migration PRs/spec tasks rather than assuming one workspace command
can validate five repositories.

## Risks and controls

| Risk | Control |
| --- | --- |
| Adapter responsibility leaks into Moirai | downstream ownership and dependency-direction review |
| Three scheduled Wyrd phases separate atomic work | one driver system by default |
| World tick is reused as settle tick | independent counters and trace assertions |
| Immediate wiring parity loses save continuation | versioned runtime state hard gate |
| Anapao Simulator is misused as an ECS backend | neutral replay plus Anapao report bridge |
| Host migration forces private API exposure | adapt/reject ledger; host-owned shim only |
| Big-bang cutover loses evidence | coherent groups, path dependency, dual traces, explicit deletion gates |

## Exit criteria

- [ ] Moirai remains one dependency-pure crate.
- [ ] Wyrd owns a safe atomic Moirai driver with persistent continuation.
- [ ] Anapao owns a real external replay/report bridge rather than a duplicate simulator.
- [ ] The canonical three-library proof passes with deterministic and restore equivalence.
- [ ] Sea of Grass and pd-asteroids meet their independent deletion gates.
- [ ] No host-specific state, stage graph, save schema, platform FFI, or adapter type leaked into
  Moirai core.

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Phase 0](./PHASE_0_ANALYSIS.md)
- [Wyrd integration contract](./docs/ARCHITECTURE.md#wyrd-integration)
- [Sea of Grass → Wyrd behavior and continuation parity](./docs/wyrd-parity.md)
- [Anapao integration contract](./docs/ARCHITECTURE.md#anapao-integration)
