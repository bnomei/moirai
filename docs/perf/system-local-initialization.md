# Initialize persistent system-local state once

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

Per-frame reconstruction of query handles, event readers, scratch buffers, and cache keys puts setup work directly in hot systems. Keeping those values outside the schedule loses ownership and initialization guarantees.

## Mechanism

`System::with_local` runs a fallible initializer exactly once during schedule construction and passes the resulting local by mutable reference on every execution. `SystemInitContext` exposes constrained resource reads, event-reader creation, and prepared-query construction without general world mutation.

All schedule validation precedes initialization. Initializers run before the execution lease is attached; a failure drops previously initialized locals and leaves the world able to build a replacement schedule.

## Measurement

`runtime_support::system_local_update` measures the steady run path after one untimed update. The
paired plain-closure and `System::with_local` app factories likewise run past startup before timing.
Prepared-query benchmarks separately measure the avoided prepare-each-execution work.

An earlier seven-sample host spot run reported a 36.22 ns median update (33.30-37.54 ns) for one
local-state system. It predates the explicit untimed warm update now performed by every input
factory, so it is retained as historical host evidence rather than attributed to the current steady
benchmark.

Exact commands:

```text
cargo bench --bench runtime_support -- system_local_update
cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control
cargo test --test schedule system_local
cargo bench --bench runtime_support -- system_local_update --sample-count 7 --max-time 0.1
```

```sh
uv run python scripts/perf_experiment.py --group system-local --cycles 7 \
  --output-dir target/perf-results/system-local \
  --baseline-command "env MOIRAI_SYSTEM_LOCAL_CONTROL=plain cargo bench --bench runtime_support -- system_local_paired_control --sample-count 100 --sample-size 100" \
  --candidate-command "env MOIRAI_SYSTEM_LOCAL_CONTROL=local cargo bench --bench runtime_support -- system_local_paired_control --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/system-local
```

The paired case performs the same captured-counter increment through a plain `FnMut` system and a
`System::with_local` system. It isolates the local-state dispatch cost; prepare-each-run versus
retained-query work is measured by `query1_paired_control` in the prepared-query document.
