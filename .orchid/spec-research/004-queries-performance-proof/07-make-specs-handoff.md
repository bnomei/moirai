# Make Specs Handoff: 004-queries-performance-proof

## Status

- research_id: 004-queries-performance-proof
- status: frozen
- intended_spec_slug: queries-performance-proof
- shape_review: GREEN
- cheap_worker_ready: yes

## Objective

Implement a stable safe query facade with complete mixed-storage behavior, explicit filters,
owner-scoped membership/result caches, and reproducible hot-path proof.

## Requirements Seed

- R001: WHEN Query1/Query2 traverse mixed storage EACH matching live entity SHALL appear once.
- R002: WHEN mutable traversal runs REFERENCES SHALL be callback-scoped and non-aliasing.
- R003: WHEN the same component is requested mutably twice RESOLUTION SHALL fail before borrowing.
- R004: WHEN a cache handle crosses World/slot lifetime QUERY SHALL return an ownership/stale error.
- R005: WHEN relevant structure changes BOTH CACHE TYPES SHALL update/invalidate correctly.
- R005a: WHEN added/changed filters use QueryCache THEY SHALL apply the cursor window; result-cache
  use SHALL return MovingChangeWindow rather than panic.
- R006: WHEN user events clear CACHE COHERENCE SHALL remain correct.
- R007: WHEN a query/cache combination is unsupported IT SHALL return QueryError, never panic.
- R008: WHEN operation traces match ITERATION OUTPUT SHALL be deterministic.

## Suggested Task Slices

- T001: spec/params/resolution/errors.
- T002: immutable sparse/table/tag/mixed traversal.
- T003: safe closure-scoped mutation.
- T004: membership cache.
- T005: materialized-result cache and ownership.
- T006: model/property/API tests.
- T007: benchmarks and allocation proof.

## Validation

Use `PHASE_5_QUERIES.md#verification` and its full functional/cache/model matrices. Benchmark setup
separately from hot traversal and record allocations after warmup.

## Open

- Exact-id missing/stale option names and non-semantic iteration tie-break details.

## Worker Context Policy

Workers read `docs/ARCHITECTURE.md`, `PHASE_5_QUERIES.md`, and exact pd query sources/tests. Raw
source cache ids and mutable-pointer tricks are rejected constraints, not API requirements.
