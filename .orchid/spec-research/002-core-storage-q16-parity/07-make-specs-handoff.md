# Make Specs Handoff: 002-core-storage-q16-parity

## Status

- research_id: 002-core-storage-q16-parity
- status: frozen
- intended_spec_slug: core-storage-q16-parity
- shape_review: GREEN
- cheap_worker_ready: yes

## Objective

Implement Moirai's checked opaque identities, exact component registration, private sparse storage,
and conventional always-available Q16 foundation.

## Requirements Seed

- R001: WHEN an entity is freed or reserved WORLD LIVENESS SHALL not report it as live.
- R002: WHEN generation cannot advance THE SLOT SHALL retire rather than wrap.
- R003: WHEN registration repeats with any conflicting property BUILD SHALL fail transactionally.
- R004: WHEN sparse lifecycle runs STALE IDS SHALL never observe a replacement entity.
- R005: WHEN Q16 arithmetic reaches a boundary ITS documented checked/saturating policy SHALL hold.
- R006: WHEN dividing Q16 by zero CHECKED DIVISION SHALL report failure.
- R007: WHEN rustdoc is generated allocator/registry/storage containers SHALL remain private.

## Decisions

- No global numeric feature.
- Reserved state anticipates deferred spawn but is not query-visible.
- Persistence never stores registry-local ids.
- Q16 and Wyrd Signal are semantically distinct.

## Suggested Task Slices

- T001: allocator and identity model.
- T002: checked component registry.
- T003: safe sparse storage and minimal final World integration.
- T004: Q16 implementation/reference tests.
- T005: public API, allocation, and benchmark proof.

## Validation

Use `PHASE_2_CORE_STORAGE.md#verification` plus randomized model tests, forced overflow,
mutation-on-error assertions, and generated-rustdoc review.

## Open

- No architecture blocker; implementation waits on the Phase 1 contract.

## Worker Context Policy

Workers read `docs/ARCHITECTURE.md`, `PHASE_2_CORE_STORAGE.md`, exact pd allocator/component/sparse
source files, and Wyrd signal conversion only for contrast. Do not copy source visibility or signal
arithmetic wholesale.
