# Performance baselines

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

## Regression gate (same machine)

1. Re-run `cargo bench` on the same host without changing power/thermal conditions.
2. Compare medians; investigate if median regresses more than ~10% without a documented safety/correctness trade.
3. Pair bench deltas with the release allocation test suite above for hot-path changes touching queries, events, commands, or schedule traversal.

## pd-asteroids comparison

No absolute pd-asteroids nanosecond parity is claimed yet. Equivalent workloads are grouped by family above (sparse query, table lifecycle, schedule update, Q16 math). Host-shaped Sea of Grass traces remain Phase 7 downstream owners.