# Changelog

All notable public API changes to Moirai are documented here. The crate follows [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html).

## 0.1.0-rc.1 — 2026-07-12

First explicit public baseline captured at Phase 6 quality closure.

### Added

- Neutral `moirai::testkit` replay driver, snapshot capture, and partial failure reports (`feature = "testkit"`).
- Query result cache, membership cache, and plan cache hot paths with steady-state allocation contracts.
- Typed event readers with payload pooling and frame/manual/bounded retention policies.
- Phase 6 verification script: `scripts/verify_phase6.sh`.

### Changed

- **Events:** `read_event` and `send` no longer require `Clone` on event types; payloads move between channel and reader without per-read cloning.
- **Events:** Unread channel payloads are recycled when no active readers remain (manual and bounded channels), keeping dispatch-only steady state allocation-free.
- **Queries:** `query`, `query2`, and `for_each_mut*` accept `&QuerySpec` to avoid per-call spec clones.
- **Schedule:** Set condition evaluation reuses per-set gate slots across updates.

### Compatibility notes (pd-asteroids adaptations)

- Runtime schedule cycles are build errors, not panics.
- Embedded raw schedule pointers and host-specific profiler hooks are rejected by design.
- Public internals (allocator, registry storage, command queue internals) remain sealed; extension uses documented builders and handles.

### Semver baseline

`cargo semver-checks` should be run against this release candidate tag before publishing `0.1.0`. Document intentional breaking changes here before each stable bump.