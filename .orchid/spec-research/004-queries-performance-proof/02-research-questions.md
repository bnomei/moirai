# Research Questions

## Resolved

- Immutable Query1/Query2 can remain intentional public facade types.
- General mutable iterators are not promised under `#![forbid(unsafe_code)]`; mutation is
  closure-scoped.
- Same-component mutable aliasing is rejected before borrowing.
- QuerySpec/QueryParams use private fields/builders and explicit inactive filters.
- Added/changed filters use World ChangeTick plus an explicit since value or Rc-owner-scoped
  QueryCursor.
- Membership and materialized-result caches are different public concepts.
- Cache handles are owner-scoped and opaque; callers never choose raw numeric keys.
- Cache coherence uses private structural epochs/logs, not user event-reader retention.
- Exact replay snapshots sort host collections; query iteration only promises determinism for the
  same operation trace.
- Query2 mutation uses sorted indices plus `split_at_mut`: sparse slots split at the top level,
  table columns split inside each archetype, and mixed storage destructures disjoint World fields.
- QueryCache applies added/changed over cached structural membership; QueryResultCache rejects that
  moving temporal window with QueryError instead of the source panic.

## Deferred to implementation spec

- Exact-id missing/stale policy names and iteration tie-break details.
