# Amortize event-reader pruning

Priority: medium

Confidence: high; a bounded 128-operation checkpoint is measured
Target metric: event send/read tail latency as live and dropped reader counts grow

Hotspot:

Each send scans every weak reader cursor to remove dead readers. Every successful read scans them again. Reader creation and frame clearing also trigger full scans.

Evidence:

- [`src/event/queue.rs:72`](../../src/event/queue.rs#L72) calls `prune_readers` on every send.
- [`src/event/queue.rs:143`](../../src/event/queue.rs#L143) calls it again after every successful read.
- [`src/event/queue.rs:174`](../../src/event/queue.rs#L174) and [`src/event/queue.rs:237`](../../src/event/queue.rs#L237) add creation/clear scans.
- [`src/event/queue.rs:290`](../../src/event/queue.rs#L290) walks the whole cursor vector and performs `Weak::strong_count` for each entry; dead entries use unordered `swap_remove`.
- Existing release allocation tests use one reader and therefore do not characterize `O(messages * readers)` work.

Candidate and mechanism:

Decouple correctness from eager cleanup. Dead weak cursors are not consulted by send/read semantics, so prune on amortized checkpoints: reader creation, frame clear, when cursor storage crosses a threshold, or every power-of-two number of channel operations. Compare this with a stable reader-slot registry where `EventReader::drop` marks a shared slot dead without needing access to `EventStorage`. Keep all designs single-threaded (`Rc`/`Cell`) to match current semantics; concurrency is not a justification here.

Expected scope (not promised speedup):

Amortization reduces repeated full cursor scans for channels with many long-lived readers and high message rates. It can increase retained weak entries between checkpoints and adds a branch/counter on each operation. With zero or one reader, eager pruning may be equally fast or faster.

Semantic and operational risks:

- Deferred pruning must not change lag calculation, fork independence, reader start points, or payload retention.
- Dead weak entries retain only weak-control metadata, but unbounded churn can grow the vector until a checkpoint; define hard thresholds.
- A shared drop token adds ownership complexity and may allocate per reader.
- Do not introduce atomic reference counting or locks into this single-threaded runtime without a separate measured requirement.

Benchmark plan:

Add component benchmarks for reader counts `[0, 1, 8, 64, 1_024]`, send/read batches `[1, 64, 4_096]`, and reader churn patterns: all live, 90% dropped before send, and continuous create/drop. Compare eager scan, periodic pruning intervals, threshold pruning, and slot-token cleanup. Record throughput, p50/p95/p99 operation latency, cursor-vector length, allocations, and retained bytes after churn. The disproof case is zero/one-reader traffic and high churn where delayed cleanup grows memory or makes checkpoint tails worse.

Result:

Accepted. Dead weak-reader slots are pruned at a bounded 128-operation checkpoint instead of on every send/read/create. Median-of-five send batches improved 69-98% with 64-1,024 readers; zero/one-reader batches improved 7-17%. Reader create/drop churn improved 29.27% at one, 34.06% at 64, and 4.83% at 4,096.

Decision and fallback:

Retain the 128-operation checkpoint and its bounded-stale-slot tests. Lag calculation, fork behavior, start policy, and allocation contracts pass. Revert to eager pruning if a constrained host shows unacceptable weak-entry retention or checkpoint tail latency.
