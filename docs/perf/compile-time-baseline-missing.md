# Compile-time tuning lacks a clean and incremental baseline

Priority: low
Confidence: high

Hotspot:

The repository has no recorded clean-build or representative incremental-build metric. That makes release-profile, feature, generic, or dependency changes impossible to evaluate as compile-time improvements.

Evidence:

- `Cargo.toml:11-21` has three empty features, no normal dependencies, and no custom profiles. The current source does not identify a likely dependency or feature bottleneck.
- CI compiles all targets/features and benchmark harnesses, but it does not save Cargo timings or compare build duration (`.github/workflows/ci.yml:125-137` is compile-only for benches).
- On the already-populated local target directory, `cargo build --release --timings` completed in 0.01 s. Switching to `cargo build --release --all-features --timings` rebuilt the crate in 1.28 s. These are useful environment observations, not clean/incremental baselines: cache state and feature switching differ.
- Cargo timing reports were generated under `target/cargo-timings/`, proving the built-in attribution surface is available without introducing another tool or target directory.

Candidate and mechanism:

Define a two-part compile benchmark before tuning: a dedicated clean release build for the no-default and shipped feature sets, plus a representative incremental rebuild after a small edit in a commonly touched module. Record wall/user time, peak RSS, Cargo timing output, rustc, target, features, and linker. Only then evaluate a concrete mechanism such as generic-body extraction, feature reduction, ThinLTO, or codegen-unit changes.

Expected scope (not promised speedup):

This establishes whether compile time is a material constraint and attributes it to rustc, linking, tests/benches, or a feature set. Given the dependency-free 1.28 s observed rebuild, optimization priority may remain low.

Semantic and operational risks:

`cargo clean` invalidates the shared repository cache and should be reserved for a coordinated measurement run, not routine validation. Incremental timing depends strongly on which file changes. LTO and fewer codegen units may improve runtime while worsening clean build/link time and peak memory.

Benchmark plan:

1. On an otherwise idle machine, record the toolchain, target, feature set, and target-directory state.
2. In a coordinated run using the repository's normal `target/`, run `cargo clean` once, then `/usr/bin/time -lp cargo build --release --timings --no-default-features`.
3. Repeat from clean enough times to characterize variance, then time a no-op rebuild and a documented representative source edit/rebuild.
4. Repeat only for feature sets that ship; keep bench/test build time separate from library build time.
5. If testing profile changes, compare status quo, ThinLTO, and codegen-units independently while recording runtime and linked size. Reject changes whose build cost lacks a measured runtime/size benefit.

Losing/crossover case:

For this small dependency-free crate, the measurement protocol can cost more engineering time than the build itself. Compile-time tuning should lose priority to runtime correctness and representative target measurements unless the downstream workspace demonstrates a bottleneck.

Result:

Accepted as a baseline, with no optimization candidate justified. After the shared repository `target/` was externally cleared, `/usr/bin/time -lp cargo build --release --timings --no-default-features` completed the clean library build in 1.25 s wall time (4.24 s user, 0.11 s system); macOS denied the final `kern.clockrate` query, so `/usr/bin/time` returned 1 after Cargo itself succeeded. An immediate no-op rebuild took 0.03 s wall time. Touching the commonly edited `src/query/mod.rs` and rebuilding took 1.14 s wall time (4.29 s user, 0.09 s system). Cargo timing HTML was retained under the normal `target/cargo-timings/` path during validation.

Decision and fallback:

Keep Cargo's current profiles and codegen defaults. The dependency-free crate does not show a material compile-time bottleneck, so LTO, codegen-unit, debug-info, feature, or generic refactors would add tradeoffs without an identified problem. Repeat this exact clean/warm/incremental protocol only for a demonstrated regression or a shipping downstream workspace.
