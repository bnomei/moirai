# Performance baselines and capture protocol

Recorded on the Phase 6 closure machine for reproducible same-host comparison. These numbers are not portable nanosecond budgets; they anchor regression review on identical hardware, toolchain, and bench configuration.

## Environment

| Field | Value |
| --- | --- |
| Commit | `ab93dbb1d68796b2c5fbb9b5976f8e56834ad4f0` |
| Date | 2026-07-12 |
| Rust | `rustc 1.96.0 (ac68faa20 2026-05-25)` |
| OS | Darwin 25.5.0 arm64 |
| CPU | Apple M4 |
| Power mode | default (not low-power) |
| Command | `cargo bench` (release profile, Divan defaults) |
| Divan | `0.1.21` |
| Timer precision | 41 ns |

## Divan families (median unless noted)

### Q16 (`benches/q16.rs`)

| Case | Median | Mean | Notes |
| --- | ---: | ---: | --- |
| `q16_mul_chain` | 416.4 ns | 1.53 µs | 100 samples × 100 iters |

### Queries (`benches/queries.rs`)

Sparse world, 256 entities unless noted. Warm paths reuse resolved specs/caches across iterations.

| Case | Median | Mean | Notes |
| --- | ---: | ---: | --- |
| `cold_query1_sparse_resolve` | 60.14 µs | 61.96 µs | plan resolve + first traversal |
| `warm_query1_sparse` | 554.8 µs | 573.3 µs | repeated Query1 traversal |
| `warm_query2_sparse` | 794.5 µs | 1.093 ms | repeated Query2 traversal |
| `mixed_query2_warm` | 406.2 µs | 406 µs | sparse + table pairing |
| `warm_query_cache_hit` | 711.6 µs | 1.38 ms | membership cache hit path |
| `closure_mutation_sparse` | 456.5 µs | 453.5 µs | `for_each_mut` closure path |

### Schedule (`benches/schedule.rs`)

| Case | Median | Mean | Notes |
| --- | ---: | ---: | --- |
| `app_update` | 2.519 µs | 12.26 µs | noop Update stage + flush/clear seam |

### Storage (`benches/storage.rs`)

| Case | Median | Mean | Notes |
| --- | ---: | ---: | --- |
| `sparse_insert_lookup` | 14.04 µs | 18.57 µs | insert + get steady churn |

### World lifecycle (`benches/world_lifecycle.rs`)

| Case | Median | Mean | Notes |
| --- | ---: | ---: | --- |
| `table_insert_get` | 2.124 µs | 2.136 µs | table component round-trip |
| `deferred_command_flush` | 2.291 µs | 2.346 µs | empty command flush |
| `archetype_move_insert_second_table_component` | 2.937 µs | 18.79 µs | archetype migration on table growth |

## Allocation contract assumptions

Steady-state zero-allocation paths are validated in `tests/allocation.rs` under:

```sh
cargo test --release --features std --test allocation -- --test-threads=1
```

Warmup/reservation covers command buffers, event payload pools, schedule set gates, and query plan caches before measurement.

## Current descriptive protocol

Timing results are descriptive evidence, not an admission gate. Run seven positive paired cycles in
alternating AB/BA order. Preserve every endpoint, including failed commands, load spikes, thermal
drift, and outliers; do not reject, discard, or exclude a capture based on host load. The summary
separates endpoint statuses in statistics and carries both statuses into paired rows so failed
captures remain visible rather than silently influencing a clean-looking aggregate.

The prepared-query benchmark has a feature-gated retained ad-hoc control with the same Divan case
name as the prepared candidate. This gives `perf_summarize.py` directly pairable rows:

```sh
uv run python scripts/perf_experiment.py \
  --group prepared-query1 \
  --cycles 7 \
  --power-note "record current power and thermal context" \
  --baseline-command "env MOIRAI_QUERY_CONTROL=adhoc cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control --sample-count 100 --sample-size 100" \
  --candidate-command "env MOIRAI_QUERY_CONTROL=prepared cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results
```

Use a fresh `--output-dir` when an experiment should be summarized in isolation. Pair timing evidence
with the single-threaded release allocation contracts for query, event, command, schedule, scratch,
resource-scope, or system-local changes. A human may investigate a surprising distribution, but the
scripts intentionally emit no threshold, pass/fail decision, or automatic exclusion.

## pd-asteroids comparison

No absolute pd-asteroids nanosecond parity is claimed yet. Equivalent workloads are grouped by family above (sparse query, table lifecycle, schedule update, Q16 math). Host-shaped Sea of Grass traces remain Phase 7 downstream owners.
