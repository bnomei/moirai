# Event-reader drains repeatedly scan retained history

Status: accepted; direct sequence offsets remove quadratic drain work.

Hotspot:

`EventStorage::read_next` searched retained entries from the front for every read. Because event sequences are contiguous and a reader advances one sequence at a time, draining `N` retained events performed triangular `O(N^2)` position work.

Evidence:

Active event sequences are contiguous, and `oldest_retained` is the sequence immediately before the first active entry. The lag check establishes `cursor >= oldest_retained`, so the next entry position is exactly `cursor - oldest_retained` while that offset is below `active_len`. The testkit sequence-override helper rebases this floor and any live cursors together when an empty channel deliberately jumps near exhaustion. Manual, frame, bounded-ring, lag, fork, and sequence-exhaustion tests encode these invariants.

Candidate and mechanism:

Compute the checked sequence offset once and index the active entry directly. Remove the linear `position_after` scan from both linear and ring storage.

Expected scope (not promised speedup):

The change primarily benefits readers draining retained history. Reading only the first retained event was the losing/control case because the former scan stopped at position zero.

Semantic and operational risks:

An incorrect relationship between `oldest_retained`, `active_len`, and entry sequences could skip or duplicate events. Checked subtraction and conversion keep malformed internal state on the no-entry path; existing ordering and lag tests cover manual, frame, and wrapped ring channels.

Benchmark plan:

`benches/events.rs` builds manual-retention channels outside the timed region and measures both full drains and first-item reads at `{1, 16, 256, 4_096}` retained events. Five prebuilt baseline/candidate pairs run in alternating order with the OS timer, one thread, 100 samples x 100 iterations, executable SHA-256 identity, and a start/end one-minute load ceiling of 4.0 on the 10-logical-CPU host.

Result:

Accepted. Median-of-five full-drain latency improved 9.84% at one event, 30.33% at 16, 83.22% at 256, and 98.56% at 4,096 (`2.126 ms` to `30.60 us`). Every drain pair improved, including the one-event control by at least 3.20%. First-item median controls also improved 2.03-4.93%. Those sub-11 ns controls do not meet the default 5%-in-four-of-five latency gate, but pass the documented work-removal/control rule: the scan is eliminated and no control median regressed.

Decision and fallback:

Retain direct sequence-offset indexing. The event queue unit suite, including capacity-17/256 repeated overflow, exact lag, frame clear, fork, and manual ordering, passes. Restore the scan only if a future retention mode permits non-contiguous active sequences; that mode must add a benchmark and explicitly redefine the offset invariant.
