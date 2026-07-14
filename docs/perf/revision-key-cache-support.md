# Provide checked revision keys for consumer caches

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

Consumer caches need a small explicit invalidation key without borrowing ECS internals or conflating cache state with change ticks. Hand-rolled integer tuples make overflow and dependency order inconsistent.

## Mechanism

`Revision` starts at `ZERO` and advances with checked overflow. `RevisionKey<N>` stores an ordered fixed-size dependency vector, supports equality/ordering/hash, and exposes the array without allocation. It is suitable for spatial grids, visible-set caches, render extraction, and layered simulation resources.

## Measurement

`runtime_support::revision_key_compare` sweeps repeated four-revision comparisons. `consumer_hot_paths` uses revision keys as part of an integrated grass-cache invalidation path.

The comparison benchmark black-boxes both operands dynamically and separates equal, unequal-first,
and unequal-last keys at 1, 8, and 64 repetitions. Earlier sub-nanosecond figures used compile-time
identical operands and are superseded; no speed claim is made until the corrected matrix is captured.

Exact commands:

```text
cargo bench --bench runtime_support -- revision_key_compare
cargo bench --bench consumer_hot_paths -- grass
cargo test --lib revision::tests
cargo bench --bench runtime_support -- revision_key_compare --sample-count 7 --max-time 0.1
```

The identical endpoints below characterize capture noise only. Equal and early/late-unequal cases
remain distinct within each endpoint; source-revision comparisons must use matching prebuilt
benchmark executables.

```sh
uv run python scripts/perf_experiment.py --group revision-key --cycles 7 \
  --output-dir target/perf-results/revision-key \
  --baseline-command "cargo bench --bench runtime_support -- revision_key_compare --sample-count 100 --sample-size 100" \
  --candidate-command "cargo bench --bench runtime_support -- revision_key_compare --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/revision-key
```
