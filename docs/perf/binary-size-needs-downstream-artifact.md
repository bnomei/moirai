# Binary-size decisions need a linked downstream artifact

Priority: medium
Confidence: high

Hotspot:

Moirai is a library for constrained and headless games (`Cargo.toml:6`) and `docs/perf.md:78` assigns host-shaped traces to Phase 7 downstream owners. The repository has no production `[[bin]]` or representative example executable, so its `.rlib` is not a shipped-size target.

Evidence:

- `Cargo.toml` declares the library's benches and tests but no linked application artifact.
- On the current host, `cargo build --release` produced `target/release/libmoirai.rlib` at 3,033,248 bytes; after `cargo build --release --all-features`, the top-level archive was 3,299,808 bytes.
- Those archives contain Rust metadata and linkable object code. Downstream feature selection, monomorphization, dead-code elimination, panic strategy, linker, symbols, target ABI, and packaging determine final shipped bytes, so the archive delta is not a final-binary result.
- No current report attributes `.text`, `.rodata`, unwind data, debug/symbol data, or packaged bytes. Applying `panic = "abort"`, stripping, LTO, boxing, or representation changes from the archive size alone would optimize an undefined metric.

Candidate and mechanism:

Make the first binary-size experiment in the Phase 7 host that actually ships Moirai. Pin the Moirai commit, target, features, linker, release profile, and representative reachable systems; record the unstripped linked artifact, stripped artifact, relevant sections/symbols, and final packaged bytes. Compare status quo with one candidate at a time, retaining a host output/correctness check.

Expected scope (not promised speedup):

This attributes size to code that survives downstream linking and exposes whether Moirai is material in the final package. It can distinguish library code size from host assets, runtime, symbols, and packaging.

Semantic and operational risks:

A tiny synthetic binary can let dead-code elimination remove most of the ECS and understate the real footprint. A large host can bury a meaningful Moirai delta in unrelated changes. Stripping reduces diagnostics; `panic = "abort"` changes unwinding/cleanup semantics; LTO raises build time and memory; type-layout changes can add allocation or indirection.

Benchmark plan:

1. Choose a Phase 7 host scenario that exercises the production component, query, schedule, command/event, and Q16 surfaces expected to ship.
2. Build from a clean, pinned checkout with the normal target directory and exact production target/profile/features. Record compiler and linker versions.
3. Capture file bytes before/after stripping, section sizes, largest retained symbols, and final package bytes. Validate the same scenario output.
4. Alternate baseline and one candidate build to detect linker/layout drift. Repeat on every shipped architecture affected by codegen or layout.
5. Reject a candidate if the linked/package delta is within run-to-run/link-layout noise, if runtime or diagnostics regress unacceptably, or if the effect disappears in the representative host.

Losing/crossover case:

For library distribution or build-cache pressure, `.rlib` bytes can still be a secondary diagnostic. They are not a proxy for a dead-code-eliminated, stripped, signed, or packaged game binary.

Result:

Deferred at an explicit downstream gate. This repository still has no shipping host binary, so the observed `.rlib` sizes cannot measure the requested outcome and no size-motivated source or profile change was made.

Decision and fallback:

Reopen only when a real shipping host links Moirai and supplies a pinned target, feature set, linker/profile, representative reachable systems, and pre/post linked section plus packaged-byte measurements. Until then, `.rlib` bytes are archive/cache diagnostics only; panic, LTO, stripping, boxing, and layout changes remain blocked.
