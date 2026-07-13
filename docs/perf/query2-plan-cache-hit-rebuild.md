# Avoid rebuilding `query2` plans on cache hits

Status: accepted; warm hits prepare identity metadata without rebuilding a discarded plan.

## Hotspot

`resolve_query2_plan` fully resolves and materializes a new `ResolvedPlan` before consulting the plan cache. Even a cache hit clones selector vectors and constructs traversal metadata, then discards that temporary plan.

## Evidence

- `src/world/query/plan_cache.rs:57-70` calls `resolve_query2` before `BTreeMap::entry`; the cache lookup occurs only after a complete plan exists.
- `src/world/query/spec.rs:104-164` resolves both component types, fills and normalizes selectors, constructs traversal, fingerprints it, and clones four scratch vectors into `ResolvedPlan`.
- By contrast, query1 computes a fingerprint first and returns the cached `Rc` before `resolve_query1` materializes the plan (`src/world/query/plan_cache.rs:38-54`).
- Query2's second component index/storage kind are returned separately rather than cached with the plan (`src/world/query/plan_cache.rs:60-70`).
- `benches/queries.rs:90-110` and `159-171` exercise repeated query2 calls, but each benchmark also constructs/populates a world and iterates results inside one timed function; there is no plan-only counter or allocation measurement.
- One current release run reported `warm_query2_sparse` median 358.2 us and `mixed_query2_warm` median 170.4 us for their respective aggregate functions. These unlike workloads do not isolate or compare planning cost.

## Candidate and mechanism

Add a query2 preparation path that computes the fingerprint and second-component metadata using reusable scratch without cloning plan-owned vectors. On a hit, return the cached `Rc<ResolvedPlan>` plus the prepared `second_index` and `second_is_table`. Materialize owned vectors only on a miss.

Alternatively cache a small query2 resolution record keyed by full query identity, but avoid duplicating structurally identical plans unless measurement supports it. Preserve the simple query1/entity cache behavior as the reference design.

## Expected scope (not promised speedup)

The candidate targets warm repeated query2 construction, particularly systems that issue many short or empty-result queries where planning is a larger fraction of total work. Long scans over many matching entities may hide the saved allocation and selector resolution.

## Semantic and operational risks

- Query identity must continue to distinguish primary component, filters, exact-ID order, and exact-ID policy.
- `second_index` and storage kind must correspond to the same world registry as the cached plan.
- A separate query2 cache can increase memory and key complexity; a prepare-on-hit path is preferable unless reuse data says otherwise.
- Cold misses may repeat component resolution once to fingerprint and once to materialize, as query1 currently does.
- Exact-ID preparation still clones/hashes IDs elsewhere; this candidate should be measured separately from exact-ID-specific changes.

## Benchmark plan

1. Add plan-only Divan helpers inside the crate or a test-only public measurement seam, with world/spec construction outside the timed region.
2. Sweep empty, one-match, and `{256, 16K}`-match query2 workloads so planning share is visible.
3. Vary selector counts `{0, 2, 8, 32}`, cache state `{cold, warm}`, and storage pairs `{sparse/sparse, sparse/table, table/table}`.
4. Compare current full resolution, scratch-only fingerprint preparation, and any query2-specific cache using wall time, allocation count, bytes allocated, and cache entries.
5. Include the expected losing case: one-shot cold queries with many selectors, where a two-pass peek-then-build path repeats registry work.
6. Validate query identity, component ordering, tag filters, exact-ID policies, and randomized query2 results.

The candidate is disproved if planning allocations are optimized away or immaterial in profiles, or if cold-query regression outweighs warm savings at the observed reuse rate.

## Result

Accepted. Query2 now prepares the fingerprint and second-component metadata first, returns the cached plan on a hit, and performs full resolution only on a miss. Median-of-five warm empty-plan latency improved 35.20% with zero selectors, 34.48% with eight, and 20.51% with thirty-two.

## Decision and fallback

Retain prepare-before-hit with full `resolve_query2` as the cold-miss path. Query identity, ordering, exact-policy, and randomized Query2 tests pass; revert if a production workload dominated by cold one-shot specs demonstrates an end-to-end loss.
