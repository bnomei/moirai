# pd-asteroids ECS characterization ledger

This is the classified source inventory for every Rust test function under
`game-core/src/ecs/` in the pd-asteroids source tree. The inventory was verified against the live
source on 2026-07-12. It contains 151 functions: 12 annotated with `#[test]` and 139 annotated with
`#[rstest]`.

The label applies to the test's intent, not to its exact source API:

- `preserve`: Moirai should retain the observable behavior.
- `adapt`: the intent remains, but the API, ownership, timing, or error behavior must change.
- `reject`: the behavior or extension surface is deliberately absent from Moirai.

An owner is the phase that must close the case. For a rejected case, that means documenting the
negative contract and, where useful, adding an API/compile-fail test; it does not mean implementing
the rejected behavior. Phase assignments follow [the architecture contract](./ARCHITECTURE.md) and
[Phase 0](../PHASE_0_ANALYSIS.md).

## Phase 0 locks proposed for sign-off

| Area | Classification | Frozen replacement contract |
| --- | --- | --- |
| System pipes | The two `system_pipe_*` tests are `reject`. | No type-erased pipe; compose normal Rust functions inside a system. |
| System interval | `system_interval_buffers_time` is `reject` to downstream policy. | FixedUpdate owns repeated simulation; throttling uses a host timer/run condition. |
| Event gating controls | Runtime enable/disable, silent dropping, and fabricated-id behavior are `reject`. | Registered sends enqueue/error; compiled emits/consumes roles validate order and scheduled access. |
| Anonymous event reader | `default_reader_id_is_shared` is `reject`. | Explicit owner-scoped `EventReader<E>` selects oldest-retained/from-now and reports lag. |
| State stack | The three push/pop/clear cases are `reject` to downstream state policy. | `State<S>` retains current/previous/pending only; pause/navigation stacks are host resources. |
| Cached added/changed filters | The panic test is `adapt` to `QueryError`. | QueryCache applies ChangeTick windows; QueryResultCache returns `MovingChangeWindow`. |
| Fixed-time default | Both `TimeFixed` construction tests are `adapt`. | Fixed is disabled by default; positive Duration is required when FixedUpdate has systems. |
| Tick overflow | `advance_tick_wraps_and_increments` is `adapt`. | First WorldTick is 1; World/Fixed/Change tick exhaustion is terminal and never wraps. |

The checked Moirai evidence mapping is generated from this inventory plus the canonical proof table
in `scripts/generate_parity_ledger.py`. Run
`uv run python scripts/generate_parity_ledger.py --check` to validate every cited path and test
symbol and to detect drift without modifying the working tree. Evidence establishes the neutral
core contract only; it does not establish either downstream game cutover.

## Summary

| Source file | Preserve | Adapt | Reject | Total |
| --- | ---: | ---: | ---: | ---: |
| `commands.rs` | 2 | 2 | 0 | 4 |
| `components.rs` | 4 | 1 | 0 | 5 |
| `entity.rs` | 5 | 2 | 0 | 7 |
| `events.rs` | 8 | 6 | 9 | 23 |
| `profiler.rs` | 0 | 1 | 0 | 1 |
| `resources.rs` | 4 | 2 | 4 | 10 |
| `schedule.rs` | 13 | 9 | 3 | 25 |
| `state.rs` | 0 | 2 | 3 | 5 |
| `storage.rs` | 4 | 0 | 0 | 4 |
| `time.rs` | 0 | 2 | 0 | 2 |
| `world.rs` | 18 | 39 | 8 | 65 |
| **Total** | **58** | **66** | **27** | **151** |

| Owner | Cases | Primary closure |
| --- | ---: | --- |
| Phase 2 | 18 | identity, registration, storage invariants |
| Phase 3 | 50 | World data lifecycle, commands, resources, events |
| Phase 4 | 35 | App lifecycle, schedule, state, fixed time |
| Phase 5 | 27 | query behavior and owner-scoped caches |
| Phase 6 | 17 | allocation and diagnostics regression evidence |
| Downstream host | 4 | State-stack and interval policy remain host-owned; Playdate clock/log FFI is also downstream but has no direct source test. |

