# Current State

- pd-asteroids exposes query plans/cache keys and uses two cache behaviors worth retaining.
- Source filtering includes magic handling tied to an `Inactive` name; Moirai makes that filter
  explicit and host-owned.
- Invalid source cache/filter combinations can panic; Moirai returns `QueryError`.
- Arbitrary type-erased mutable multi-component iteration is the principal pressure toward unsafe
  ECS code. Closure-scoped callbacks and safe dynamic guards bound the problem.
- User-visible lifecycle events may be cleared/retained independently from internal cache
  coherence, so caches cannot consume those public reader streams as their sole truth.
- `PHASE_5_QUERIES.md` defines functional/cache/model/benchmark matrices.

