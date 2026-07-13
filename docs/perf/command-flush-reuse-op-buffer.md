# Reuse command storage after successful flushes

Priority: high

Confidence: high; allocation reuse and steady/post-burst latency are measured
Target metric: allocations and latency across repeated successful deferred-command flushes

Hotspot:

A successful flush removes the queue's entire `Vec<CommandOp>` with `mem::take`, consumes and drops that vector, and leaves the queue with a fresh zero-capacity vector. The next command batch must grow command storage again.

Evidence:

- [`src/command/queue.rs:214`](../../src/command/queue.rs#L214) implements `take_ops` as `core::mem::take(&mut self.ops)`.
- [`src/world/mod.rs:550`](../../src/world/mod.rs#L550) takes the vector, consumes it in a `for` loop, then drops its allocation rather than returning cleared storage to `CommandQueue`.
- [`tests/allocation.rs:227`](../../tests/allocation.rs#L227) covers repeated **discard**, not repeated successful non-empty flushes.
- [`tests/allocation.rs:214`](../../tests/allocation.rs#L214) covers only empty flushes after warmup.
- [`benches/world_lifecycle.rs:47`](../../benches/world_lifecycle.rs#L47) rebuilds a world for every measured call, so its 1.291 us median on this checkout includes setup and cannot reveal cross-frame buffer reuse.

Candidate and mechanism:

Temporarily move the ops vector out to satisfy the mutable-world borrow, consume operations through `drain(..)` or an index-safe equivalent, clear any unconsumed tail on error, and return the now-empty allocation to `CommandQueue` before returning. Encapsulate this as a queue method so success and mid-commit failure paths both restore reusable storage.

Expected scope (not promised speedup):

The candidate should eliminate command-vector growth after a representative batch has warmed capacity. It does not eliminate per-value boxing for `Insert` commands or preflight scratch allocation. Benefits should be visible in steady repeated structural batches; one-shot worlds can be neutral or slightly slower due to restoration bookkeeping.

Semantic and operational risks:

- A commit can fail after earlier operations have applied. Remaining boxed values must drop once and the public guarantee that pending commands are cleared must remain intact.
- Restoring the buffer before every return is easy to miss; use a narrow helper or guard and targeted failure tests.
- Retaining a burst-sized vector pins memory. Define a capacity ceiling or post-burst shrink policy based on measured command distributions.
- `drain` drop behavior during panic must not reapply or leak operations.

Benchmark plan:

Add a Divan benchmark that constructs one world outside the timed loop, warms with a chosen batch size, then repeatedly queues and successfully flushes `[1, 8, 128, 2_048]` spawn/insert batches while reusing freed slots. Pair it with the counting allocator and record allocation count/bytes per frame, latency distribution, and retained capacity after a single 16K-command burst followed by small frames. Compare current `mem::take`, restored `Vec`, and a capped-retention variant. The disproof case is one-shot or highly bursty batches where retained memory or cleanup bookkeeping outweighs reuse.

Result:

Accepted. Across five paired captures, warmed successful non-empty flushes improved median latency by 34.9% through 95.8% over the measured batch sizes. Small frames after a 16K-operation burst improved by 75.8% through 99.1%. The new release allocation contract `successful_command_flush_reuses_capacity_after_warmup` passes, confirming that the restored operation vector eliminates steady command-buffer regrowth.

Decision and fallback:

Retain drain-and-restore reuse on both successful and commit-error returns. Existing partial-failure and non-clone drop tests remain green, warmed flushes meet the allocation contract, and combined command/preflight scratch is capped at 256 KiB per `World` after recovery. Fall back to dropping the allocation only if a future workload shows the bounded retention policy increases tail latency or memory pressure beyond its measured gains.