## `game-core/src/ecs/entity.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 112 | `entity_id_roundtrip` | adapt | Phase 2 | Keep the private 32-bit slot/32-bit generation representation; expose no public raw constructor/conversion. |
| 120 | `entity_id_from_u64_roundtrip` | adapt | Phase 2 | Test private packing internally without making raw bits a public or persistent identity. |
| 128 | `allocates_sequential_indices` | preserve | Phase 2 | Deterministic fresh-slot allocation is useful and compatible with the checked allocator. |
| 139 | `free_reuses_index_with_bumped_generation` | preserve | Phase 2 | Reuse must bump generation; overflow retires the slot instead of wrapping it back into service. |
| 149 | `free_rejects_stale_ids` | preserve | Phase 2 | Same-World stale and double-free rejection is a core safety invariant. |
| 157 | `is_alive_tracks_generation` | preserve | Phase 2 | Liveness requires both occupancy and the current generation. |
| 170 | `allocator_default_marks_unknown_ids_dead` | preserve | Phase 2 | Unknown slots are never live in a new allocator. |

## `game-core/src/ecs/components.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 215 | `registry_returns_stable_ids` | adapt | Phase 2 | Exact repeat registration stays idempotent, but conflicts return an error and the registry remains private. |
| 228 | `registry_resolves_by_type` | preserve | Phase 2 | Typed component registration must resolve consistently within one World schema. |
| 237 | `registry_resolves_by_name` | preserve | Phase 2 | Authored/dynamic component names still resolve through checked registration. |
| 247 | `registry_tracks_storage_and_tags` | preserve | Phase 2 | Tag versus data-component layout is required metadata. |
| 256 | `registry_tracks_storage_kind` | preserve | Phase 2 | Sparse/table policy remains observable through component metadata, not raw storage access. |

## `game-core/src/ecs/storage.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 201 | `insert_get_remove` | preserve | Phase 2 | Private sparse storage must implement the basic value lifecycle correctly. |
| 215 | `insert_replaces_existing` | preserve | Phase 2 | Reinsertion replaces and returns the previous value without duplicating membership. |
| 224 | `remove_swaps_dense_tail` | preserve | Phase 2 | Dense compaction must retain all unaffected entities and correct sparse indexes. |
| 240 | `iterates_all_entities` | preserve | Phase 2 | Internal traversal must visit every live member exactly once; order is not frozen. |

## `game-core/src/ecs/commands.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 138 | `bundle_collects_components_in_order` | adapt | Phase 3 | Replace the public boxed-id bundle with the safe `BundleWriter` trait and `DynamicBundle`; retain deterministic insertion order. |
| 152 | `commands_queue_and_take_ops` | adapt | Phase 3 | Keep deferred structural operations behind borrowed `Commands`; keep `CommandOp` private and remove Spawn-no-op and `RunSystem`. |
| 208 | `commands_restore_reuses_queue_buffer` | preserve | Phase 3 | World-owned command storage should be cleared and reused after a flush. |
| 220 | `commands_drain_clears_queue` | preserve | Phase 3 | Consuming a pending batch leaves no commands queued. |

## `game-core/src/ecs/resources.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 285 | `insert_get_remove` | preserve | Phase 3 | Registered typed resource values support checked immediate insert, borrow, and optional removal. |
| 296 | `insert_replaces_existing` | preserve | Phase 3 | Inserting the same resource type replaces and returns the prior value. |
| 305 | `get_mut_updates_resource` | preserve | Phase 3 | Typed mutable access updates the stored resource. |
| 315 | `tracks_added_and_changed_ticks` | adapt | Phase 3 | Record `WorldTick` plus a monotonic revision; callers no longer inject arbitrary tick values into the store. |
| 328 | `insert_named_replaces_by_name` | reject | Phase 3 | Named resources are intentionally absent; hosts use typed newtypes for multiple logical instances. |
| 342 | `set_named_prefers_name_and_sets_type` | reject | Phase 3 | Name/type aliasing creates surprising identity and is not ported. |
| 353 | `set_named_falls_back_to_type_and_sets_name` | reject | Phase 3 | A resource may not silently acquire a second identity through a string fallback. |
| 364 | `get_mut_named_updates_ticks` | reject | Phase 3 | Named mutable access is removed with the named-resource surface. |
| 376 | `set_inserts_when_missing_and_updates_when_present` | preserve | Phase 3 | Typed set/upsert preserves the original added tick and advances change revision on replacement. |
| 388 | `take_and_restore_entry_roundtrip` | adapt | Phase 3 | The raw entry operation stays private and is proven through alias-safe `World::resource_scope`. |

