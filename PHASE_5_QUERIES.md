# Phase 5 — Query facade, safe mutation, and cache semantics

**Status:** ready after World contracts stabilize; implementation may overlap Phase 4
**Depends on:** [Phase 3](./PHASE_3_WORLD_LIFECYCLE.md)
**Research:** [packet 004](./.orchid/spec-research/004-queries-performance-proof/),
[packet 006](./.orchid/spec-research/006-single-crate-api-architecture/)

## Goal

Complete the source-required query behavior behind one stable `moirai::query` facade while keeping
plans, borrow machinery, archetype traversal, and cache entries private.

The design must support sparse, table, tag, and mixed queries; deterministic id selection; explicit
filters; two distinct cache semantics; and useful mutation without unsafe code.

## Requirements

| ID | Acceptance requirement |
| --- | --- |
| R501 | WHEN Query1/Query2 read mandatory sparse/table/tag/mixed combinations EACH live match SHALL appear once. |
| R502 | WHEN mutable traversal runs IT SHALL be callback-scoped and use the frozen split_at_mut strategy. |
| R503 | WHEN duplicate mutable identities are requested RESOLUTION SHALL fail before borrowing. |
| R504 | WHEN traversal needs side effects QueryEffects SHALL expose only disjoint Commands/events, never unrestricted World. |
| R505 | WHEN explicit exact-id order is requested OUTPUT SHALL follow caller order and named missing-id policy. |
| R506 | WHEN added/changed filters run THEY SHALL use `(since, captured_now]` and advance a spec-bound QueryCursor only on full success/exhaustion. |
| R507 | WHEN QueryCache is used IT SHALL cache structural membership and apply moving change filters at traversal. |
| R508 | WHEN QueryResultCache receives added/changed IT SHALL return MovingChangeWindow, never panic. |
| R509 | WHEN cache/cursor handles cross World/spec/slot lifetime THEY SHALL return ownership/stale errors. |
| R510 | WHEN user event queues clear QUERY CACHE COHERENCE SHALL remain correct. |
| R511 | WHEN identical operation traces run traversal order SHALL be deterministic within the documented scope. |
| R512 | WHEN Phase 5 exits private plans/guards/entries SHALL remain hidden and allocation/benchmark gates SHALL pass. |

## Public shape

The approved public vocabulary is:

- `QuerySpec`: structural selection and filter authoring;
- `QueryParams`: execution options with private fields;
- `QueryCursor`: owner-scoped last-observed `ChangeTick` for added/changed windows;
- `Query1` and `Query2`: intentionally limited read traversal;
- `QueryCache`: owner-scoped membership acceleration;
- `QueryResultCache`: owner-scoped materialized result reuse;
- `QueryError`: non-exhaustive configuration/ownership/borrow diagnostics.

Raw query plans, dense component arrays, archetype bitsets, cache keys/slots, revision counters, and
iterator implementation structs remain private.

`QuerySpec` and `QueryParams` use constructors/builders, never public struct literals. This leaves
room for later filters without a breaking field addition and prevents invalid combinations.

## Selection model

A resolved query may contain:

- required data components;
- required tags;
- excluded components/tags;
- optional components where the source contract needs them;
- added/changed component filters over an explicit ChangeTick window;
- a host-provided ordered list of exact entity ids;
- explicit include/exclude policy for inactive/domain markers;
- one of the supported cache policies.

Moirai never recognizes a component because its diagnostic name is `"Inactive"`. By default,
queries include every matching live entity. A host explicitly adds `without::<Inactive>()` or
wraps a standard host query spec.

Specs resolve component types/ids against one World schema before traversal. Unknown, conflicting,
or cross-world identifiers produce `QueryError`, not a panic or empty result.

`QuerySpec::added::<T>()` and `changed::<T>()` use either
`QueryParams::since(ChangeTick)` or a mutable `QueryCursor`. Each traversal captures `now` before
borrowing values and selects metadata in the exact half-open interval `(since, captured_now]`.
Mutations made by a mutable callback are therefore seen by the next window, never retroactively in
the current one.

