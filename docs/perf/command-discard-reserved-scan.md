# Remove the discard-time reserved-entity allocation

Priority: high

Confidence: confirmed by release allocation and five-run latency measurements
Target metric: allocator calls and latency when deferred command batches are discarded or fault cleanup runs

Hotspot:

`CommandQueue::discard` first materializes all reserved entities into a new `Vec<EntityId>`, releases them, and then clears the command buffer. The temporary vector allocates again on every non-empty discard containing a deferred spawn.

Evidence:

- [`src/command/queue.rs:184`](../../src/command/queue.rs#L184) calls `reserved_entities` before clearing the queue.
- [`src/command/queue.rs:194`](../../src/command/queue.rs#L194) creates an unreserved `Vec`, pushes every `SpawnReserved`, returns it, and drops it after the release loop.
- [`tests/allocation.rs:227`](../../tests/allocation.rs#L227) intends to prove command-buffer reuse by repeatedly enqueueing one spawn and discarding it.
- On this checkout (`dfd4177b`, Rust 1.96.0, Darwin arm64), `cargo test --release --features std --test allocation command_buffer_reuses_capacity_after_warmup -- --exact --test-threads=1` failed: allocation count rose from 1 to 2 on the second measured step (`bytes=128`). This directly disproves the stated steady-state allocation contract.

Candidate and mechanism:

Fuse collection and release: iterate `self.ops` by reference, release each `SpawnReserved` directly through the separately borrowed allocator, then clear `self.ops`. This removes the temporary collection while preserving command-buffer capacity. If borrow structure prevents the direct form, move only the ops vector to a local reusable buffer and restore it after releases; do not create a second vector of entity IDs.

Expected scope (not promised speedup):

This removes one allocator call/growth sequence per discard batch with at least one reserved spawn and reduces copied entity IDs from `O(reserved_spawns)` to zero. It is most relevant to explicit rollback, caught faults, and test/replay workloads; successful flushes do not use this exact path. For an empty queue or a batch without spawns, the latency gain may be below timer noise.

Semantic and operational risks:

- All reserved entities must be released exactly once, including when a later release reports an error.
- Current early-return behavior on allocator error leaves `ops` uncleared; any refactor must decide and test whether that observable failure behavior is preserved.
- Values boxed in discarded insert operations must still drop exactly once.
- The queue's retained capacity must not grow without bound after an exceptional burst.

Benchmark plan:

Extend the release allocation test to assert zero allocator calls after warmup for discard batches containing `[1, 8, 128, 2_048]` reserved spawns, plus insert payloads. Add a Divan scaled benchmark with discard-only and fault-cleanup cases, recording time, allocations, bytes, and retained command-buffer capacity. Include no-spawn batches as the disproof case: if the fused scan adds measurable overhead where no reservation can exist, consider tracking a reserved count to skip it. Preserve the non-clone drop-count and rollback tests in `tests/commands_deferred.rs`.

Result:

Accepted. Five paired benchmark captures of `discard_reserved` show median latency reductions of 59.8%, 43.0%, 32.3%, and 25.9% for batches of 1, 8, 128, and 2,048 reserved entities respectively. The release allocation contract `command_buffer_reuses_capacity_after_warmup`, which failed before the change, now passes: the fused scan performs no temporary reserved-entity allocation after warmup. Deferred non-clone drop and rollback coverage also remains green.

Decision and fallback:

Retain the direct scan. It removes the confirmed allocator regression and clears the latency acceptance threshold at every measured batch size without changing release order or early-error behavior. Command and preflight scratch retained after recovery is capped at 256 KiB per `World`; revert to the collecting implementation only if future semantic regression tests expose behavior the direct scan cannot preserve.
