# Make Specs Handoff: 006-single-crate-api-architecture

## Status

- research_id: 006-single-crate-api-architecture
- status: frozen
- intended_spec_slug: single-crate-api-architecture
- shape_review: GREEN
- cheap_worker_ready: yes

## Objective

Freeze Moirai as one dependency-pure crate with private deep internals, a curated facade, safe
App/World/Schedule ownership, numeric-agnostic ECS behavior, and honest downstream seams for Wyrd
and Anapao.

## Requirements Seed

- R001: WHEN Moirai exposes public API THE CRATE SHALL publish only semantic modules, curated root
  conveniences, and a smaller prelude.
- R002: WHEN systems execute THE APP SHALL safely borrow sibling World and Schedule fields without
  unsafe code or an embedded schedule.
- R003: WHEN numeric components are stored THE WORLD SHALL permit f32, integer, and Q16 values in
  the same build.
- R004: WHEN setup completes THE BUILDER SHALL reject registration and schedule conflicts before
  execution.
- R005: WHEN ecosystem adapters are implemented THEY SHALL remain downstream-owned and use stable
  Moirai seams.
- R006: WHEN SoG wiring is removed WYRD SHALL first prove versioned restore continuation.
- R007: WHEN custom stages/frame events are configured THEIR Update/Render owner SHALL determine App
  execution, structural authority, observation, and clearing.
- R008: WHEN temporal counters/cursors advance THEIR exact windows and exhaustion policy SHALL be
  explicit and non-wrapping.
- R009: WHEN Wyrd driver state restores RuntimeState/last-completed/next tick SHALL validate together
  before mutation.

## Scope

In scope:
- `docs/ARCHITECTURE.md` contract and corresponding Phase 0–7 rewrites.
- Module visibility, facade/prelude, ownership, features, Q16, safe schedule, testkit, adapters.

Out of scope:
- Production implementation and host cutovers.

## Current-State Facts

- pd-asteroids embeds Schedule in World and uses unsafe raw pointers during safe callbacks.
- Wyrd and Anapao already use private implementations plus curated facade patterns.
- Wyrd Signal count/level semantics are not a general Q16 API.
- Anapao Simulator cannot run external World values.
- SoG WiringSave persists state Wyrd cannot currently snapshot.

## Decisions

- One Moirai crate through 1.0; App owns sibling World/Schedule.
- Core features are additive; Q16 is unconditional.
- Wyrd and Anapao own adapters; Moirai owns only neutral seams/testkit.
- Source tests are classified, not blindly frozen.
- Update owns structural Commands/flush; Render is topology-read-only; App rejects pending idle
  batches rather than adopting them.
- The initial 151-row classification is `docs/parity.md`; Phase 6 reconciles it against landed
  tests.

Rejected:
- Embedded schedule, microcrates, global numeric features, host state/stages in core, fake Anapao
  scenarios, and behavior-only SoG deletion.

Open:
- None architecturally; Phase 0 is accepted and implementation follows the Phase dependency graph.

## Implementation Shape Excerpts

- Copy module tree, dependency direction, facade lists, lifecycle, features, tick table, Wyrd
  persistence gate, and Anapao boundary from `docs/ARCHITECTURE.md` into the phase specs.

## Suggested Spec Shape

- spec_kind: architecture-contract
- fanout_policy: phase-owned sequential dependencies with Phase 4/5 overlap after World
- execution_policy: implementation begins with Phase 1 and follows the Phase dependency graph
- task_slices:
  - T001: facade and feature contract
  - T002: checked core data model
  - T003: World structural completeness
  - T004: safe App and compiled schedule
  - T005: query completion
  - T006: neutral testkit, parity, and quality closure
  - T007: downstream interop and migration using the frozen testkit

## Validation

- Validate every research packet.
- Check Markdown references and `git diff --check`.
- Future implementation validation is listed in `docs/ARCHITECTURE.md` and each phase.

## Worker Context Policy

- Workers may read:
  - `docs/ARCHITECTURE.md`
  - their assigned `PHASE_*.md`
  - exact source modules named by that phase
- Workers must not be sent to:
  - raw/
  - broad current-state research
  - rejected alternatives