Cursor commitment is part of traversal semantics, not iterator Drop. A lazy Query1/Query2 iterator
sets its cursor to `captured_now` only when `next()` reaches `None`; dropping after any prefix leaves
the prior cursor unchanged so unseen matches remain visible. A zero-match iterator is committed when
the caller observes exhaustion. Eager/closure traversal commits only after it visits every match and
returns success. A callback error or panic never consumes the window, although earlier component
value mutations remain non-transactional as documented below.

A QueryCursor is bound to one resolved spec fingerprint as well as its World owner. Construction
explicitly chooses `from_start` (ChangeTick zero) or `from_now`. Reusing it with another spec is a
`WrongQuery` error; this prevents one logical query from accidentally consuming another's window.
It is not Clone; `fork(&World)` explicitly creates an independent cursor at the same observation.

Value predicates remain outside persistent structural cache keys. A caller may filter values in
its closure, but value-dependent membership cannot be reused merely because the archetype revision
is unchanged.

## Read traversal

`Query1<T>` and `Query2<A, B>` provide ergonomic immutable iteration over live matches:

```rust
for (entity, position) in world.query::<Position>(spec)? {
    // ...
}

for (entity, position, velocity) in world.query2::<Position, Velocity>(spec)? {
    // ...
}
```

The actual syntax may be refined by the implementation spec, but these contracts are fixed:

- items borrow from World and cannot outlive it;
- stale/reserved entities are never yielded;
- tags are selected but not materialized as fake values;
- mixed storage produces one entity once;
- same operation trace produces the same traversal order;
- no cross-release global sort order is promised.

Canonical replay snapshots sort host-domain collections explicitly. Query iteration order is a
performance contract only where documented by a specific iterator.

Exact-id queries preserve caller id order, reject cache policies that would reorder it, and define
whether stale/missing ids are reported or skipped through an explicit parameter—not an implicit
mode. Because compact EntityId has no owner token, a coincident id from another World is caller
misuse and cannot always be distinguished; owner-scoped ComponentId, QueryCursor, and cache handles
are rejected reliably.

## Mutable traversal without unsafe code

Moirai does not promise a general `Iterator<Item = &mut T>` over arbitrary type-erased mixed
storage. Such an API is where ECS implementations commonly rely on unsafe aliasing machinery.

The 1.0 safe surface is closure-scoped:

```rust
world.for_each_mut::<Position>(spec, |entity, position| {
    position.x += 1;
})?;

world.for_each2_mut::<Position, Velocity>(spec, |entity, position, velocity| {
    position.x += velocity.x;
})?;
```

Internally, Query2 resolves two distinct component ids, sorts their storage/column indices, and
uses `split_at_mut` to obtain disjoint trait objects before safe `Any` downcasts:

- sparse + sparse splits the top-level sparse-slot slice;
- table + table splits each matching archetype's column slice;
- sparse + table destructures World storage into its disjoint sparse/archetype fields;
- tags participate only in membership and never create a fake mutable value.

The callback is invoked while those two references are live and cannot retain them after the
visit. Same-type/duplicate mutable identities fail before storage borrowing. Query1 borrows one
resolved store/column directly. A mutable traversal conservatively stamps each yielded component at
the point it grants `&mut T`, even if the callback chooses not to change the value. This is the
required safe strategy, not an implementation spike.

Before an infallible mutable traversal begins, it counts matches and preflights the required
ChangeTick increments, so clock exhaustion cannot stamp only a prefix. A fallible callback may
still leave value changes from earlier successful visits; it is explicitly non-transactional and
returns the last completed entity in its error context.

System-side effects use an explicit closure variant with
`moirai::query::QueryEffects<'_>`. World is destructured into component storage plus disjoint
allocator/command/event fields; QueryEffects exposes `commands()` and checked immediate
`send::<E>()`, but not unrestricted World or resources. Resource-dependent traversal nests inside
`World::resource_scope`. This avoids temporary entity Vec allocations without permitting
structural aliasing.