## `game-core/src/ecs/state.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 86 | `new_initializes_state` | adapt | Phase 4 | Preserve initialization on generic `State<S>`; `GameState` remains pd-asteroids code. |
| 94 | `set_updates_current_and_previous` | adapt | Phase 4 | Retain transition history without coupling Moirai to a game enum. |
| 102 | `push_and_pop_manage_stack` | reject | Downstream host | Pause/overlay stack policy is not part of generic `State<S>`; a host controller may layer it on top. |
| 117 | `pop_without_stack_is_noop` | reject | Downstream host | Empty-pop behavior belongs to the omitted host stack policy. |
| 126 | `clear_stack_drops_saved_states` | reject | Downstream host | Clearing saved states is host navigation policy, not an ECS state primitive. |

## `game-core/src/ecs/time.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 36 | `new_initializes_step_and_accumulator` | adapt | Phase 4 | Replace public `f32` fields with validated `Duration`-based fixed-time state. |
| 43 | `default_initializes_zeroed_time` | adapt | Phase 4 | Fixed is disabled until AppBuilder receives positive `FixedConfig`; config has no zero-step Default. |

## `game-core/src/ecs/profiler.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 239 | `profiler_record_is_allocation_free_after_warmup` | adapt | Phase 6 | Apply the allocation budget to neutral diagnostics aggregation; Playdate clock, logging, and FFI stay downstream. |

## `game-core/src/ecs/events.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 546 | `event_id_conversions_roundtrip` | reject | Phase 3 | Owner-scoped `EventId` is non-forgeable; raw construction/conversion is not public behavior. |
| 554 | `initializes_empty` | preserve | Phase 3 | A private event queue starts with no retained payloads or cursor progress. |
| 563 | `defaults_construct_empty_structs` | adapt | Phase 3 | Keep empty initialization while moving registries and queues behind World-owned event channels. |
| 576 | `sends_event_to_queue` | preserve | Phase 3 | A registered send appends one payload to its channel. |
| 583 | `queues_multiple_events_in_order` | preserve | Phase 3 | Event delivery is deterministic FIFO within one channel. |
| 600 | `default_reader_id_is_shared` | reject | Phase 3 | Remove the magic anonymous/global reader; consumers use explicit owner-scoped `EventReader<E>`. |
| 610 | `separate_reader_ids_have_separate_cursors` | preserve | Phase 3 | Each registered reader advances independently. |
| 627 | `pooled_events_reuse_payloads` | adapt | Phase 6 | Keep pooling private and prove reinitialization plus warm-path allocation behavior without exposing pool length. |
| 654 | `compact_handles_empty_and_no_readers` | preserve | Phase 3 | Private compaction safely handles empty and readerless channels. |
| 664 | `compact_releases_only_consumed_events` | preserve | Phase 3 | Persistent history is reclaimed only after every relevant reader has passed it. |
| 682 | `compact_noops_when_reader_cursor_is_zero` | preserve | Phase 3 | A lagging reader prevents premature reclamation. |
| 697 | `registry_returns_stable_ids` | adapt | Phase 3 | Exact registration repeats stay stable, but the registry is private and handles are owner-scoped and checked. |
| 710 | `registry_is_empty_tracks_entries` | preserve | Phase 3 | Retain as a private registry invariant. |
| 718 | `registry_debug_includes_type_name` | adapt | Phase 6 | Assert contextual public diagnostics rather than freezing private `Debug` output. |
| 727 | `gating_defaults_off` | reject | Phase 3 | No public mutable gate mode; registered sends must not silently depend on hidden runtime toggles. |
| 735 | `gating_blocks_until_enabled` | reject | Phase 3 | Replace silent runtime drops with checked registration and ScheduleBuilder event-role validation. |
| 746 | `gating_can_be_disabled` | reject | Phase 3 | Public runtime gating toggles are absent from the prepared-execution contract. |
| 757 | `pooled_gating_blocks_until_enabled` | reject | Phase 3 | Both public pooling controls and public runtime gating are omitted. |
| 780 | `enable_by_id_requires_gating_and_expands_enabled` | reject | Phase 3 | A fabricated sparse id must never expand registries or create queues. |
| 794 | `send_pooled_by_id_respects_gating_and_unknown_ids` | reject | Phase 3 | Unknown ids fail contextually instead of materializing channels on demand. |
| 813 | `read_next_by_id_returns_none_for_missing_queue` | reject | Phase 3 | Invalid handles are prevented or return an event error, not an ambiguous empty read. |
| 821 | `pooled_send_and_read_are_allocation_free_after_warmup` | adapt | Phase 6 | Recast through registered typed events with retained capacity and no raw-id/gating surface. |
| 865 | `event_queue_compact_is_allocation_free_after_warmup` | adapt | Phase 6 | Preserve as a private warm-capacity allocation proof with an explicit topology bound. |

