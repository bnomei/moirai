# Replace quadratic command-preflight membership checks

Priority: high

Confidence: high; asymptotic and small-batch behavior are measured
Target metric: deferred-command flush latency and peak temporary bytes as live entity and batch sizes grow

Hotspot:

`CommandQueue::preflight` constructs a `LiveSet` containing every live or reserved entity, then validates every queued operation against that set. `LiveSet::contains` is a linear `Vec::contains`; `insert` repeats that scan, and `remove` performs another linear search.

Evidence:

- [`src/command/queue.rs:204`](../../src/command/queue.rs#L204) builds the live snapshot for every non-empty flush and walks every command.
- [`src/command/queue.rs:302`](../../src/command/queue.rs#L302) stores the snapshot as `Vec<EntityId>`; lines 313-330 implement linear membership, insertion, and removal.
- [`src/world/mod.rs:867`](../../src/world/mod.rs#L867) scans every allocator slot and reserves enough temporary vector capacity for all live plus reserved entities.
- The existing [`benches/world_lifecycle.rs:47`](../../benches/world_lifecycle.rs#L47) case covers only one deferred spawn plus one insert in a newly built empty world. On this checkout (`dfd4177b`, Rust 1.96.0, Darwin arm64), `cargo bench --bench world_lifecycle -- deferred_command_flush` reported a 1.291 us median over 100 samples x 100 iterations, but it does not exercise the scaling mechanism.
- No CPU or allocation profile has yet attributed end-to-end workload time to this path.

Candidate and mechanism:

Represent preflight liveness by allocator slot/generation state plus a compact batch-local overlay for transitions made by earlier commands. One credible safe-Rust design is a generation-stamped dense state vector indexed by `EntityId::slot`, initialized lazily for only slots touched by the batch. It changes membership from repeated `O(live_entities)` scans to `O(1)` indexed checks and avoids copying every live entity before validation. A simpler `BTreeSet<EntityId>` or sorted-vector alternative should remain in the comparison because it may use less scratch memory for small sparse batches.

Expected scope (not promised speedup):

The candidate removes an `O(command_count * live_entity_count)` component and the unconditional full live-world snapshot. It should matter most for large worlds with structural command bursts. The current vector can still win for tiny worlds and one- or two-command batches because it is contiguous and has no overlay bookkeeping.

Semantic and operational risks:

- The overlay must preserve exact in-batch ordering: spawn then insert is valid, despawn then insert is invalid, and stale generations must remain rejected.
- Owner, generation, reserved, live, free, and retired states must not be conflated.
- A dense scratch vector can retain memory proportional to the highest slot after a burst; define a retention cap or shrink boundary.
- Adding a tree or hash collection increases code size and, for hashing, raises determinism and adversarial-key questions in a `no_std` crate.

Benchmark plan:

Add a Divan component benchmark that builds the world outside the timed region and times queue-plus-flush separately for live entity counts `[0, 64, 1_024, 16_384]`, batch sizes `[1, 8, 128, 2_048]`, and mixes of spawn/insert/remove/despawn. Compare the current vector, a dense generation-stamped overlay, and a sparse ordered alternative. Include sparse high slot indices and stale-generation failures. Record wall time, allocator calls/bytes, and retained scratch capacity; run correctness tests differentially against the current preflight result. The disproof case is a small world with batches of one or two commands where overlay initialization makes the candidate slower or materially larger.

Result:

Accepted. Five paired captures of the scaled `preflight_and_flush_mixed` matrix show representative large-world and large-batch median latency reductions from 76% through 99.7%, removing the former live-world-by-command growth curve. Small cases were neutral to faster rather than exposing the anticipated crossover regression. Ordering, owner, and stale-generation tests pass, and the implementation avoids copying every live entity by using a reusable generation-aware slot overlay.

Decision and fallback:

Retain the dense generation-aware overlay. It satisfies the work-removal and latency gates while preserving exact failing command indices and details. The overlay and restored command buffer share a 256 KiB retained-scratch ceiling per `World` after recovery, with the 16K-burst regression test enforcing the bound. Reconsider a sparse ordered fallback only if future workloads demonstrate repeated high-slot allocation churn under that cap.
