# Avoid linear event-type lookup on every send

Priority: medium

Confidence: high; a 16-entry prefix/tree crossover is measured
Target metric: typed event-send latency as registered event count grows

Hotspot:

`World::send<E>` resolves `TypeId::of::<E>()` through a linear scan of the event registry before accessing the already-dense event channel. Thus each message pays `O(registered_event_types)` lookup work.

Evidence:

- [`src/world/events.rs:7`](../../src/world/events.rs#L7) calls `registry.id_of::<E>` for every send.
- [`src/event/registry.rs:197`](../../src/event/registry.rs#L197) delegates to `id_of_type_id`.
- [`src/event/registry.rs:201`](../../src/event/registry.rs#L201) uses `entries.iter().position(...)` for the lookup.
- The same scan is used for reader creation, but send frequency makes it the primary candidate.
- Existing event allocation tests register one event type and no benchmark sweeps registry size.

Candidate and mechanism:

Add a registration-time `BTreeMap<TypeId, u32>` for ordinary typed events while retaining the dense `entries` vector as the canonical metadata store. `alloc::collections::BTreeMap` preserves `no_std + alloc` compatibility and deterministic behavior. Compare it with the status quo rather than assuming a tree wins; for small registries, a contiguous vector scan is credible. A small-vector-plus-tree threshold is only justified by a measured crossover.

Expected scope (not promised speedup):

The candidate changes lookup from linear to logarithmic and should stabilize send cost for worlds with many registered event and lifecycle types. It adds a second registry structure, registration allocations, pointer chasing, and binary-size cost. Small worlds may regress.

Semantic and operational risks:

- Lifecycle event entries intentionally share payload `TypeId`s and are excluded by `lifecycle_component_index`; the typed map must preserve that distinction.
- Duplicate idempotent registration and same-name/different-type errors must remain exact.
- A second index can become inconsistent unless all registration paths update it atomically.
- Adding a hash map instead would require an explicit hasher/security/determinism decision; it is not the default candidate.

Benchmark plan:

Add Divan cases with `[1, 4, 16, 64, 256]` registered ordinary/lifecycle event entries and hot sends targeting first, middle, and last ordinary entries. Compare vector, `BTreeMap`, and (only if justified) a threshold hybrid. Separate registration time from send time; record send throughput, instruction counts if available, allocation calls during registration, registry bytes, and binary-size delta. The disproof case is one to eight registered events, where contiguous scanning may remain faster and smaller.

Result:

Accepted adaptively. The first 16 ordinary event types stay in the contiguous metadata vector; later ordinary types are additionally indexed in a `BTreeMap`, while lifecycle entries remain excluded. Median-of-five hot lookup/send improved 40-76% for middle/last targets at 64-256 entries and stayed within 1.8% for all 16-entry positions. Registration/build improved 16-18% at 16/256 entries and stayed within 1% at 1/64.

Decision and fallback:

Retain the 16-entry prefix plus deterministic tree fallback. Duplicate registration, lifecycle/ordinary separation, fallback idempotence, ownership, and allocation tests pass. Keep the vector-only path as the fallback if a shipping workload never exceeds the prefix.
