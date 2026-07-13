# Coverage policy

Source-line coverage runs on current stable only. It does not define the Rust 1.75 library MSRV
contract.

Phase 1 establishes the crate boundary without executable ECS behavior. The 100% source-line gate
begins once owning phases land real code paths.

## Current snapshot (2026-07-13)

Merged 4-flavor union (`no-default-features`, `std`, `testkit`, `all-features`) reports
**100.00%** executable line coverage (10,652 / 10,652 DA lines; 0 intersection misses).

`cargo llvm-cov --all-features --summary-only` alone reports **97.58%** lines
(10,840 covered / 262 missed / 11,102 instrumented regions mapped to 10,840+262 line buckets).

Recent closure work: `emit_component_removed_if` helper for remove emit paths, `find_conflict` else-if
restructure + exact-repeat probe, Query2 skip iteration with primary-only plan, archetype migration
replace branch, table/tag/sparse remove emit-error tests, `send` Ok path via `.map(|_| ())`.

## Merge matrix (required before 100% sign-off)

Run and merge, per Phase 6:

1. `cargo llvm-cov --no-default-features`
2. `cargo llvm-cov --features std`
3. `cargo llvm-cov --features testkit`
4. `cargo llvm-cov --all-features`

Tooling-only exclusions must be line-specific and recorded here before claiming 100%.