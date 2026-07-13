# Replace quadratic exact-ID duplicate validation

Status: measured and reverted; the large-batch crossover did not satisfy the small-workload gate.

## Hotspot

Every exact-ID query validates duplicates with a prefix scan. For an all-unique input of length `n`, the validator performs `0 + 1 + ... + (n - 1)` equality checks before iteration, making setup quadratic in the requested ID count.

## Evidence

- `src/world/query/filter.rs:8-23` iterates with `enumerate()` and calls `ids[..index].contains(&entity)` for every eligible ID.
- `src/world/query/query1.rs:21-22`, `src/world/query/query2.rs:15-16`, and `src/world/query/entities.rs:16-17` invoke exact-ID validation at query construction, including repeated use of an already resolved plan.
- `tests/query.rs:298-320` covers duplicate rejection semantics, but only with two identical IDs.
- `benches/queries.rs` has no exact-ID workload and does not sweep requested ID count or duplicate position.
- The current `cargo bench --bench queries` release run on `rustc 1.96.0`, `aarch64-apple-darwin` did not exercise this path; therefore its timings cannot quantify this finding.

## Candidate and mechanism

Compare the current prefix scan against an order-preserving membership check backed by a temporary set. A `BTreeSet<EntityId>` is available in `alloc`, deterministic, collision-independent, and reduces expected validation work to `O(n log n)`. A sorted scratch copy followed by adjacent duplicate detection is another `O(n log n)` alternative with contiguous access, at the cost of cloning and reordering the scratch copy. Preserve the original ID vector for observable query order.

Keep owner, liveness, reservation, and policy checks unchanged. Reuse a world-owned scratch set/vector only if retained-capacity policy and nested-borrow behavior remain explicit.

## Expected scope (not promised speedup)

The candidate targets construction latency for medium and large exact-ID batches, especially all-unique inputs and duplicates near the end. Tiny exact-ID lists may remain faster with the allocation-free prefix scan. No end-to-end improvement is assumed until setup and iteration are measured separately.

## Semantic and operational risks

- Duplicate detection currently ignores foreign, stale, and reserved entities before comparing them; the replacement must preserve that exact eligibility rule.
- Query iteration order must remain the caller's order even if validation uses sorted or tree-backed scratch storage.
- A hash set would add collision-resistance and determinism decisions; prefer a tree or sorted scratch baseline first.
- Temporary storage increases allocation and peak memory unless safely reused.
- For the common one-to-four-ID case, set construction and branching can cost more than the nested scan.

## Benchmark plan

1. Add a Divan benchmark that constructs the world and ID corpus outside the timed region, then measures exact-query construction separately from full exhaustion.
2. Sweep ID counts `{0, 1, 2, 4, 16, 64, 256, 4K}` and policies `SkipUnavailable` and `ErrorOnUnavailable`.
3. Test all-unique input, duplicate at the start, middle, and end, plus mixes of stale, reserved, and foreign IDs.
4. Compare prefix scan, `BTreeSet`, and sorted scratch copy using wall time, allocations, bytes allocated, and peak retained capacity.
5. Include the expected losing case: one-to-four unique live IDs, where the current allocation-free scan can win.
6. Run the exact-ID order/error tests and randomized query model against every candidate.

The candidate is disproved as a default if production-shaped exact lists remain tiny or if allocation and code-size costs outweigh the asymptotic benefit across the supported size distribution. A small-list threshold is valid only if the crossover is stable.

## Result

Rejected after an isolated five-run control/candidate experiment. An adaptive `BTreeSet` path above 128 IDs reduced 256-ID construction by 45.18% and full exhaustion by 38.86%, but added a 6.01% regression to one-ID construction and 3.78-5.43% regressions to several small exhaustion cases. Only this validator was toggled for the isolated comparison.

## Decision and fallback

Revert the adaptive validator and retain the allocation-free prefix scan. The large-batch asymptotic win is real, but it fails the agreed no-more-than-3% losing-case gate. Reopen only with a workload or API that can select a large-batch implementation without taxing the dominant small path.
