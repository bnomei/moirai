# Maintain delta membership from structural mutation logs

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

A cache called "delta" is not useful if any relevant topology revision silently triggers a full entity rescan. Games with a few structural changes per frame need work proportional to changed entities, not world size.

## Mechanism

The world records committed component-topology mutations as `(sequence, entity, component)` entries. Each live delta query owns a weakly registered cursor. Refresh filters the unseen entries by the plan's dependency set and updates a dense reverse slot index one entity at a time. Initial preparation is the only full materialization scan. Dropped cursors allow old log entries to be pruned.

Removal uses swap-remove and repairs the moved entity's reverse slot in O(1). Mutation-log entity
deduplication uses a dense reverse slot index and overwrites that slot with the newest entity
generation instead of performing a linear `contains` scan. Refresh converts each cursor sequence
directly into a `VecDeque` range offset, so a current query skips prefixes retained for a lagging
peer. Sequence space is rebased with every live cursor before exhaustion. Survivor order is not an
API contract. Despawn, deferred changes, slot-generation reuse, two-query lag, and sequence rebasing
are covered by tests.

## Measurement

`query2_same_workload_membership_churn` compares Prepared, Membership, DeltaMembership, and Result
policies over identical topology toggles, layouts, scales, and population skews. `consumer_hot_paths`
adds integrated churn cases. `query1_delta_current_vs_lagging` compares one unseen change against a
4,097-entry catch-up while both queries share the same retained log. The capture workflow retains
raw runs regardless of host load.

A seven-sample spot run reported a 60.08 us median (56.54-63.33 us) for one membership change plus
full traversal of 4,096 entities. It establishes a reproducible workload, not an isolated delta-log
cost or an admission threshold.

Exact commands:

```text
cargo bench --features bench-internals --bench prepared_queries -- query2_same_workload_membership_churn
cargo bench --features bench-internals --bench prepared_queries -- query1_delta_current_vs_lagging
cargo bench --bench consumer_hot_paths -- churn
cargo bench --features bench-internals --bench prepared_queries -- query2_same_workload_membership_churn --sample-count 7 --max-time 0.1
```

The identical endpoints below are an unchanged-command noise control, not an improvement estimate.
Policy deltas are visible as separate cases in each retained capture; revision-to-revision claims
must substitute matching prebuilt benchmark executables for the two commands.

```sh
uv run python scripts/perf_experiment.py --group delta-membership --cycles 7 \
  --output-dir target/perf-results/delta-membership \
  --baseline-command "cargo bench --features bench-internals --bench prepared_queries -- query2_same_workload_membership_churn --sample-count 100 --sample-size 100" \
  --candidate-command "cargo bench --features bench-internals --bench prepared_queries -- query2_same_workload_membership_churn --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/delta-membership
```