The mandatory 1.0 matrix is:

- sparse + sparse;
- table + table in one archetype;
- table + table across multiple archetypes;
- sparse + table;
- tag filters around every combination.

Only requests outside that matrix—more than two mutable data components, duplicate mutable
identities, cached value predicates, or a callback requesting unrestricted World—are rejected.
None falls through to a raw pointer escape hatch.

## Query planning

Resolution produces a private plan containing only prepared ids and traversal choices. Candidate
selection uses the cheapest known structural source, such as a matching archetype row range or the
smallest relevant sparse membership set, then verifies remaining requirements.

Plans are invalidated or refreshed by schema/topology epochs, not component value writes. Because
component registration is frozen for a built App, most plan structure is stable for its lifetime.

Plan compilation validates:

- component ids belong to this schema and have expected type/layout;
- required and excluded sets do not conflict;
- mutable identities are distinct;
- exact-id and cache order policies are compatible;
- requested cache semantics support the selected filters;
- explicit reader/revision requirements are initialized.

## Two cache types

The source has two useful but different behaviors. Moirai names and tests both.

### `QueryCache` — membership cache

This tracks which live entities structurally match a resolved spec. It updates from private
structural/lifecycle epochs or logs and is appropriate when the same membership is traversed
repeatedly while values change.

It does not cache component values or callback results. Added/changed filters are applied to the
cached structural membership using the caller's current ChangeTick window.

### `QueryResultCache` — materialized id result

This retains the exact entity-id result for a resolved structural query and reuses it while all
relevant structural revisions remain unchanged. It is useful when result construction itself is
expensive.

It is invalidated by spawn/despawn, relevant add/remove/tag changes, and any ordering-affecting
operation. Value-only writes do not invalidate a structural result.

`QueryResultCache` rejects specs containing added/changed filters with a contextual
`QueryError::MovingChangeWindow`. A materialized structural id list cannot honestly cache a
per-cursor moving temporal window. This is the deliberate adaptation of the source panic.

### Owner scope

Callers never supply raw `u64` cache keys. Cache handles contain an unforgeable private owner
identity plus slot/generation; actual entries remain World-owned. Using a handle with another
World, after removal, or after slot reuse returns `QueryError::WrongOwner` or `StaleCache`.

World and public cache handles share a private `alloc::rc::Rc` owner token. `Rc::ptr_eq` validates
ownership while the handle keeps an old token allocation alive, so address reuse cannot revive it.
The cache slot also has a generation. This needs no global counter or target atomics and matches the
single-threaded 1.0 contract.

Removing/reusing a cache slot uses checked generation. If that generation can no longer advance,
the slot is retired permanently and a later cache may allocate another slot. This local capacity
loss neither mutation-poisons World nor faults App; stale handles to the retired slot remain stale.

Cache updates are independent from public event retention/readers. Clearing user-visible
component events must not make a cache silently stale.

## Borrow and structural rules

- Immutable queries may coexist according to normal Rust borrows.
- A mutable traversal obtains exclusive split borrows for its component stores.
- Nested structural/query access is statically unavailable because callbacks do not receive World.
- Structural mutation is prohibited while a traversal is active.
- Commands may be queued only through a safe split variant and only in an Update-owned stage;
  Render operation context rejects `QueryEffects::commands()`.
- Event sends during traversal use QueryEffects and preserve declared immediate event semantics.
- A callback panic is outside recoverable runtime behavior; guards still release through RAII.

Every plan/alias error includes the requested component/query identity.

## Tests

### Functional matrix

- Query1 and Query2 for sparse/table/tag/mixed layouts;
- empty, one, many, despawned, moved-archetype, and reserved entities;
- required/excluded/optional/tag filters;
- added/changed filters with explicit since ticks and owner-scoped cursors;
- exact `(since, captured_now]` boundaries, empty/full iterator exhaustion, early drop, and failed
  closure cursor non-advancement;
