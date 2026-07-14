# Changelog

All notable public API changes to Moirai are documented here. The crate follows [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html).

## Unreleased

### Added

- Canonical `moirai::examples` Rustdoc lessons organized into ordered tiers, with stable runnable
  doctests covering world construction, scheduling, queries, constrained host data, and replay.

### Removed

- Public test-only fault injection and inspection traits `WorldTestExt` and `ScheduleTestExt`;
  repository failure-path tests now use crate-internal test support.
- The accidentally public `schedule::RunContext` execution scratch type, which was not part of
  host authoring.

## 0.1.0-rc.1 — 2026-07-12

First explicit public baseline captured at Phase 6 quality closure.

### Added

- Neutral `moirai::testkit` replay driver, snapshot capture, and partial failure reports (`feature = "testkit"`).
- `WorldTestExt` and `ScheduleTestExt` test controls, available only through `moirai::testkit`.
- Checked `Schedule::stage_id`/`stage_label` lookup around opaque `StageId` handles.
- Entity-only and runtime-component-id queries plus owner-bound `EntityScratch`.
- Query result cache, membership cache, and plan cache hot paths with steady-state allocation contracts.
- Typed event readers with payload pooling and frame/manual/bounded retention policies.
- Phase 6 verification script: `scripts/verify_phase6.sh`.

### Changed

- **Events:** registered payloads use one explicit `Clone + 'static` broadcast contract; every
  independent reader owns its cloned payload and frame events remain until their operation clears.
- **Schedule:** systems declare typed event emission/consumption roles, which are validated at build
  time and enforced during execution.
- **Facade:** `Bundle` is available at the crate root and in the prelude; `BundleWriter` remains in
  the advanced `moirai::world` namespace.
- **Queries:** `query`, `query2`, and `for_each_mut*` accept `&QuerySpec` to avoid per-call spec clones.
- **Schedule:** Set condition evaluation reuses per-set gate slots across updates.

### Compatibility notes (pd-asteroids adaptations)

- Runtime schedule cycles are build errors, not panics.
- Embedded raw schedule pointers and host-specific profiler hooks are rejected by design.
- Public internals (allocator, registry storage, command queue internals) remain sealed; extension uses documented builders and handles.

### Semver baseline

`cargo semver-checks` should be run against this release candidate tag before publishing `0.1.0`. Document intentional breaking changes here before each stable bump.

This release candidate does not claim a Sea of Grass or pd-asteroids cutover, nor does this quality
reconciliation establish a new performance result.
