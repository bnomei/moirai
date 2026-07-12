# Make Specs Handoff: 005-ecosystem-adapters-migration

## Status

- research_id: 005-ecosystem-adapters-migration
- status: frozen
- intended_spec_slug: ecosystem-adapters-migration
- shape_review: GREEN
- cheap_worker_ready: yes

## Objective

Deliver downstream-owned Wyrd/Anapao integration and evidence-gated pd-asteroids/Sea of Grass
migrations without adding sibling dependencies or host policy to Moirai.

## Requirements Seed

- R001: WHEN Wyrd runs from Moirai SAMPLE/LOOM/APPLY SHALL execute atomically in one default system.
- R002: WHEN a driver does not complete Apply ITS SettleTick SHALL not advance and it SHALL fault.
- R003: WHEN RuntimeState/driver envelope restores IT SHALL validate
  version/topology/numeric/phase/last-next invariants before mutating.
- R004: WHEN old Sea of Grass wiring is deleted ALL 16 behaviors and continuation cases SHALL pass.
- R005: WHEN replay captures IT SHALL observe final-flushed state before frame-event clear.
- R006: WHEN two seed-42 runs execute EXACT HOST SNAPSHOTS SHALL match step by step.
- R007: WHEN Anapao evaluates Moirai output IT SHALL use supported report/evaluator APIs, not a fake
  CompiledScenario.
- R008: WHEN either host ECS is deleted ITS path-dependency/dual-run/full-suite gate SHALL be
  archived.

## Decisions

- Wyrd and Anapao own adapters.
- Moirai owns only neutral testkit and runtime seams.
- Four tick domains remain independent.
- WorldMap remains a Sea of Grass resource.
- Apply precedes portal travel.
- Exact ECS replay is typed; Anapao receives selected scalar/report data.

## Suggested Spec/Task Fanout

- T001 (Wyrd): RuntimeState/fingerprint/restore/corruption matrix.
- T002 (Wyrd): Moirai atomic driver, last/next tick envelope, and numeric configurations.
- T003 (cross-repo): testkit-based canonical replay fixture.
- T004 (Anapao): report builder and external bridge.
- T005 (Sea of Grass): wiring fixtures and persistence.
- T006 (Sea of Grass): ECS domain migration/cutover.
- T007 (pd-asteroids): parity/path-dependency/cutover.
- T008 (cross-repo): canonical deterministic and restore proof.

T001 blocks old wiring deletion. The completed Phase 6 testkit and any Anapao builder work block
T004. Core host migrations can prepare in parallel but cannot cross their deletion gates early.

## Validation

Use every owning-repository check and deletion checklist in `PHASE_7_INTEGRATIONS.md`. Preserve
exact command/commit/trace evidence in the eventual specs/PRs.

## Open

- Exact Wyrd topology fingerprint format and Anapao builder API are upstream spec decisions.

## Worker Context Policy

Workers receive their repository's task, `docs/ARCHITECTURE.md` interop sections,
`docs/wyrd-parity.md`, `PHASE_7_INTEGRATIONS.md`, and exact sibling source contracts. Do not send
Moirai core workers to invent adapter or save semantics.