- exact-id order and explicit missing-id policy;
- deterministic order for identical mutation traces;
- immutable and closure-scoped mutable traversal;
- same-type mutable rejection;
- mutation followed by archetype move/flush.
- side-effect variants that queue spawn/despawn and send a declared event while iterating.

### Cache matrix

- cold build and hot hit for both cache types;
- unrelated component changes do not invalidate;
- relevant add/remove/spawn/despawn does invalidate/update;
- value-only mutation preserves structural caches;
- cross-world and stale-handle rejection;
- cache slot reuse cannot revive an old handle;
- exhausted cache-slot generation retires only that slot and permits another slot;
- user event clear does not break cache coherence;
- unsupported spec/cache combinations return `QueryError`.
- QueryResultCache rejects added/changed while QueryCache filters them correctly.

### Model/property tests

A small reference model selects entity ids from plain maps/sets. Randomized operation traces compare
uncached, membership-cached, and result-cached output after each flush.

## Performance contract

Benchmarks land with implementation, not deferred to Phase 6:

- cold plan resolution;
- warm Query1/Query2 for sparse, table, and mixed storage;
- closure-scoped mutation;
- cache miss, incremental membership update, and hot hit;
- sparse/high-churn versus dense/steady workloads;
- steady-state allocation counts after warmup.

Benchmark setup time is excluded from traversal measurement. Both f32 and `Q16` component
workloads compile in the same build; there is no numeric feature matrix.

## Tasks

- [ ] **T501** Finalize private-field QuerySpec/QueryParams builders and resolution diagnostics.
- [ ] **T502** Implement private plans for sparse/table/tag/mixed and exact-id traversal.
- [ ] **T503** Implement Query1/Query2 immutable public iterators.
- [ ] **T504** Implement closure-scoped one- and two-component mutation with split-at-mut.
- [ ] **T505** Implement QueryEffects variants over safely split commands/events.
- [ ] **T506** Add explicit inactive/domain filters; remove magic-name behavior.
- [ ] **T507** Add ChangeTick-based added/changed filters and owner/spec-scoped QueryCursor.
- [ ] **T508** Implement owner-scoped QueryCache and QueryResultCache with frozen filter policy.
- [ ] **T509** Decouple cache coherence from user-visible event queues.
- [ ] **T510** Port/classify all source query tests, including rejected raw-key/panic expectations.
- [ ] **T511** Add reference-model randomized tests.
- [ ] **T512** Add hot/cold/cache/allocation Divan families.
- [ ] **T513** Add public API and rustdoc examples without exposing iterator internals.

## Verification

```sh
cargo test --no-default-features query
cargo test --features std query
cargo clippy --all-targets --all-features -- -D warnings
cargo bench --bench queries
cargo doc --no-deps --all-features
```

## Risks and controls

| Risk | Control |
| --- | --- |
| Mutable iteration requires hidden aliasing | closure-scoped safe guards; reject unsupported combinations |
| Cache key crosses World boundary | private owner identity plus generated slot |
| Result cache goes stale after lifecycle change | private structural epochs/log, independent of user events |
| Magic inactive policy survives migration | explicit host filter only |
| Public iterator structs freeze traversal internals | only intentional Query1/2 facade; impl types private |
| Optimizing changes observable order | deterministic trace tests and narrow order promise |

## Exit criteria

- [ ] All intended sparse/table/tag/mixed selections and mutations work without unsafe code.
- [ ] Both source cache semantics are named, owner-scoped, and reference-model verified.
- [ ] Invalid filters, aliasing, ids, owners, and cache modes return errors rather than panics.
- [ ] No raw plan/cache entry/iterator implementation is public.
- [ ] Steady-state hot query paths meet the allocation contract.
- [ ] Phase 6 has a complete classified query corpus and reproducible baselines.

## References

- [Architecture](./docs/ARCHITECTURE.md)
- [Phase 3](./PHASE_3_WORLD_LIFECYCLE.md)
- [Rust visibility and privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
