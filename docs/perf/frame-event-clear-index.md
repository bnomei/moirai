# Index frame-event channels by operation

Priority: medium

Confidence: high; the proposed index was rejected and an O(1) recycle alternative retained
Target metric: empty update/render boundary latency as registered event-channel count grows

Hotspot:

Every update and render boundary scans all event channels to find those owned by that operation, even when no frame events were sent and most channels use manual or bounded retention.

Evidence:

- [`src/app.rs:190`](../../src/app.rs#L190) clears update-owned frame events on every completed update; [`src/app.rs:240`](../../src/app.rs#L240) does the same for render.
- [`src/world/events.rs:163`](../../src/world/events.rs#L163) forwards each boundary to `EventStorage::clear_frame`.
- [`src/event/queue.rs:231`](../../src/event/queue.rs#L231) walks every channel, checks retention, then recycles only matching frame channels.
- The current `app_update` benchmark has no registered events; on this checkout it reported a 1.249 us median, so it provides no channel-count scaling evidence.
- The allocation `event_compact` case registers one update-frame channel and passed, but it does not cover many non-frame channels.

Candidate and mechanism:

At registration/channel creation, maintain dense lists of frame-channel indices for `Update` and `Render`, and clear only the relevant list. If retention can change after creation, update the index transactionally; current code sets retention through `ensure_channel`, so build-time registration is the natural boundary. A dirty-list variant that records only frame channels receiving an event in the current operation is a second candidate and can remove empty-channel clears too.

Expected scope (not promised speedup):

An operation index changes boundary work from `O(all_channels)` to `O(frame_channels_for_operation)`; a dirty list changes it to `O(active_frame_channels)`. It should help applications registering many persistent/lifecycle channels. For a handful of channels, extra index storage and send-side dirty tracking can lose.

Semantic and operational risks:

- Every frame channel must be indexed exactly once under the correct operation.
- Lifecycle-generated channels and future retention reconfiguration must not bypass index maintenance.
- Dirty tracking needs duplicate suppression without scanning, and must still clear after panic/fault cleanup according to current lifecycle guarantees.
- Additional index vectors retain memory and slightly increase build-time work/code size.

Benchmark plan:

Extend schedule/event Divan cases with total channel counts `[0, 8, 64, 512, 4_096]`, frame fractions `[0%, 1%, 25%, 100%]`, and active-frame fractions `[0%, 1%, 100%]`. Measure update and render boundary latency separately for full scan, operation index, and dirty index; record allocations and index bytes. Include a tiny all-frame workload as the disproof case, where a direct contiguous full scan may be faster. Differential tests must verify update/render ownership and panic cleanup.

Result:

The per-operation channel index was implemented, measured, and reverted: active-frame cases regressed 16-145% because index maintenance/indirection outweighed the avoided scan. The retained alternative keeps the full channel scan but clears each matching channel in O(1) by resetting its logical active length and reusing its slots. Median-of-five clear latency then improved 14-84% for active frame sets and stayed within 2.44% for zero-active controls.

Decision and fallback:

Reject the channel index and retain full scanning plus O(1) payload-slot recycle. This closes the measured clear cost without extra index state or retention-reconfiguration risk. Reopen operation/dirty indexing only beyond the measured 256-channel range or with a workload dominated by non-frame channels.
