# Coverage policy

Source-line coverage runs on current stable only. It does not define the Rust 1.75 library MSRV
contract.

The Phase 6 gate is the union of four independently collected feature flavors:

1. `--no-default-features`
2. `--features std`
3. `--features testkit`
4. `--all-features`

Run the complete, reproducible gate with:

```sh
scripts/verify_coverage_union.sh
```

`scripts/verify_phase6.sh` includes this command. The runner requires `cargo-llvm-cov` and `uv`.

## Source-line contract

Each flavor starts with `cargo llvm-cov clean --workspace`, runs its tests with `--no-report`, and
then exports both LCOV and full LLVM JSON before the next flavor is cleaned. Every non-clean
`cargo llvm-cov` invocation applies `--ignore-filename-regex '(^|/)src/examples(/|$)'`, so the
documentation-only lesson hierarchy cannot enter either report format. The gate uses LCOV `DA`
records because they are LLVM's explicit source-line/count representation. A
canonical production source line is executable when at least one flavor emits a `DA` record for it,
and covered when any flavor reports a positive count. Generic monomorphizations and repeated records
therefore cannot inflate the denominator.

Only production Rust files under `src/` participate. The analyzer parses each `cfg` predicate and
balances an item independently when that predicate cannot be true with `test=false`. This includes
`cfg(test)` and predicates such as `cfg(all(test, feature = "std"))` without dropping production
items that follow them. Test-only external module declarations are resolved using Rust's module path
rules, and those files and their external child modules are excluded transitively. Predicates such
as `cfg(any(test, feature = "testkit"))` and `cfg(not(test))` remain in the denominator because they
can ship in a non-test build. Unsupported cfg syntax fails the audit rather than guessing.

`src/bench_internals.rs` is the sole whole-file exclusion: it contains benchmark-only implementation
enabled by the private `bench-internals` feature and is not production runtime code. The whole
`src/examples/` directory is excluded separately because it contains documentation-only lessons
validated through stable package doctests. Both exclusions are recorded with their reasons in
`summary.json` and `manifest.json`; the analyzer also fails closed if an LCOV or JSON export still
contains a `src/examples/` source record.

There are no coverage attributes or production-code coverage suppressions. Any future tooling-only
or documentation-only exclusion must be narrow, reviewed, and documented here before a 100% result
can be claimed.

## Evidence and failure behavior

A successful or coverage-missing run publishes one internally consistent directory at
`target/coverage-union/`:

- `flavors/{no-default,std,testkit,all-features}.lcov`
- `flavors/{no-default,std,testkit,all-features}.json`
- `summary.json`
- `missing-lines.txt`
- `manifest.json`

The manifest records the Git revision and dirty state, `cargo-llvm-cov` and `rustc` versions, exact
flavor commands, SHA-256 digests for every evidence file, and the audited test-only cfg ranges. The
runner uses NUL-delimited `git ls-files --cached --others --exclude-standard` output to hash every
tracked and unignored untracked repository input before the first clean and after the last flavor
export. This includes tests, examples, benchmarks, fixtures and documentation consumed by tests,
build scripts, Cargo and toolchain configuration, and coverage tooling. Ignored generated output such
as `target/` is omitted naturally. Any visible path or content change aborts before analysis or
publication; the analyzer checks the same digest again and records it in the manifest. A lock, fresh
staging directory, and directory-level replacement prevent interrupted or concurrent runs from
publishing mixed evidence.

The command exits non-zero when `missing-lines.txt` is non-empty, after publishing the coherent
artifacts needed to diagnose the miss. `summary.json` is the authoritative current result; snapshot
numbers in prose are intentionally avoided so documentation cannot outlive the measured evidence.
