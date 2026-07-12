# Implementation Shape

The canonical detailed shape is `docs/ARCHITECTURE.md`. This artifact distills worker boundaries.

## Ownership

- `src/app.rs`: App/AppBuilder and frame lifecycle.
- `src/operation.rs`: dependency-neutral StageOperation shared by schedule/event policy.
- `src/world/`: ECS data, safe access, immediate setup mutation, command flush, resource/event APIs.
- `src/schedule/`: authoring builder, validation, dense compiled order, systems/conditions, safe run.
- `src/world/query/`: private query implementation; `src/query.rs` is the stable facade.
- `src/entity`, `component`, `event`, `command`, `math`: semantic facades over private internals.
- `src/testkit`: optional neutral replay; no sibling dependency.

World must not contain Schedule, systems, or a platform clock. It may own private query-cache
entries because cache lifetime follows World structure; public handles remain opaque and
owner-scoped. Schedule may execute a World. App is the only normal owner of both.

## Public contracts

- Root: App/AppBuilder, World/WorldBuilder, ids/options, Commands/Bundle/DynamicBundle,
  Schedule/System/StageId/StageOperation types, query vocabulary, generic State, time vocabulary.
- Prelude: system-authoring subset only.
- Public structs have private fields; growing error enums are non-exhaustive.
- Root, namespace, prelude, README, and doctest imports are tested as downstream code.

## Data/control flow

```text
AppBuilder registers typed world data + authored schedule
  → build validates conflicts/resources/events/operation-local order/cycles
  → App { World, compiled Schedule }
  → reject any pending idle command batch
  → update increments WorldTick and runs only Update-owned stages
  → systems enqueue structural Commands
  → configured flush points apply commands
  → observation hook runs after final flush
  → all queued Update-owned frame events clear/compact
  → render runs topology-read-only Render-owned stages, observes, clears Render-owned frame events
```

Advanced construction is `WorldBuilder::build → ScheduleBuilder::build(&mut World) →
App::from_parts`. Schedule execution remains crate-private so App is the sole lifecycle owner.
World ChangeTick drives added/changed resource/component windows independently of outer WorldTick.
Those windows are `(since, captured_now]` and cursor progress commits only on full traversal.
EventReader/query handles use Rc owner/cursor tokens, while queues/World retain Weak references
where lifecycle cleanup requires them.

## Interop

- Wyrd-owned driver is an atomic resource-scoped system ordered by host policy. It owns last/next
  SettleTick and awaits upstream versioned snapshot/restore with cross-field phase validation before
  SoG migration.
- `moirai::testkit` drives App through public seams, captures exact host-defined snapshots and
  scalar metrics, and observes after flush.
- Anapao-owned adapter maps only scalar metrics/events to Anapao reports/assertions/artifacts; exact
  ECS snapshots remain typed Moirai test evidence.

## Vertical slices

1. Facade/visibility/feature contract and compile tests.
2. Checked entity/component registration plus a minimal final-type sparse World path and Q16.
3. Tables/archetypes, immediate/deferred mutation, typed resources/events, bundles, lifecycle
   events.
4. Safe compiled Schedule and App with generic state/fixed step/observer hook.
5. Query facade, owner-scoped state/caches, mixed storage and mutation seams.
6. Neutral replay testkit, classified parity closure, safety regressions, and
   API/coverage/performance proof.
7. Downstream adapters, persistence continuation, canonical ecosystem replay, and host cutovers.

## Validation

- Core Rust 1.75 no_std library check; stable std/testkit tests.
- `cargo fmt`, strict Clippy, tests, doctests, docs, benches compile.
- Source-line coverage plus targeted state-machine/property tests for allocator, registration,
  schedule, cache, and Wyrd restore continuations.
- No `--all-features` exception is needed inside Moirai because its features remain additive.

## Stop conditions

- Stop if a worker needs to expose storage/registry internals to finish a public API.
- Stop if World begins owning Schedule or unsafe code is introduced.
- Stop if a host-specific stage/state/persistence policy enters core.
- Stop SoG wiring deletion until Wyrd restore continuation is proven.
