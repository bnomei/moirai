# Typed cached iteration revalidates its handle per entity

Status: measured and reverted; large scans improved but small-query regressions failed the gate.

Hotspot:

`Query1State::Cached` resolves and validates the membership/result-cache handle on every `Iterator::next` call. Owner, generation, slot, and fingerprint checks therefore repeat for every yielded entity.

Evidence:

An immutable query iterator prevents cache mutation while it borrows the world, so resolving the cache to an entity slice during iterator construction is semantically sufficient. The repeated validation is measurable work rather than a required concurrency guard.

Candidate and mechanism:

Validate once during `Query1` construction and store the borrowed entity slice in the cached iterator state. Query2 inherits the same path through its inner Query1 iterator.

Expected scope (not promised speedup):

The candidate targets full exhaustion of large stable membership/result caches. Empty, one-result, and first-item-only queries are expected losing cases because they cannot amortize construction and code-layout changes.

Semantic and operational risks:

Stale handles must still fail before iteration, cursor commit behavior must remain unchanged, and no cache mutation may become possible while the slice is borrowed.

Benchmark plan:

`benches/queries.rs` measures typed membership-cache first-item access and membership/result-cache full exhaustion at `{0, 1, 16, 256, 4_096}` results. Five prebuilt baseline/candidate pairs use the same alternating, executable-identified, load-capped protocol as the event-reader experiment.

Result:

Rejected. Full exhaustion improved 12.39-15.51% for membership caches and 15.31-18.86% for result caches at 256-4,096 entities. However empty/one-result full exhaustion regressed 7.09-14.14%, and first-item controls regressed up to 14.53% (6.79% for non-empty caches). These losing cases exceed the 3% ceiling.

Decision and fallback:

Restore per-step handle resolution and keep the new benchmarks. Reopen only with an adaptive representation that preserves the empty, one-result, and first-item paths without adding a cardinality branch or larger iterator state that reproduces the measured regressions.
