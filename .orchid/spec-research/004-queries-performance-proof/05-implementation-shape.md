# Implementation Shape

Private implementation lives under `world/query/`; `query.rs` is the stable facade.

Task order:

1. freeze QuerySpec/Params builders and error policies;
2. resolve structural ids/filters into private plans;
3. implement immutable Query1/Query2 across every storage pairing;
4. implement closure-scoped mutation through the frozen split-at-mut matrix and alias rejection;
5. implement exact-id, explicit inactive, `(since, captured_now]`, and full-exhaustion QueryCursor
   behavior;
6. implement World-owned membership and materialized-result cache entries;
7. add cross-world/stale handle tests and cache/reference randomized model;
8. add hot/cold/miss/hit/mixed/mutable/allocation Divan families;
9. add downstream examples while keeping impl types out of rustdoc.

The mandatory sparse/sparse, table/table, sparse/table, and tag-filter matrix must work. Requests
outside the 1.0 surface (more than two mutable values, duplicate identities, value-predicate
caching, or unrestricted-World callbacks) fail during plan resolution.