## `game-core/src/ecs/schedule.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 831 | `validates_ordered_producer_consumer` | preserve | Phase 4 | A declared producer/consumer edge with deterministic order must compile. |
| 843 | `rejects_missing_order_for_same_stage_event` | preserve | Phase 4 | Same-stage event flow requires an ordering dependency. |
| 854 | `rejects_missing_event_producer` | preserve | Phase 4 | A consumed event without a declared producer is a build error. |
| 864 | `ignores_component_event_missing_producer` | adapt | Phase 4 | Component lifecycle production stays engine-owned but uses typed channels, never magic `OnAdd:<name>` strings. |
| 871 | `ignores_cross_stage_event_order` | preserve | Phase 4 | Explicit stage order supplies cross-stage producer/consumer order. |
| 879 | `rejects_cross_stage_dependency_links` | preserve | Phase 4 | System edges remain stage-local; stage edges express cross-stage order. |
| 890 | `rejects_missing_resources` | preserve | Phase 4 | Required typed resources are validated and locked at build time. |
| 900 | `rejects_duplicate_labels` | preserve | Phase 4 | Duplicate diagnostic/authoring labels are contextual build errors. |
| 911 | `startup_stage_runs_once` | preserve | Phase 4 | Startup executes once on the first successful App update. |
| 930 | `stage_flush_updates_are_allocation_free_after_warmup` | adapt | Phase 6 | Recast through compiled `App` and explicit `Commands`; retain the steady-state allocation budget. |
| 959 | `fixed_update_steps_respect_accumulator` | adapt | Phase 4 | Use `Duration`, `FixedStep`, the substep cap/debt policy, and fractional-remainder preservation. |
| 980 | `flush_mode_stage_applies_commands_between_stages` | adapt | Phase 4 | Systems enqueue through `world.commands()`; a stage flush still makes mutations visible downstream. |
| 1013 | `flush_mode_end_defers_commands_until_update_end` | adapt | Phase 4 | Use explicit commands and retain end-of-operation visibility semantics. |
| 1046 | `system_in_state_gates_execution` | adapt | Phase 4 | Gate on generic `State<S>` through App-owned execution and ticks. |
| 1071 | `system_state_changed_runs_on_transition` | adapt | Phase 4 | Observe an explicitly applied generic transition, not manually injected ticks. |
| 1098 | `state_transition_update_is_allocation_free_after_warmup` | adapt | Phase 6 | Recast around the generic transition boundary and compiled App. |
| 1128 | `system_interval_buffers_time` | reject | Downstream host | Per-system intervals are omitted; use FixedUpdate or a host timer/run condition. |
| 1148 | `system_pipe_passes_values_between_piped_systems` | reject | Phase 4 | Untyped `Box<dyn Any>` piping is absent; resources and events provide explicit dataflow. |
| 1187 | `system_pipe_without_payload_is_allocation_free_after_warmup` | reject | Phase 4 | The underlying untyped pipe feature is intentionally omitted. |
| 1231 | `system_run_if_gates_execution` | preserve | Phase 4 | Per-system run conditions remain part of compiled schedule policy. |
| 1255 | `set_run_if_gates_execution_once_per_stage` | preserve | Phase 4 | A set condition evaluates once per stage invocation and gates all members. |
| 1305 | `rejects_unknown_set` | preserve | Phase 4 | Unknown set references fail during schedule compilation. |
| 1315 | `rejects_duplicate_set_labels` | preserve | Phase 4 | Duplicate set labels remain build errors. |
| 1326 | `set_and_system_conditions_are_both_required` | preserve | Phase 4 | Set and member conditions compose with documented AND semantics. |
| 1362 | `run_if_and_set_are_allocation_free_after_warmup` | adapt | Phase 6 | Preserve against compiled order/condition buffers under App ownership. |

