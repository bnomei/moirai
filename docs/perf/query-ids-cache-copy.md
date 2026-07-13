# Iterate query-ID caches without copying every hit

Status: measured and reverted; removing the copy doubled large-cache iteration latency.

## Hotspot

`query_ids` and `query_entities` copy the full cached ID slice into a fresh `Vec<EntityId>` on every cache hit before returning an iterator. A result cache therefore avoids membership recomputation but not `O(matches)` copying and allocation.

## Evidence

- `src/world/query/entities.rs:22-31` calls `.to_vec()` for result-cache hits and membership-cache hits.
- `src/query/entity.rs:35-44` stores only an owned `Vec<EntityId>` in `QueryIds`, forcing materialization before iteration.
- Typed immutable queries already use a handle-backed iterator state: `src/query/iter.rs:33-36` stores `QueryCachedSource`, and `src/query/iter.rs:153-181` reads cached entities by index without cloning the whole slice.
- The cache owns its vector in `src/world/query/result_cache.rs:12-18` and exposes a validated slice at lines 143-160.
- `tests/query_result_cache.rs:116-147` verifies entity result-cache refresh semantics, but does not assert allocation or compare cached/uncached ID iteration.
- `benches/queries.rs` benchmarks only typed query cache hits; it has no `query_ids` or `query_entities` cache case.

## Candidate and mechanism

Give `QueryIds` a source enum analogous to `Query1State`: an owned vector for uncached or temporally filtered results, and a validated cache handle plus index for cache-backed results. Store the immutable world reference already held by `QueryIds` and fetch the cached slice by handle during iteration.

Start with result caches, whose IDs are already the final result. Extend to membership caches only for specs with no moving change window; otherwise keep the current owned filtered vector. Preserve `ExactSizeIterator` by deriving the remaining length from the validated cached slice.

## Expected scope (not promised speedup)

The candidate targets allocation count, copied bytes, and cache-hit latency for large ID/entity result sets. It should matter most when callers repeatedly exhaust stable result caches. Small result sets, partial iteration, or topology changes that force a full cache refresh may see little benefit.

## Semantic and operational risks

- Cache validation and stale-handle behavior must remain eager where the public API currently returns an error at query construction.
- Cursor commitment occurs only after observed exhaustion; a new source state must preserve `Drop`, `size_hint`, and `ExactSizeIterator` behavior.
- Membership caches with added/changed filters still require temporal filtering and cannot blindly expose structural members.
- Revalidating a handle on every `next()` adds bounds and generation checks; validating once is safe only because the returned iterator immutably borrows the world and prevents cache mutation.
- An enum increases iterator size and code paths; tiny cached results may not recover the added branch cost.

## Benchmark plan

1. Add Divan cases for `query_ids` and `query_entities` using no cache, membership cache, and result cache.
2. Separate world/cache construction and refresh from the timed warm-hit region.
3. Sweep result cardinality `{0, 1, 16, 256, 16K}` and consumption `{first item, 10%, 100%}`.
4. Record wall time, allocator calls, bytes allocated/copied, and iterator size; compare owned copy, per-step handle lookup, and validate-once handle state.
5. Exercise warm stable hits separately from a topology change that forces refresh.
6. Include the expected losing case: zero- or one-result caches, where the existing contiguous copy is tiny and a source enum/handle validation may cost more.
7. Run cursor, stale-cache, owner-scope, exact-size, and randomized query-model tests.

The candidate is disproved if copies are below the allocation profiler's materiality threshold or handle-based iteration regresses the dominant small-cardinality workload.

## Result

Rejected. A handle-backed iterator removed the per-hit `Vec<EntityId>` copy, but five-run result-cache benchmarks regressed 256 IDs by 90.93% and 4,096 IDs by 103.06%; 16 IDs also regressed 5.44%. Per-step handle validation and iterator branching cost more than the contiguous copy in the measured workload.

## Decision and fallback

Fully revert the handle-backed `QueryIds` source and keep the owned contiguous vector. Reopen only if a validate-once borrowed-cache representation can preserve eager stale-handle errors and iterator semantics without per-item validation.
