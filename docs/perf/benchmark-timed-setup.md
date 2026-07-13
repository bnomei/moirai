# Timed setup obscures the operation-level benchmarks

Priority: high
Confidence: high

Hotspot:

The Divan query, schedule, storage, and world-lifecycle numbers in `docs/perf.md` are used as operation-level regression baselines, but their timed bodies also construct the state and, for the query cases, perform the stated warmup.

Evidence:

- `benches/queries.rs:53-65` creates a 64-entity world inside `cold_query1_sparse_resolve` and then runs the query 32 times. Only the first resolution can be cold.
- `benches/queries.rs:69-88`, `91-110`, `113-135`, and `138-157` create a 256-entity world inside each "warm" benchmark. The 8 warmup traversals and the 128 or 64 intended traversals are all in the same timed function.
- `benches/schedule.rs:16-20` builds the app inside `app_update`; `benches/storage.rs:15-32` and `benches/world_lifecycle.rs:21-61` likewise include world construction in cases named after insert, lookup, migration, and flush operations.
- `docs/perf.md:29` describes the query paths as warm, and `docs/perf.md:68` says warmup/reservation happens before measurement. The benchmark source does not establish that timing boundary.
- Divan 0.1.21's `Bencher::with_inputs` contract says input generation does not affect benchmark timing; `bench_local_refs` supports fresh mutable input per iteration. That is a direct mechanism for expressing the missing boundary.

Candidate and mechanism:

Keep setup-inclusive end-to-end controls, but add separately named operation benchmarks that accept `divan::Bencher`, generate a fresh world/app with `with_inputs`, and time only the operation through `bench_local_refs` or `bench_local_values`. Resolve and warm caches before the timed closure for warm cases. Give cold resolution one fresh unresolved world/spec per timed operation rather than aggregating one cold and 31 warm traversals.

Expected scope (not promised speedup):

This changes attribution, not library runtime. It should reveal whether an observed delta belongs to construction, cache resolution, traversal, mutation, or flush. The likely impact is largest for the single-operation schedule and world-lifecycle cases, where setup can be a substantial share of the current body.

Semantic and operational risks:

Moving setup outside timing can hide allocator and cache costs that users pay in genuinely cold workflows. Fresh input generation can also perturb caches between samples. Existing baseline names and medians become incompatible if their meaning changes silently.

Benchmark plan:

1. Preserve every current case under an explicit `*_including_setup` or `*_end_to_end` name.
2. Add isolated cold and warm cases at 64, 256, and at least one larger entity count. Use fresh input per iteration for mutation and lifecycle operations.
3. For queries, measure exactly one unresolved lookup in the cold case and a separately pre-resolved traversal in the warm case. Report entities visited with a Divan item counter.
4. Compare setup-inclusive and operation-only distributions on the same commit, then run the release allocation contract and output/count assertions.
5. Treat a candidate as disproved if operation-only time is timer-noise dominated or if the end-to-end control moves in the opposite direction.

Losing/crossover case:

For one-shot worlds, tiny entity counts, or cold-start-sensitive hosts, the setup-inclusive benchmark is the more representative metric. Operation-only results must not replace it; the two answer different questions.

Result:

Accepted as a measurement-only correction. The original query, schedule, storage, and world-lifecycle cases are retained under explicit `*_including_setup` names. New operation-only cases build and warm inputs outside the timed closure and cover cold/warm cache behavior, scaled entity counts, sparse occupancy, migration, and composite-condition depth. `cargo bench --no-run` passes for all benchmark targets.

Decision and fallback:

Use operation-only cases to attribute implementation changes and setup-inclusive controls to detect end-to-end reversals. No library speedup is claimed for this harness change itself, and historical medians keep their original setup-inclusive meaning.
