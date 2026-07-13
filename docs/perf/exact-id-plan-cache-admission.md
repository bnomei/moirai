# Bound exact-ID plan-cache admission

Status: measured and reverted; exclusion bounded retention but materially regressed repeated exact queries.

## Hotspot

Resolved query plans are stored for the lifetime of a `World` in an unbounded `BTreeMap`. Exact-ID content participates in the fingerprint and is cloned into the cached plan, so workloads that issue many distinct dynamic ID batches retain every batch even after the caller drops its `QuerySpec`.

## Evidence

- `src/world/mod.rs:72-74` stores `resolved_plan_cache: BTreeMap<u64, Rc<ResolvedPlan>>`; no capacity, eviction, or clear policy is present.
- `src/world/query/plan_cache.rs:27-35`, `46-54`, and `65-70` insert every cache miss into that map.
- `src/world/query/spec.rs:69-70`, `126-127`, and `183-184` clone `QuerySpec::exact_ids` into `TraversalSource::Exact`.
- `src/world/query/plan.rs:8-13` makes that owned `Vec<EntityId>` part of every exact resolved plan.
- `src/world/query/spec.rs:443-449` hashes the full ordered ID list, so changing batch membership or order normally creates another key.
- Membership and result caches explicitly reject exact-ID specs (`src/world/query/cache.rs:36-45`, `src/world/query/result_cache.rs:54-56`), so this retention is not required to support those public caches.
- `benches/queries.rs` contains no stream of changing exact-ID specs and records neither plan-cache cardinality nor RSS.

## Candidate and mechanism

Do not admit exact-ID plans to the long-lived structural plan cache by default. Resolve their structural selectors into an ephemeral `Rc<ResolvedPlan>` for the iterator, while continuing to cache plans whose identity depends only on component selectors and policies.

Compare that simple exclusion with a bounded admission design only if telemetry shows repeated reuse of identical large exact-ID batches. Any bounded design needs full-key verification rather than trusting a 64-bit fingerprint alone, plus a deterministic eviction policy suitable for `no_std + alloc`.

## Expected scope (not promised speedup)

The primary target is bounded retained memory in worlds that build frame-varying exact-ID batches. Avoiding tree growth and long-lived ID-vector retention may also reduce insertion work, but repeated identical exact queries could lose plan reuse. Static structural-query performance should remain unchanged.

## Semantic and operational risks

- Exact queries still need an owned ID list for iterator lifetime and must preserve caller order and exact-ID policy.
- Excluding exact plans repeats selector resolution for identical specs; this can regress stable repeated batches.
- A bounded cache adds metadata, eviction work, and reproducibility questions to a constrained `no_std` crate.
- Cache-key collision handling is a correctness concern independent of capacity; an admission redesign must not widen that risk.
- Measuring only live vector lengths misses allocator fragmentation and `Rc` retention by active iterators.

## Benchmark plan

1. Add a component benchmark that issues `{1, 64, 4K, 64K}` distinct exact-ID specs against one world, then drops every spec and iterator.
2. Vary batch length `{1, 16, 256, 4K}`, overlap `{0%, 50%, 100%}`, and repeated-spec rate `{0%, 10%, 100%}`.
3. Record plan-cache entries, bytes retained by exact-ID vectors, allocator bytes, peak RSS, and query-construction latency.
4. Compare unbounded admission, exact-plan exclusion, and one explicitly bounded policy; keep iteration and error output identical.
5. Include the expected losing case: one identical exact-ID batch reused for every query, where unbounded admission has maximal hit rate and bounded/excluded plans repeat work.
6. After a burst, measure steady-state memory recovery rather than only peak allocation.

The candidate is disproved if exact-spec diversity is demonstrably bounded and stable, retained bytes are negligible, or exclusion materially regresses representative repeated batches without recovering meaningful memory.

## Result

Rejected. Exact-plan exclusion was implemented and correctness-tested, but five-run repeated exact-query cases regressed 15-56% at 1-64 IDs while the 256-ID case improved about 33%. The experiment did not produce a representative RSS or retained-byte benefit large enough to justify that dominant repeated-query cost.

## Decision and fallback

Restore exact plans to the resolved-plan cache. Reopen bounded admission only with real high-diversity exact-query telemetry plus retained-memory/RSS evidence; any design must preserve the current repeated-spec hit path within the 3% gate.
