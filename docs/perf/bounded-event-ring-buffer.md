# Replace bounded-event front shifts with ring storage

Priority: high

Confidence: high; crossover measured and implemented adaptively
Target metric: event-send latency and bytes moved for bounded channels

Hotspot:

When a bounded channel exceeds capacity, it removes index zero from both `payloads` and `sequences`. `Vec::remove(0)` shifts every retained element, so steady-state send cost is linear in configured capacity.

Evidence:

- [`src/event/queue.rs:257`](../../src/event/queue.rs#L257) enforces retention after every send.
- [`src/event/queue.rs:260`](../../src/event/queue.rs#L260) repeatedly calls `self.payloads.remove(0)` and `self.sequences.remove(0)` until within capacity.
- The parallel vectors store boxed payload pointers and `u64` sequences, so both tails are shifted.
- [`tests/allocation.rs:251`](../../tests/allocation.rs#L251) and [`tests/allocation.rs:298`](../../tests/allocation.rs#L298) use `EventOptions::bounded(1)`. Both release allocation tests passed on this checkout, but capacity one is precisely the case where front shifting is minimal.
- Existing Divan benches have no event-throughput family or capacity sweep.

Candidate and mechanism:

Use `VecDeque` for payloads and sequences, or a single ring of `{ sequence, payload }` entries, and evict with `pop_front`. A single entry ring improves synchronization between payload and sequence storage and makes eviction `O(1)`. Preserve the existing free-payload pool for warmed allocation reuse.

Expected scope (not promised speedup):

The candidate changes steady-state bounded retention from `O(capacity)` pointer/integer movement per send to amortized `O(1)`. It should matter for large retention windows or bursty channels. `Vec` is likely faster and denser for capacity one and may win for small capacities due to simpler indexing and contiguous layout.

Semantic and operational risks:

- Reader position lookup, lag reporting, `oldest_retained`, frame clear, recycling, and sequence overflow behavior must remain exact.
- `VecDeque` can split storage into two slices, affecting scans and cache locality.
- Combining payload and sequence changes layout; an entry may add padding and reduce cache density.
- Ring capacity and free-pool retention can pin burst memory; measure post-burst RSS/capacity.

Benchmark plan:

Add Divan event benchmarks with capacities `[1, 4, 16, 256, 4_096]`, event payload sizes `[8, 64, 1_024]` bytes, send/read ratios (send-only, one current reader, one lagging reader), and burst then steady phases. Compare parallel `Vec`, parallel `VecDeque`, and a single-entry ring. Record send throughput, p50/p95/p99 latency, allocations/bytes after warmup, and retained capacity. The required disproof/crossover case is capacity one and four, where the current vector may be faster and smaller. Differential tests must preserve exact event order and dropped counts.

Result:

Accepted with a measured crossover. Manual, frame, and bounded channels at capacity 16 or below use linear `Vec` slots; larger bounded channels use a single-entry `VecDeque` ring. Both reuse warmed payload boxes and preserve active order/sequence in one entry. Median-of-five send latency stayed within 1.9% for the 1,024-byte capacity-one/control cases, improved 49.65% at capacity 256 and 91.88% at 4,096, and improved 84-99% for 8/64-byte payloads at large capacities. Current/lagging reader cases improved across the sweep.

Decision and fallback:

Retain the adaptive threshold at 16. Small linear eviction reuses the oldest slot before insertion; large ring eviction is O(1). Order, lag, retention-reconfiguration, and six release allocation contracts pass. Recalibrate the threshold on a shipping constrained target before changing it.
