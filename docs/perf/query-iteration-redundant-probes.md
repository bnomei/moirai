# Remove redundant storage probes from typed query iteration

Status: partially accepted; Query2 specialization wins while the losing Query1 specialization was reverted.

## Hotspot

Typed query iteration proves component membership, then performs another lookup to fetch the same component value. Sparse traversal also starts from a dense slot but discards the aligned dense index/value, reconstructs an `EntityId`, probes the sparse index for structural membership, and probes it again for the value. Query2 similarly checks its second component through the required-filter loop and fetches it again in `Query2::next`.

## Evidence

- Query1 resolution always inserts its primary component into `required_indices` (`src/world/query/spec.rs:62-67`); query2 inserts both components (`src/world/query/spec.rs:114-121`).
- `entity_matches_structural` probes every required component (`src/world/query/filter.rs:26-41`).
- Sparse iteration walks `dense_slots`, then calls `query1_match_sparse` (`src/query/iter.rs:94-108`); that function runs all filters and finally calls `store.get(entity)` (`src/world/query/query1.rs:91-104`).
- `SparseSet::get` performs another slot-to-dense lookup (`src/storage/sparse.rs:75-78`) even though the iteration began from the dense index.
- Table iteration selects archetypes known to contain the primary component (`src/world/mod.rs:129-144`) but still checks required membership and calls `get_table` (`src/world/query/query1.rs:106-117`).
- Query2 then calls `query2_second` after the inner query accepted the entity (`src/query/iter.rs:228-241`), despite the second component already being in `required_indices`.
- The current query benchmarks cover these loops, but they have no alternative driver and no hardware-counter/profile evidence attributing the aggregate time to probes.

## Candidate and mechanism

Make the traversal driver carry proof and data:

- Sparse state should iterate aligned dense `(slot, &T)` entries, excluding the primary index from generic required checks.
- Table state should resolve the primary column once per archetype and fetch by row, excluding the primary from per-entity membership checks.
- Query2 should exclude the second index from generic required checks when the same iteration step will fetch it; the fetch itself remains the membership test for optional failure.

Represent source-covered indices explicitly in the resolved plan or use specialized filter helpers, rather than silently assuming positions in `required_indices`. Keep liveness/reservation and all unrelated filters until storage invariants prove they can also be elided.

## Expected scope (not promised speedup)

The candidate targets CPU time, cache misses, and branch count in high-cardinality typed scans, especially sparse query2 intersections. Cached result iteration already skips generic structural filters when no moving window is present, so its benefit may be smaller. Highly selective filters that reject early may also reduce the value of faster accepted-item retrieval.

## Semantic and operational risks

- Dense sparse entries and archetype rows must never outlive stale entity generations or committed removals; current redundant checks may mask an invariant violation.
- Excluding a source-covered requirement must not exclude the same component from added/changed tick filters.
- Query2 must still skip entities missing the second component, as explicitly tested.
- Table column-by-row access crosses erased type boundaries and must retain checked downcasts unless separate evidence justifies anything stronger.
- Specialized iterator states increase code size and maintenance cost in a constrained crate.
- For tiny result sets, extra state and branching can lose to the existing uniform helper path.

## Benchmark plan

1. Profile `warm_query1_sparse`, `warm_query2_sparse`, and `mixed_query2_warm` first to attribute time to sparse probes, table lookup, filtering, or planning.
2. Add scaled isolation cases with setup outside timing and entity counts `{0, 1, 64, 4K, 64K}`.
3. Sweep match rates `{0%, 1%, 50%, 100%}`, required/excluded filter counts, moving tick filters, and storage pairs.
4. Compare current uniform helpers, primary-driver proof only, and primary-plus-query2 proof using wall time, instructions, branch misses, and cache misses where available.
5. Include the expected losing case: tiny or highly selective queries where uniform early rejection is cheaper than specialized driver state.
6. Differentially validate randomized queries, stale generation reuse, reserved entities, table migration, exact-ID order, query2 missing-second behavior, and added/changed windows.

The candidate is disproved if profiles do not attribute material cost to repeated probes or if specialization regresses code size/tiny-query latency beyond the accepted tradeoff.

## Result

Accepted for Query2 only. Source-covered primary/second filtering and aligned dense-value access reduced median-of-five sparse Query2 latency 37.08% at zero entities and 32.12% at one, was 0.59% faster at 64, and 2.30% slower at 4,096, inside the 3% ceiling. Applying the same specialization to ordinary Query1 regressed small exact-query exhaustion, so the default Query1 sparse/table matchers were restored exactly.

## Decision and fallback

Retain the Query2-only specialization and the generic Query1 matcher. Query2 unit/integration and exact Query1 suites pass. Reopen Query1 only with a representation that does not add branches or code-layout cost to its small/exact paths.
