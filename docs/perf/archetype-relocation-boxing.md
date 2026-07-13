# Remove per-component heap transfers from archetype relocation

Status: accepted; safe direct erased-column transfer removes per-component boxes and is measured.

## Hotspot

Adding or removing a table component relocates every retained table component for the entity. The relocation path materializes each retained value as `Box<dyn Any>` and also collects the erased rows into a temporary `Vec`.

## Evidence

- `src/storage/archetype.rs:294-371` implements relocation. Lines 325-334 clone the source signature and collect all rows into a fresh `Vec`; lines 345-357 then consume that collection.
- `src/storage/table.rs:25-29` stores every erased row value in a `Box<dyn Any>`, and `TypedTableColumn::take_row` allocates that box at lines 70-75.
- A new or replacement table value also enters the erased column through a `Box` at `src/storage/archetype.rs:66-92`.
- The allocation contract in `tests/allocation.rs` covers steady-state table queries but has no assertion for repeated archetype migration.
- The existing `archetype_move_insert_second_table_component` benchmark includes world construction, spawn, both inserts, and lookup in one timed function (`benches/world_lifecycle.rs:33-43`), so it cannot attribute relocation cost or allocation count. On commit `dfd4177b293651536413377be783b3ac0c19bc9f`, `cargo bench --bench world_lifecycle -- archetype_move_insert_second_table_component` produced a 1.666 us median, 2.498 us mean, and 55.2 us slowest sample on Apple M4 with `rustc 1.96.0`; this is a baseline for the composite case, not proof that boxing dominates it.

## Candidate and mechanism

Add an internal type-erased move operation that transfers one typed value directly from a source column to the same-typed destination column, together with its added/changed ticks. Relocation should iterate the source columns once and append directly to destination columns, without `Box<dyn Any>` per retained value and without an intermediate row `Vec`.

Keep the current boxed path as the reference implementation until differential tests prove identical row repair, drop behavior, removed-value return semantics, and tick preservation. If direct type-erased movement would require fragile unsafe code, first test a reusable erased scratch representation or a specialized move path for the common add-component case.

## Expected scope (not promised speedup)

The possible benefit is confined to structural churn involving table components: adding/removing a component, despawning table-backed entities, and migrations triggered by bundles or deferred commands. It should not improve steady-state sparse access or read-only table queries. Expected allocation reduction grows with the number of retained table columns per migrated entity.

## Semantic and operational risks

- Type erasure currently centralizes ownership transfer through `Any`; bypassing it can introduce type confusion or double-drop bugs.
- Swap-removal must still repair the moved entity's location exactly.
- Added and changed ticks must remain byte-for-byte equivalent.
- Removal must still return ownership of the requested typed value.
- A more specialized vtable can increase binary size and compile time through per-component monomorphization.
- For rare structural changes or one-column archetypes, added dispatch machinery can cost more than the removed allocations.

## Benchmark plan

1. Add a Divan benchmark that constructs source and destination archetypes before timing, then oscillates pre-reserved entities between them.
2. Sweep retained table-column counts `{1, 2, 4, 8, 16}`, component payload sizes `{4 B, 32 B, 256 B}`, and batch sizes `{1, 32, 1024}`.
3. Count allocator calls and allocated bytes around only the migration loop with the serialized counting allocator used by `tests/allocation.rs`.
4. Keep the current boxed implementation as the rival baseline and compare medians plus distribution tails on the same host/toolchain.
5. Include the expected losing case: a single cold migration of one tiny component, where extra type-erased dispatch or scratch setup may outweigh allocation savings.
6. Run table lifecycle, tick-preservation, drop-count, bundle, deferred-command, and full release tests after each implementation.

The experiment is disproved if direct movement does not reduce allocator calls in the timed migration loop, or if the component benchmark shows no reliable improvement beyond host noise for representative churn while increasing binary size or semantic risk materially.

## Result

Accepted. Safe erased-column transfer moves retained components directly between tables, eliminating their individual boxes and the temporary row vector; only a removed value returned to the caller remains boxed. Median-of-five migration latency improved 43.27% for one column/one entity and 47.88-48.65% for four-column cases from one through 256 entities.

## Decision and fallback

Retain the safe direct-transfer implementation. Table lifecycle, bundle, component lifecycle, randomized query-model, and release allocation suites pass. No unsafe code was introduced; the boxed row remains the documented fallback if future erased-storage changes invalidate the ownership contract.
