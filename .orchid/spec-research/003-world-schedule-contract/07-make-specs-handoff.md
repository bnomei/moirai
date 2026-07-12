# Make Specs Handoff: 003-world-schedule-contract

## Status

- research_id: 003-world-schedule-contract
- status: frozen
- intended_spec_slug: world-schedule-contract
- shape_review: GREEN
- cheap_worker_ready: yes

## Objective

Implement complete World data semantics and a separately owned validated Schedule/App lifecycle in
safe Rust.

## Requirements Seed

- R001: WHEN World is idle structural methods SHALL commit immediately.
- R002: WHEN World is running immediate structural methods SHALL reject; Update SHALL defer through
  Commands and Render SHALL reject Commands.
- R003: WHEN a command batch is invalid NO structural operation in that batch SHALL commit.
- R004: WHEN Schedule builds cycles/missing declarations SHALL fail before first update.
- R005: WHEN App updates World and Schedule SHALL be mutably split without unsafe code.
- R006: WHEN observation runs Update SHALL be post-flush and Render SHALL be post-system, both before
  their operation-owned frame clear.
- R007: WHEN a system/flush fails APP SHALL record a fault and not silently continue.
- R008: WHEN fixed substeps run THEIR counter SHALL be independent from outer WorldTick.
- R009: WHEN resources are mutated THEIR change ticks SHALL support generic conditions.
- R010: WHEN advanced parts are composed THEIR ExecutionLease SHALL match and Schedule run SHALL
  remain crate-private.
- R011: WHEN event readers are dropped WEAK CURSOR TRACKING SHALL stop blocking compaction.
- R012: WHEN custom stages/frame events are configured THEIR Update/Render owner SHALL determine
  execution and clearing, including prequeued external input.
- R013: WHEN App sees an idle command batch IT SHALL reject before execution until explicit
  flush/discard.
- R014: WHEN counters exhaust App clocks, ChangeTick, cache slots, entities, and event channels SHALL
  follow their separate non-wrapping policies.

## Decisions

- App is the root runtime convenience.
- World has no Schedule edge.
- Standard Update stages are Startup/FixedUpdate/Update; Render is a separate topology-read-only
  operation.
- `State<S>` and host-installed transitions replace `GameState`.
- Platform diagnostics and host stages remain downstream.

## Suggested Task Slices

- T001: archetypes/bundles/immediate lifecycle.
- T002: Commands transactional preflight/commit.
- T003: typed resources/events/change/event-lifetime semantics.
- T004: Schedule/System graph build.
- T005: operation-local App update/render/fixed/flush/observation/frame lifetime.
- T006: faults, diagnostics, public API, state-machine tests, benches.

## Validation

Run both Phase 3 and Phase 4 verification sections. Add trace tests for all ordering boundaries and
state-before/state-after assertions for rejected batches and execution faults.

## Open

- No architecture blocker; implementation waits on Phases 1–2.

## Worker Context Policy

Workers read `docs/ARCHITECTURE.md`, Phases 3–4, and exact pd World/Schedule sources. Source unsafe
blocks and host presets are evidence to replace, never code-shape instructions.
