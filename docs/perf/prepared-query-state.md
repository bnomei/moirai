# Reuse prepared query state across system runs

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

Repeated ad-hoc queries forced systems to reconstruct execution state and exposed cache choices at every call site. That makes the common one- and two-component loops harder to optimize than the specialized loops used by small game engines.

## Mechanism

`PreparedQuery1<T>` and `PreparedQuery2<A, B>` bind a resolved plan to one world and are affine, non-`Clone` values. `System::with_local` initializes them once through `SystemInitContext`; each run supplies only a `QueryWindow`. Q2 chooses the currently smaller component population as its driver for sparse, table, and mixed layouts while preserving A/B callback order. Mut/read execution stamps only A.

The supported policies are `Prepared`, `Membership`, `DeltaMembership`, and `Result`. The former ad-hoc world query entry points remain internal controls, not a compatibility API.

## Measurement

`benches/prepared_queries.rs` covers Q1/Q2 reads, Q1 mutation, Q2 mut/read, mut/mut and effects,
all sparse/table pairings, both population skews, four policies where temporally valid, All/Since/
Cursor windows, an asserted-empty cursor case, same-workload churn, and a feature-gated retained
ad-hoc control. Every mutation input executes once before timing so reusable entity scratch and
materialized state are warm. Host results collected on this workstation are provisional because
other processes were allowed to run and no endpoint was discarded.

The retained seven-cycle paired capture at
`target/perf-results/final-prepared-query1-20260714` ran with one-minute load readings from 11.243
through 12.024. Across those runs, the descriptive medians were 598.7 ns ad hoc versus 468.5 ns
prepared at 64 entities, and 28.20 us versus 27.95 us at 4,096 entities. Individual paired 4,096
deltas ranged from -2.64% through +1.21%. All endpoints are retained; these figures describe this
noisy host capture and are not a stable improvement claim or admission gate. The capture directory
is intentionally ignored build output rather than a source artifact.

Exact commands:

```text
cargo bench --features bench-internals --bench prepared_queries
cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control
cargo bench --features bench-internals --bench prepared_queries -- query2_read_all_population_matrix
cargo bench --features bench-internals --bench prepared_queries -- query1_read_all --sample-count 7 --max-time 0.1
```

Retained control/candidate capture and descriptive summary:

```sh
uv run python scripts/perf_experiment.py --group prepared-query1 --cycles 7 \
  --cohort final-prepared-query1-20260714 \
  --power-note "shared workstation; other processes allowed; retain every endpoint" \
  --output-dir target/perf-results/final-prepared-query1-20260714 \
  --baseline-command "env MOIRAI_QUERY_CONTROL=adhoc cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control --sample-count 7 --max-time 0.05" \
  --candidate-command "env MOIRAI_QUERY_CONTROL=prepared cargo bench --features bench-internals --bench prepared_queries -- query1_paired_control --sample-count 7 --max-time 0.05"
uv run python scripts/perf_summarize.py target/perf-results/final-prepared-query1-20260714
```

## Contract evidence

Unit and integration tests cover wrong-world rejection, Q2 cursor construction and commit, exact callback ordering, mixed mut/read ticks, and persistent system-local prepared state.
