# Compact sparse membership indices

Status: measured and reverted; the 75% slot reduction failed the lookup-latency gate.

## Hotspot

Every populated sparse component or tag store maintains a slot-indexed `Vec<Option<usize>>`. Its length grows to the highest entity slot inserted into that store, even when only a small fraction of those slots hold the component.

## Evidence

- `src/storage/sparse.rs:8-14` defines the sparse index alongside four dense vectors.
- `src/storage/sparse.rs:58-72` grows and writes the slot-indexed map on insertion; `ensure_sparse` at lines 126-129 resizes through `slot + 1`.
- Lookups and removal read the representation directly at lines 75-116, so its footprint and locality affect the core sparse path.
- Entity slots are already bounded to `u32` (`src/entity/id.rs:20-33`), and dense slots are stored as `u32` (`src/storage/sparse.rs:10`).
- A local release-host type-size probe with `rustc 1.96.0` on `aarch64-apple-darwin` measured `size_of::<Option<usize>>() == 16`, `size_of::<usize>() == 8`, and `size_of::<u32>() == 4`. Therefore the current sparse index consumes 16 bytes per covered entity slot on this target before vector capacity overhead; a checked `u32` sentinel or `Option<NonZeroU32>` candidate would consume 4 bytes per slot. This is a representation measurement, not an RSS or latency result.
- `benches/storage.rs` tests only 128 densely allocated entities and combines construction, insertion, and lookup. It does not vary high-water slot, occupancy, number of sparse stores, retained capacity, or peak memory.

## Candidate and mechanism

Compare the current `Vec<Option<usize>>` with a safe four-byte encoding:

- `Vec<Option<NonZeroU32>>`, storing `dense_index + 1`; or
- `Vec<u32>` with `u32::MAX` as the vacant sentinel.

Convert to `usize` only at dense-vector indexing boundaries, with checked construction and an explicit maximum dense length. Preserve the current slot-indexed O(1) access and swap-remove repair behavior.

## Expected scope (not promised speedup)

The primary target is memory footprint and cache density for constrained worlds with many sparse/tag stores or a large entity-slot high-water mark. On the measured 64-bit target, the theoretical sparse-index element width falls from 16 bytes to 4 bytes. Real RSS savings depend on vector capacity, populated slot ranges, allocator behavior, number of stores, and the rest of each world. Lookup throughput might improve through locality, but no runtime improvement is assumed.

## Semantic and operational risks

- A `u32` representation must reserve one value or encode `index + 1`, reducing the maximum representable dense index.
- Every conversion boundary must reject overflow rather than wrap.
- A sentinel representation is easier to misuse than `Option`; `Option<NonZeroU32>` preserves the type-level vacant/present distinction.
- On 32-bit targets, the current `Option<usize>` layout may differ, so the memory ratio must be measured per supported target.
- For tiny worlds, conversion instructions and code size may outweigh any cache benefit, while absolute memory savings remain negligible.
- This does not solve retained high-water capacity after mass despawn; shrink policy is a separate lifecycle decision.

## Benchmark plan

1. Add a storage benchmark that separates setup from timed lookup/mutation and accepts entity high-water mark, occupancy, and store count as independent arguments.
2. Sweep high-water slots `{128, 4K, 64K}`, occupancy `{1%, 10%, 50%, 100%}`, and sparse/tag store counts `{1, 8, 32}`.
3. Record sparse-index capacity bytes, total allocator bytes, peak RSS where available, and lookup/insert/remove throughput.
4. Compare current `Option<usize>`, `Option<NonZeroU32>`, and sentinel `u32` on every supported architecture; keep identical entity sequences and preallocation.
5. Include the expected losing case: one tiny, fully dense store whose index remains hot in cache and where checked decode overhead can dominate.
6. Validate swap-removal, stale entity handling, generation reuse, maximum-index rejection, tags, and all release tests.

The candidate is disproved as a default if representative constrained workloads show negligible peak-memory reduction, if decode costs cause a material hot-lookup regression, or if the reduced index domain conflicts with supported world limits.

## Result

Rejected after implementation and five-run measurement. `Option<NonZeroU32>` reduced the sparse reverse-index slot from 16 bytes to 4 bytes on this 64-bit host, but low-occupancy lookup regressed 6.94% at a 256-slot high-water mark and 8.02% at 4,096. The setup-inclusive control was +1.89%.

## Decision and fallback

Fully restore `Option<usize>`. The memory reduction is concrete but violates the agreed latency ceiling. Reopen only for a constrained target/workload where measured resident-memory pressure outweighs the lookup loss, with the same boundary and overflow tests.
