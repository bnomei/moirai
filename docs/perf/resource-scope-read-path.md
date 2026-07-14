# Separate immutable and mutable resource scopes

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

The former resource scope always exposed `&mut R` and therefore had to advance the resource change tick even for read-only cache lookups. That produced false invalidation and consumed global tick budget.

## Mechanism

`resource_scope_ref` temporarily removes a resource to permit access to the rest of the world, restores its original added/changed metadata, and issues no tick. `resource_scope_mut` issues exactly one changed tick when the resource is present and no tick when it is absent. Both paths reject same-resource reborrowing and restore safely during unwinding.

The scope guard retains the original erased `Box<dyn Any>` and borrows the typed value through a
checked downcast. Restoration moves that same box back into the store, so both present scope paths
perform zero allocations and zero deallocations after resource construction.

## Measurement

`runtime_support` isolates present immutable and mutable scopes. Integration tests assert exact tick preservation/advancement, missing-resource behavior, reborrow rejection, exhaustion, unwind restoration, and drop counts.
The release allocation contract asserts a strict zero allocation/deallocation lifecycle for one
immutable plus one mutable warmed scope.

An earlier seven-sample host spot run, before erased-box reuse landed, reported 23.86 ns median for
immutable scope and 21.59 ns for mutable scope. Those allocation-bearing measurements are retained
as historical evidence but do not describe the current implementation; the commands below are the
reproducible current workload. The immutable path's primary contract remains correct invalidation
behavior plus zero steady allocation.

Exact commands:

```text
cargo bench --bench runtime_support -- resource_scope
cargo test --test resources resource_scope
cargo bench --bench runtime_support -- resource_scope --sample-count 7 --max-time 0.1
```

The identical endpoints below characterize capture noise only. Immutable and mutable scope cases
are reported separately inside each endpoint; source-revision comparisons must use matching
prebuilt benchmark executables.

```sh
uv run python scripts/perf_experiment.py --group resource-scopes --cycles 7 \
  --output-dir target/perf-results/resource-scopes \
  --baseline-command "cargo bench --bench runtime_support -- resource_scope --sample-count 100 --sample-size 100" \
  --candidate-command "cargo bench --bench runtime_support -- resource_scope --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/resource-scopes
```