## `game-core/src/ecs/world.rs`

| Line | Test function | Class | Owner | Rationale |
| ---: | --- | --- | --- | --- |
| 4963 | `query1_returns_all_entities_with_component` | preserve | Phase 5 | Query1 visits every matching live entity exactly once. |
| 4981 | `query2_returns_intersection` | preserve | Phase 5 | Query2 returns the intersection of both component memberships. |
| 5000 | `table_component_insert_get_query` | preserve | Phase 5 | Table-backed values participate in typed access, queries, and removal. |
| 5022 | `table_component_migration_preserves_ticks` | preserve | Phase 2 | Archetype moves preserve existing values and their added/changed metadata. |
| 5084 | `query2_mixed_table_and_sparse_components` | preserve | Phase 5 | The required safe matrix includes mixed table/sparse read traversal. |
| 5114 | `query_cached_reuses_plan` | adapt | Phase 5 | Retain compiled-plan reuse behind an owner-scoped cache handle, not a caller-supplied raw key. |
| 5125 | `query_cached_results_updates_on_add_remove` | preserve | Phase 5 | Membership-result caches track structural additions and removals. |
| 5156 | `query_cached_results_respects_without` | preserve | Phase 5 | Cached membership honors exclusion filters across structural changes. |
| 5203 | `query_excludes_inactive_when_enabled` | adapt | Phase 5 | Preserve explicit exclusion/include behavior without a magic component name or World-global toggle. |
| 5236 | `query_cached_results_respects_inactive_changes` | adapt | Phase 5 | Cache invalidation follows the explicit host-supplied exclusion filter. |
| 5282 | `query_cached_results_clear_rebuilds` | adapt | Phase 5 | Owner-scoped cache state can be invalidated and rebuilt without public raw keys. |
| 5312 | `query_cached_results_enables_events_when_gated` | adapt | Phase 5 | Preserve automatic cache invalidation through private lifecycle tracking, not public event-gate mutation. |
| 5338 | `apply_event_gating_is_allocation_free_after_warmup` | adapt | Phase 6 | Move role resolution to build time and prove hot event dispatch instead of `World::schedule_mut` gating scans. |
| 5359 | `query_cached_results_is_allocation_free_after_warmup` | adapt | Phase 6 | Retain the warm cache-hit budget using owner-scoped handles and private storage. |
| 5391 | `query_cached_results_rejects_added_or_changed` | adapt | Phase 5 | QueryCache applies ChangeTick windows; QueryResultCache returns `MovingChangeWindow` rather than panicking. |
| 5406 | `command_buffer_applies_on_flush` | adapt | Phase 3 | Idle structural methods are immediate; reproduce deferred visibility with explicit `world.commands()`. |
| 5417 | `command_buffer_flush_is_allocation_free_after_warmup` | preserve | Phase 6 | A representative warmed structural batch flushes without allocation. |
| 5446 | `spawn_bundle_inserts_components_on_flush` | adapt | Phase 3 | Use immediate idle spawn or deferred `Commands::spawn_bundle` with the safe bundle contract. |
| 5460 | `bundle_insert_helpers_resolve_components` | adapt | Phase 3 | Replace boxed helpers with typed `BundleWriter` and checked `DynamicBundle` insertion. |
| 5474 | `bundle_insert_helpers_return_false_when_component_missing` | adapt | Phase 3 | Missing registration returns a contextual bundle/build error rather than an uninformative boolean. |
| 5490 | `command_buffer_despawn_removes_components` | adapt | Phase 3 | Use explicit deferred commands during execution; despawn still removes all membership and invalidates liveness atomically. |
| 5504 | `query_respects_without_list` | preserve | Phase 5 | Query1 exclusion filters remove entities carrying any excluded component. |
| 5530 | `event_gating_from_schedule_controls_dispatch` | adapt | Phase 4 | Compile typed/named roles into ordering and scheduled access checks; sends never disappear behind a runtime gate. |
| 5545 | `read_event_typed_downcasts_payload` | adapt | Phase 3 | Typed channels provide typed reads directly; dynamic named channels report a type mismatch instead of exposing downcast plumbing. |
| 5569 | `read_event_is_allocation_free_after_warmup` | adapt | Phase 6 | Retain the warm typed-read budget without runtime gating or raw reader ids. |
| 5595 | `frame_events_are_dropped_after_update` | adapt | Phase 4 | `App::update` clears frame events only after final flush and the `update_with` observation point. |
| 5604 | `persistent_events_survive_update_when_unread` | adapt | Phase 4 | Persistent typed events survive App operations until reader/retention policy permits compaction. |
| 5617 | `validate_schedule_via_world` | adapt | Phase 4 | `AppBuilder::build` validates required typed resources; World has no schedule or named resources. |
| 5628 | `query_added_filters_by_tick` | preserve | Phase 5 | Added filters compare component metadata against an explicit ChangeTick/QueryCursor window. |
| 5662 | `query_changed_filters_by_tick` | preserve | Phase 5 | Changed filters select mutations strictly newer than the observation boundary. |
| 5700 | `get_mut_marks_changed_tick` | preserve | Phase 3 | Typed mutable component access advances change metadata for later queries. |
| 5739 | `query_cached_params_updates_last_tick` | adapt | Phase 5 | Owner-scoped query state advances its observation only through the checked query lifecycle. |
| 5781 | `query_cache_is_owner_scoped` | adapt | Phase 5 | Strengthen the test to reject cache handles across Worlds using the private owner token. |
| 5826 | `register_component_records_metadata_and_events` | adapt | Phase 3 | Checked component registration prepares typed lifecycle channels without magic event strings. |
| 5842 | `register_component_untyped_requires_tag` | adapt | Phase 2 | Untyped authored registration is tag-only and returns a contextual error, not a debug panic. |
| 5848 | `component_events_emitted_on_insert_and_remove` | adapt | Phase 3 | Preserve add/remove lifecycle events through typed helpers and component ids. |
| 5870 | `component_add_not_emitted_on_replace` | preserve | Phase 3 | Replacing an existing value is a change, not a second add. |
| 5885 | `component_events_enabled_on_read_when_gated` | adapt | Phase 3 | Reader/role registration prepares lifecycle delivery; a read does not toggle a public runtime gate. |
| 5899 | `component_event_dispatch_is_allocation_free_after_warmup` | adapt | Phase 6 | Prove typed lifecycle dispatch on private pooled channels without fabricated ids. |
| 5948 | `read_component_event_is_allocation_free_after_warmup` | adapt | Phase 6 | Retain the warmed typed lifecycle-read budget without magic names. |
| 5974 | `query_ids_filters_with_and_without` | preserve | Phase 5 | Dynamic id queries implement conjunction of includes and exclusion of any `without` member. |
| 6001 | `query_ids_excludes_inactive_when_enabled` | adapt | Phase 5 | Express host inactivity as an ordinary explicit filter, never a reserved `Inactive` tag. |
| 6040 | `query_ids_cached_last_tick_updates_on_exhaust` | preserve | Phase 5 | A fully exhausted change query advances its observation boundary. |
| 6070 | `query_ids_cached_last_tick_skips_on_partial_iteration` | preserve | Phase 5 | Dropping a partially consumed change query must not hide unseen matches on the next run. |
| 6098 | `query_spec_from_names_reports_missing_components` | reject | Phase 5 | Query specs accept only owner-checked `ComponentId` values; name lookup and its missing-name error are intentionally absent. |
| 6107 | `query_spec_from_names_builds_expected_spec` | reject | Phase 5 | Name-based query identity is intentionally absent; hosts retain authored-name policy outside Moirai core. |
| 6125 | `query1_params_panics_on_unknown_component` | adapt | Phase 5 | Unknown or foreign component handles return `QueryError`, never panic. |
| 6139 | `query_ids_panics_on_unknown_component` | adapt | Phase 5 | Dynamic specs validate owner/schema and return `QueryError`. |
| 6149 | `resource_changed_ticks_update` | adapt | Phase 3 | Keep added WorldTick plus monotonic revision without public manual tick injection. |
| 6169 | `resource_named_ticks_and_changed_flags_update` | reject | Phase 3 | Named resources and their name/type fallback identity are intentionally absent. |
| 6203 | `resource_scope_updates_and_marks_changed` | adapt | Phase 3 | Preserve alias-safe scoping and revision updates using a typed `Other` newtype, not a named resource. |
| 6225 | `resource_scope_returns_none_when_missing` | preserve | Phase 3 | Scoping an absent optional resource reports absence without mutation or aliasing. |
| 6232 | `register_and_run_system_updates_registry` | reject | Phase 4 | World has no dynamic system registry; systems are compiled into sibling Schedule state. |
| 6251 | `by_id_component_helpers_respect_entity_lifecycle` | preserve | Phase 3 | Checked dynamic component access rejects dead entities and preserves change tracking. |
| 6285 | `commands_run_system_executes_on_flush` | reject | Phase 4 | `RunSystem` is not a structural command and cannot re-enter or bypass compiled schedule order. |
| 6300 | `run_system_once_executes` | reject | Phase 4 | One-shot dynamic system execution is outside the compiled App lifecycle; hosts may call ordinary functions directly. |
| 6318 | `steady_state_update_is_allocation_free` | adapt | Phase 6 | Rebuild the composite budget around App, owner-scoped query state, and registered typed events. |
| 6385 | `steady_state_table_query_is_allocation_free` | adapt | Phase 6 | Retain the table-backed steady-state budget through the safe query implementation. |
| 6451 | `inactive_tag_sets_exclusion_and_accessor` | reject | Phase 5 | No reserved component name or World-global inactivity switch exists. |
| 6471 | `advance_tick_wraps_and_increments` | adapt | Phase 4 | App alone advances WorldTick from first value 1; exhaustion is terminal and never wraps. |
| 6481 | `apply_event_gating_reenables_when_disabled` | reject | Phase 4 | Public runtime gate toggling and reapplication are omitted from prepared execution. |
| 6493 | `world_render_runs_render_stage_systems` | adapt | Phase 4 | `App::render` runs the sibling Schedule's Render stage against World. |
| 6511 | `query_cached2_handles_registered_and_missing_components` | adapt | Phase 5 | Registered pairs are cached; an unregistered type returns `QueryError` rather than an empty result. |
| 6533 | `get_mut_by_id_comp_fast_updates_value` | adapt | Phase 3 | Keep checked dynamic mutation behind safe guards; do not expose an unchecked `fast` bypass. |
| 6550 | `steady_state_flush_reuses_command_buffer` | adapt | Phase 6 | Prove buffer reuse with valid structural commands, not the rejected `RunSystem` operation. |
