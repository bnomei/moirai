# Narrow query-cache invalidation to affected topology

Status: accepted; dependency snapshots skip unrelated rebuilds with an O(1) stable fast path.

## Hotspot

One world-wide topology revision invalidates every membership cache, result cache, and table-archetype cache. A spawn with no components or an insertion/removal of an unrelated component therefore causes the next cached query to rebuild its full entity list.

## Evidence

- `src/world/mod.rs:67-73` stores one `query_topology_revision` shared by all query-side caches.
- `src/world/mod.rs:123-127` increments that revision and clears the complete table-archetype cache.
- `src/world/query/cache.rs:103-118` rebuilds an entire membership vector whenever its saved revision differs.
- `src/world/query/result_cache.rs:143-160` applies the same global-revision test to result caches.
- `src/world/mod.rs:247-263` bumps topology for every spawn and despawn.
- `src/world/events.rs:168-220` bumps the same revision for every newly added or removed component through lifecycle emission.
- Tests such as `tests/query_cache.rs:404-426` and `tests/query_result_cache.rs:174-196` verify that caches refresh after relevant mutation, but no test or benchmark distinguishes relevant from unrelated churn.
- `benches/queries.rs:113-135` measures only a stable warm membership-cache hit; it does not mutate topology between hits.

## Candidate and mechanism

Track dependency-aware topology versions or dirty sets. A resolved plan already lists required, excluded, and tag component indices; cache refresh can compare only revisions that may change that plan's membership, plus an entity-liveness revision for entity-only traversal.

Table traversal needs special treatment: moving an entity between table archetypes can create a new source archetype even when the changed component is not a filter. Either increment table-archetype revisions for every component in the old and new signatures, or maintain table source lists incrementally. Start with version vectors and explicit dependencies before considering per-cache dirty fan-out.

## Expected scope (not promised speedup)

The candidate targets worlds with many independent cached queries and localized structural churn. It removes full-list rebuilds when a mutation cannot affect a cache's membership. Workloads dominated by spawn/despawn, broad entity-only queries, or mutations that affect most plans may gain nothing and pay extra bookkeeping.

## Semantic and operational risks

- Exclusion filters are dependencies too: adding a `without` component can remove an entity from results.
- Spawn/despawn affects entity-only queries and may affect typed queries during bundled insertion; revision updates must follow committed visibility and rollback semantics.
- Table archetype movement means naive per-changed-component counters are insufficient.
- Deferred commands, reserved entities, lifecycle-event failure, bundle rollback, and generation reuse must not expose stale membership.
- Version vectors consume memory proportional to registered components; cache-local dependency snapshots consume memory proportional to filters/cache count.
- Extra mutation-side work can regress churn-heavy workloads where nearly all caches are invalidated anyway.

## Benchmark plan

1. Build worlds with `{1, 8, 64}` live caches over disjoint and overlapping component sets, separating setup from timed frames.
2. Sweep entity count `{256, 16K}`, cache selectivity, and mutations per frame `{1, 16, 1K}`.
3. Compare no mutation, empty spawn/despawn, relevant component insertion/removal, unrelated sparse/tag mutation, and unrelated table-archetype movement.
4. Measure frame latency distributions, number of cache rebuilds, entities rescanned, allocator bytes, mutation cost, and retained metadata.
5. Include the expected losing case: bulk spawn/despawn or bundle churn that legitimately affects nearly every cache, where dependency bookkeeping adds work without avoiding refreshes.
6. Differentially compare cached and uncached results after every mutation, including deferred flush, rollback, and generation reuse.

The candidate is disproved if representative structural churn usually overlaps all cached queries, or if bookkeeping costs more than the rescans it avoids at the observed cache count and entity cardinality.

## Result

Accepted after one retune. Cache snapshots retain the global topology revision as an O(1) stable fast path and consult entity/component dependency revisions only after topology changes; an unrelated change advances the observed global revision without rebuilding. Median-of-five unrelated-churn latency improved 51.35% at 256 entities and 49.07% at 4,096. Stable hits improved 0.96%/0.40%; relevant invalidation improved 3.04% at 256 and was +0.04% at 4,096.

## Decision and fallback

Retain dependency-aware invalidation with the global scalar fast path. Membership/result cache integration suites, deferred mutation paths, and randomized query models pass. Fall back to global-only invalidation if future mutation types cannot conservatively identify their component/entity dependencies.
