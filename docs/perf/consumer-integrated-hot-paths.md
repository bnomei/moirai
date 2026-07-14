# Measure asteroid and grass systems as integrated hot paths

Status: implemented; host timings are provisional and are not an admission gate.

## Finding

Primitive microbenchmarks cannot show whether prepared queries, scratch storage, cadence, resource
scopes, and revision keys compose efficiently in the systems that motivated them. Pd-asteroids gains
speed from intentionally narrow one- and two-component loops; Sea of Grass builds visibility and
simulation caches around Bevy rather than asking a general ECS query to solve every problem.

## Mechanism

`benches/consumer_hot_paths.rs` keeps persistent prepared state and preallocated buffers outside the
timed body. Input construction runs the asteroid simulation once, including mutation scratch and
collision staging, before steady timing; delta-policy inputs also complete and restore a churn
cycle. Both local-mask grass paths run cache-hit and invalidation warmups before timing. Asteroid
cases combine 1/64-step Q2 mut/read movement, collision-candidate staging,
`DenseEntityScratch`, population skew, three storage layouts, and constant-population structural
churn. Grass cases combine layered power-of-two cadence, membership-prepared Q2, a dense visible
subset, per-blade scratch cache, `RevisionKey<2>`, and immutable/mutable resource scopes.
The scheduled grass case runs the same ingredients through an actual fixed-step `App` with
`Condition::fixed_step_mod` and persistent `System::with_local` state; the local-mask cases remain
useful controls that isolate consumer work from scheduler dispatch.

These are supported shapes, not promises that Moirai handles every ECS query form. The public API
stays focused on prepared Q1/Q2 execution and explicit consumer-owned caches.

## Measurement

Earlier seven-sample asteroid medians ranged from 19.24 to 24.20 us across the six 1,024-dense and
4,096-quarter-density layout cases. Grass cache-hit medians ranged from 9.499 to 28.29 us across the
six cadence/visibility/layout cases. Those captures predate the explicit input warmups now described
above, so they remain historical workload evidence rather than current steady results. Other
processes were allowed to run and no endpoints were discarded.

Exact commands:

```text
cargo bench --bench consumer_hot_paths -- --test
cargo bench --bench consumer_hot_paths -- asteroid_movement_collision_steady --sample-count 7 --max-time 0.1
cargo bench --bench consumer_hot_paths -- grass_layered_cadenced_cache_hit --sample-count 7 --max-time 0.1
cargo bench --bench consumer_hot_paths -- grass_scheduled_fixed_cadence_with_local_cache --sample-count 7 --max-time 0.1
```

Seven-cycle unchanged-control capture characterizes current host variance while retaining every
endpoint; use matching prebuilt revision executables to compare source revisions:

```sh
uv run python scripts/perf_experiment.py --group consumer-hot-paths --cycles 7 \
  --output-dir target/perf-results/consumer-hot-paths \
  --baseline-command "cargo bench --bench consumer_hot_paths -- asteroid_movement_collision_steady --sample-count 100 --sample-size 100" \
  --candidate-command "cargo bench --bench consumer_hot_paths -- asteroid_movement_collision_steady --sample-count 100 --sample-size 100"
uv run python scripts/perf_summarize.py target/perf-results/consumer-hot-paths
```
