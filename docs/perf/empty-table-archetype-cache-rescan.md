# Distinguish empty table-cache hits from misses

Status: accepted; explicit initialization state removes repeated empty-cache scans.

## Hotspot

The table-archetype cache uses an empty `Vec<usize>` for both "not computed" and "computed with no matching archetypes." A query for a registered table component with no current matching archetype therefore rescans every archetype on every call.

## Evidence

- `src/world/mod.rs:129-144` grows the cache with empty vectors and recomputes whenever `slot.is_empty()`.
- `src/storage/archetype.rs:217-224` implements recomputation as a scan of all signatures followed by collection into a new vector.
- `src/world/query/query1.rs:27-46` calls `ensure_table_archetypes` for every table-primary query before constructing the iterator; query2 does the same at `src/world/query/query2.rs:21-40`.
- A legitimate computed result is empty when the component has no instances, while unrelated table components can still create many archetype signatures.
- `benches/queries.rs:33-50` builds only a mixed world where the table component is populated; no benchmark queries an absent table component in a world with many unrelated archetypes.

## Candidate and mechanism

Represent initialization separately from the result. Compare `Vec<Option<Vec<usize>>>` with a parallel initialized bitset/vector while keeping the inner `Vec<usize>` unchanged. An initialized empty entry then returns immediately until a relevant topology invalidation.

Combine this representation fix with any later invalidation-granularity work only after measuring it independently; the empty-hit ambiguity exists even with the current global revision.

## Expected scope (not promised speedup)

The candidate targets repeated empty table queries in worlds with many archetypes, such as optional systems whose component population is currently zero. Populated table queries are unaffected after their first fill. Worlds with few archetypes or rare empty queries will see negligible benefit.

## Semantic and operational risks

- Initialization state must reset on every topology change that can create or remove a matching archetype.
- `Option<Vec<_>>` increases outer representation size; a parallel bitset adds synchronization invariants.
- Global cache clears currently make reset simple; future narrow invalidation must not leave an initialized empty result stale.
- For a world with zero or one archetype, checking an extra initialization flag may cost as much as rescanning.
- This does not address the broader cost of clearing all component entries after unrelated topology changes.

## Benchmark plan

1. Build worlds with `{0, 1, 16, 256, 4K}` table archetypes created by combinations of unrelated table components while leaving the queried table component absent.
2. Separate world/archetype construction from repeated query timing and measure cold plus warm calls.
3. Compare the empty-vector sentinel, `Option<Vec<usize>>`, and a compact initialized flag using wall time, allocations, outer-cache bytes, and signatures inspected.
4. Add populated controls with one and many matching archetypes.
5. Include the expected losing case: a single cold empty query in a zero-archetype world, where explicit initialization state adds code/data without avoiding repeated work.
6. Validate insertion/removal, archetype migration, despawn, deferred flush, and empty-to-populated-to-empty transitions.

The candidate is disproved if empty table queries are absent from representative workloads or if archetype counts remain so small that repeated scans fall below the noise floor.

## Result

Accepted. The cache now stores `Option<Vec<usize>>`, so an initialized empty result is distinct from a miss. Median-of-five warm empty-query latency improved 4.76% with four archetypes and 16.50% with sixteen; the zero-archetype control improved 1.98% and stayed within the neutral/noise band.

## Decision and fallback

Retain the explicit initialization sentinel. Query, table lifecycle, and archetype-migration tests pass; revert to the compact ambiguous sentinel only if a constrained target demonstrates material outer-cache memory pressure.
