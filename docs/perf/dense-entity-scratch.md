# Use dense generational entity scratch storage

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

Tree-backed transient entity maps add comparison and allocation overhead to collision candidates, visibility sets, and per-frame accumulators. These consumers already have dense entity slots and do not need arbitrary ordered keys.

## Mechanism

`DenseEntityScratch<V>` indexes values by entity slot, records the full generation, and binds to one world owner. A dense active-slot list makes clear and liveness retention proportional to stored values. Per-slot active indices make removal O(1) with swap-index repair. Stale generations never alias reused entity slots.

## Measurement

`runtime_support` sweeps 64, 1,024, and 16,384 live values for insert/get/clear and sparse liveness retention. `consumer_hot_paths` uses the same structure inside asteroid- and grass-shaped loops.

Seven-sample spot medians for insert + get + clear were 218.5 ns at 64 values, 3.957 us at 1,024,
and 60.83 us at 16,384. These direct spot results are provisional. Use the paired capture protocol
when raw output and endpoint metadata must be retained for later inspection.

Exact commands:

```text
cargo bench --bench runtime_support -- dense_scratch
cargo bench --bench consumer_hot_paths
cargo test --test world_lifecycle_state entity_scratch
cargo bench --bench runtime_support -- dense_scratch_insert_get_clear --sample-count 7 --max-time 0.1
```

The identical endpoints below characterize capture noise only. They do not compare dense scratch
against the removed tree-backed implementation; source-revision comparisons must use matching
prebuilt benchmark executables.

```sh
uv run python scripts/perf_experiment.py --group dense-scratch --cycles 7 \
  --output-dir target/perf-results/dense-scratch \
  --baseline-command "cargo bench --bench runtime_support -- dense_scratch_insert_get_clear --sample-count 100 --sample-size 100" \
  --candidate-command "cargo bench --bench runtime_support -- dense_scratch_insert_get_clear --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/dense-scratch
```
