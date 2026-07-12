# Design Decisions

- Public: QuerySpec, QueryParams, Query1, Query2, QueryCache, QueryResultCache, QueryError.
- Private: resolved plans, archetype candidates, borrow guards, iter implementation types, cache
  entries/slots/revisions.
- Immutable traversal borrows World; mutable traversal is closure-scoped.
- The concrete safe mutation strategy is sorted distinct indices plus `split_at_mut` and safe
  downcasts for sparse/sparse and table/table, with World field splitting for sparse/table.
- A safe split callback may expose Commands, never unrestricted World beside mutable components.
- Explicit required/excluded/tag/optional/id selection replaces magic component-name policy.
- Added/changed windows are exactly `(since, captured_now]`; lazy cursors commit only on iterator
  exhaustion and eager cursors only on complete success.
- Value predicates do not participate in persistent structural cache reuse.
- QueryCache tracks membership; QueryResultCache reuses materialized entity-id results.
- QueryCache supports added/changed as a traversal filter; QueryResultCache rejects them.
- A private shared `Rc` owner identity plus slot/generation rejects cross-world/stale cache
  handles without atomics.
- Cache generation exhaustion retires only its slot and may allocate another; it never poisons App.
- Randomized output is compared to a plain reference model after every structural flush.
- f32 and Q16 component benches run together; no numeric feature matrix.
