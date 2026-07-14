# Express binary fixed-step cadence with a mask

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

Layered simulation commonly updates expensive systems every 2, 4, 8, or 16 fixed steps. Ad-hoc counters duplicate state and modulo work, and one-based step numbering makes phase zero surprising.

## Mechanism

Fixed-step indices are now zero-based. `Condition::fixed_step_mod(period, phase)` validates a nonzero power-of-two period and an in-range phase, then evaluates cadence with `index & (period - 1)`. It is always false outside `FixedUpdate`.

## Measurement

`runtime_support` measures condition evaluation. The consumer suite has both a consumer-local mask
loop and `grass_scheduled_fixed_cadence_with_local_cache`, which runs an actual `App` fixed schedule
with `Condition::fixed_step_mod`, `System::with_local`, a retained prepared query, and a retained
dense cache. Tests prove phases 0 and 4 across the first eight fixed steps and cover validation errors.

Seven-sample host spot medians for one due fixed step were 77.02 ns (period 1), 77.66 ns (period 8),
and 77.01 ns (period 64). The period-64 range widened to 69.21-336.1 ns while other processes were
active; the outlier is retained and the run remains provisional.

Exact commands:

```text
cargo bench --bench runtime_support -- cadence_condition
cargo bench --bench consumer_hot_paths -- grass
cargo test --test schedule fixed_step_mod
cargo bench --bench runtime_support -- cadence_condition_outside_fixed_update --sample-count 7 --max-time 0.1
cargo bench --bench runtime_support -- fixed_cadence_condition --sample-count 7 --max-time 0.1
```

The identical endpoints below characterize scheduler/capture noise only. Period cases remain
separate within each capture; source-revision comparisons must use matching prebuilt executables.

```sh
uv run python scripts/perf_experiment.py --group fixed-cadence --cycles 7 \
  --output-dir target/perf-results/fixed-cadence \
  --baseline-command "cargo bench --bench runtime_support -- fixed_cadence_condition --sample-count 100 --sample-size 100" \
  --candidate-command "cargo bench --bench runtime_support -- fixed_cadence_condition --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/fixed-cadence
```
