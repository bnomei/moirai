//! Parity ledger closure proofs for all 151 pd-asteroids characterization tests.
//! Generated from docs/parity.md — do not hand-edit; regenerate via scripts/generate_parity_ledger.py

struct ParityClosure {
    source_line: usize,
    source: &'static str,
    class: &'static str,
    owner: &'static str,
    moirai_proof: &'static str,
}

const PARITY_CLOSURES: &[ParityClosure] = &[
    ParityClosure { source_line: 112, source: "entity_id_roundtrip", class: "adapt", owner: "Phase 2", moirai_proof: "src/entity/id.rs::entity_id_carries_private_owner_and_packed_position" },
    ParityClosure { source_line: 120, source: "entity_id_from_u64_roundtrip", class: "adapt", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs::deterministic_initial_allocation_order" },
    ParityClosure { source_line: 128, source: "allocates_sequential_indices", class: "preserve", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs::deterministic_initial_allocation_order" },
    ParityClosure { source_line: 139, source: "free_reuses_index_with_bumped_generation", class: "preserve", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs::reuse_bumps_generation" },
    ParityClosure { source_line: 149, source: "free_rejects_stale_ids", class: "preserve", owner: "Phase 2", moirai_proof: "tests/hostile.rs::stale_entity_id_is_not_alive" },
    ParityClosure { source_line: 157, source: "is_alive_tracks_generation", class: "preserve", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs::freed_but_not_reallocated_is_dead + tests/hostile.rs::stale_entity_id_is_not_alive" },
    ParityClosure { source_line: 170, source: "allocator_default_marks_unknown_ids_dead", class: "preserve", owner: "Phase 2", moirai_proof: "tests/hostile.rs::freed_slot_is_not_alive" },
    ParityClosure { source_line: 215, source: "registry_returns_stable_ids", class: "adapt", owner: "Phase 2", moirai_proof: "tests/events.rs::duplicate_event_registration_requires_matching_options" },
    ParityClosure { source_line: 228, source: "registry_resolves_by_type", class: "preserve", owner: "Phase 2", moirai_proof: "tests/public_api.rs::phase_2_root_and_namespace_paths_compile" },
    ParityClosure { source_line: 237, source: "registry_resolves_by_name", class: "preserve", owner: "Phase 2", moirai_proof: "tests/parity_gaps.rs::register_tag_resolves_by_name" },
    ParityClosure { source_line: 247, source: "registry_tracks_storage_and_tags", class: "preserve", owner: "Phase 2", moirai_proof: "tests/query.rs::query_with_tag_filter" },
    ParityClosure { source_line: 256, source: "registry_tracks_storage_kind", class: "preserve", owner: "Phase 2", moirai_proof: "tests/hostile.rs::conflicting_registration_leaves_registry_unchanged" },
    ParityClosure { source_line: 201, source: "insert_get_remove", class: "preserve", owner: "Phase 2", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source_line: 215, source: "insert_replaces_existing", class: "preserve", owner: "Phase 2", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source_line: 224, source: "remove_swaps_dense_tail", class: "preserve", owner: "Phase 2", moirai_proof: "src/storage/sparse/tests.rs::swap_remove_repairs_reverse_indices" },
    ParityClosure { source_line: 240, source: "iterates_all_entities", class: "preserve", owner: "Phase 2", moirai_proof: "tests/query.rs::query1_returns_all_entities_with_component" },
    ParityClosure { source_line: 138, source: "bundle_collects_components_in_order", class: "adapt", owner: "Phase 3", moirai_proof: "tests/world_table_bundle.rs::spawn_bundle_with_table_components" },
    ParityClosure { source_line: 152, source: "commands_queue_and_take_ops", class: "adapt", owner: "Phase 3", moirai_proof: "tests/commands_deferred.rs::deferred_spawn_is_not_alive_until_flush" },
    ParityClosure { source_line: 208, source: "commands_restore_reuses_queue_buffer", class: "preserve", owner: "Phase 3", moirai_proof: "tests/allocation.rs::command_buffer_reuses_capacity_after_warmup" },
    ParityClosure { source_line: 220, source: "commands_drain_clears_queue", class: "preserve", owner: "Phase 3", moirai_proof: "tests/commands_deferred.rs::discard_releases_reserved_entities" },
    ParityClosure { source_line: 285, source: "insert_get_remove", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source_line: 296, source: "insert_replaces_existing", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source_line: 305, source: "get_mut_updates_resource", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::resource_mut_updates_value" },
    ParityClosure { source_line: 315, source: "tracks_added_and_changed_ticks", class: "adapt", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip + tests/parity_gaps.rs::resource_added_tick_updates" },
    ParityClosure { source_line: 328, source: "insert_named_replaces_by_name", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 342, source: "set_named_prefers_name_and_sets_type", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 353, source: "set_named_falls_back_to_type_and_sets_name", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 364, source: "get_mut_named_updates_ticks", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 376, source: "set_inserts_when_missing_and_updates_when_present", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source_line: 388, source: "take_and_restore_entry_roundtrip", class: "adapt", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_scope_mut_updates_value" },
    ParityClosure { source_line: 86, source: "new_initializes_state", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::in_state_gates_execution" },
    ParityClosure { source_line: 94, source: "set_updates_current_and_previous", class: "adapt", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::state_set_updates_current_and_previous" },
    ParityClosure { source_line: 102, source: "push_and_pop_manage_stack", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source_line: 117, source: "pop_without_stack_is_noop", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source_line: 126, source: "clear_stack_drops_saved_states", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source_line: 36, source: "new_initializes_step_and_accumulator", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::fixed_update_respects_accumulator_and_cap" },
    ParityClosure { source_line: 43, source: "default_initializes_zeroed_time", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::fixed_update_without_config_is_rejected_at_build" },
    ParityClosure { source_line: 239, source: "profiler_record_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::diagnostics_observer_steady_state_is_allocation_free" },
    ParityClosure { source_line: 546, source: "event_id_conversions_roundtrip", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 554, source: "initializes_empty", class: "preserve", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source_line: 563, source: "defaults_construct_empty_structs", class: "adapt", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source_line: 576, source: "sends_event_to_queue", class: "preserve", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source_line: 583, source: "queues_multiple_events_in_order", class: "preserve", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source_line: 600, source: "default_reader_id_is_shared", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 610, source: "separate_reader_ids_have_separate_cursors", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::separate_event_readers_advance_independently" },
    ParityClosure { source_line: 627, source: "pooled_events_reuse_payloads", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_pool_reuses_payload_after_warmup" },
    ParityClosure { source_line: 654, source: "compact_handles_empty_and_no_readers", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_compact_handles_empty_queue" },
    ParityClosure { source_line: 664, source: "compact_releases_only_consumed_events", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_compact_releases_consumed_payloads" },
    ParityClosure { source_line: 682, source: "compact_noops_when_reader_cursor_is_zero", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_compact_retains_unread_payloads" },
    ParityClosure { source_line: 697, source: "registry_returns_stable_ids", class: "adapt", owner: "Phase 3", moirai_proof: "tests/events.rs::duplicate_event_registration_requires_matching_options" },
    ParityClosure { source_line: 710, source: "registry_is_empty_tracks_entries", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_registry_tracks_entries" },
    ParityClosure { source_line: 718, source: "registry_debug_includes_type_name", class: "adapt", owner: "Phase 6", moirai_proof: "tests/parity_gaps.rs::registration_error_includes_component_name" },
    ParityClosure { source_line: 727, source: "gating_defaults_off", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 735, source: "gating_blocks_until_enabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 746, source: "gating_can_be_disabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 757, source: "pooled_gating_blocks_until_enabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 780, source: "enable_by_id_requires_gating_and_expands_enabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 794, source: "send_pooled_by_id_respects_gating_and_unknown_ids", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 813, source: "read_next_by_id_returns_none_for_missing_queue", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 821, source: "pooled_send_and_read_are_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_send_read_steady_state_is_allocation_free" },
    ParityClosure { source_line: 865, source: "event_queue_compact_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_compact_steady_state_is_allocation_free" },
    ParityClosure { source_line: 831, source: "validates_ordered_producer_consumer", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_validates_ordered_event_roles" },
    ParityClosure { source_line: 843, source: "rejects_missing_order_for_same_stage_event", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_rejects_missing_same_stage_event_order" },
    ParityClosure { source_line: 854, source: "rejects_missing_event_producer", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_rejects_missing_event_producer" },
    ParityClosure { source_line: 864, source: "ignores_component_event_missing_producer", class: "adapt", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_accepts_intrinsic_component_event_producer" },
    ParityClosure { source_line: 871, source: "ignores_cross_stage_event_order", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_accepts_ordered_cross_stage_event_roles" },
    ParityClosure { source_line: 879, source: "rejects_cross_stage_dependency_links", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::cross_stage_system_edge_is_rejected_at_build" },
    ParityClosure { source_line: 890, source: "rejects_missing_resources", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::missing_required_resource_is_rejected_at_build" },
    ParityClosure { source_line: 900, source: "rejects_duplicate_labels", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::duplicate_system_labels_are_rejected_at_build" },
    ParityClosure { source_line: 911, source: "startup_stage_runs_once", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::startup_runs_once_across_updates" },
    ParityClosure { source_line: 930, source: "stage_flush_updates_are_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::stage_flush_steady_state_is_allocation_free" },
    ParityClosure { source_line: 959, source: "fixed_update_steps_respect_accumulator", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::fixed_update_respects_accumulator_and_cap" },
    ParityClosure { source_line: 980, source: "flush_mode_stage_applies_commands_between_stages", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::flush_mode_stage_makes_commands_visible_between_stages" },
    ParityClosure { source_line: 1013, source: "flush_mode_end_defers_commands_until_update_end", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::flush_mode_final_defers_commands_until_update_end" },
    ParityClosure { source_line: 1046, source: "system_in_state_gates_execution", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::in_state_gates_execution" },
    ParityClosure { source_line: 1071, source: "system_state_changed_runs_on_transition", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::state_changed_runs_after_explicit_apply" },
    ParityClosure { source_line: 1098, source: "state_transition_update_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::state_transition_steady_state_is_allocation_free" },
    ParityClosure { source_line: 1128, source: "system_interval_buffers_time", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source_line: 1148, source: "system_pipe_passes_values_between_piped_systems", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 1187, source: "system_pipe_without_payload_is_allocation_free_after_warmup", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 1231, source: "system_run_if_gates_execution", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::run_if_skips_system_body" },
    ParityClosure { source_line: 1255, source: "set_run_if_gates_execution_once_per_stage", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::set_run_if_gates_all_members_once_per_stage" },
    ParityClosure { source_line: 1305, source: "rejects_unknown_set", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::unknown_system_set_is_rejected_at_build" },
    ParityClosure { source_line: 1315, source: "rejects_duplicate_set_labels", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::duplicate_set_labels_are_rejected_at_build" },
    ParityClosure { source_line: 1326, source: "set_and_system_conditions_are_both_required", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::set_and_system_conditions_compose_with_and_semantics" },
    ParityClosure { source_line: 1362, source: "run_if_and_set_are_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::run_if_and_set_steady_state_is_allocation_free" },
    ParityClosure { source_line: 4963, source: "query1_returns_all_entities_with_component", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query1_returns_all_entities_with_component" },
    ParityClosure { source_line: 4981, source: "query2_returns_intersection", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query2_returns_intersection" },
    ParityClosure { source_line: 5000, source: "table_component_insert_get_query", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::table_component_insert_get_query" },
    ParityClosure { source_line: 5022, source: "table_component_migration_preserves_ticks", class: "preserve", owner: "Phase 2", moirai_proof: "tests/world_lifecycle_state.rs::archetype_move_preserves_retained_component" },
    ParityClosure { source_line: 5084, source: "query2_mixed_table_and_sparse_components", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query2_mixed_table_and_sparse_components" },
    ParityClosure { source_line: 5114, source: "query_cached_reuses_plan", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::membership_policies_are_reusable_and_track_relevant_topology" },
    ParityClosure { source_line: 5125, source: "query_cached_results_updates_on_add_remove", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::membership_policies_are_reusable_and_track_relevant_topology" },
    ParityClosure { source_line: 5156, source: "query_cached_results_respects_without", class: "preserve", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query_cache_respects_without" },
    ParityClosure { source_line: 5203, source: "query_excludes_inactive_when_enabled", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::explicit_without_excludes_domain_marker_without_magic_name" },
    ParityClosure { source_line: 5236, source: "query_cached_results_respects_inactive_changes", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query_cache_respects_inactive_changes" },
    ParityClosure { source_line: 5282, source: "query_cached_results_clear_rebuilds", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_result_cache.rs::result_policy_is_reusable_and_refreshes_relevant_topology" },
    ParityClosure { source_line: 5312, source: "query_cached_results_enables_events_when_gated", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query_cache_survives_frame_event_clear" },
    ParityClosure { source_line: 5338, source: "apply_event_gating_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_dispatch_steady_state_is_allocation_free" },
    ParityClosure { source_line: 5359, source: "query_cached_results_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::query_result_cache_hit_is_allocation_free" },
    ParityClosure { source_line: 5391, source: "query_cached_results_rejects_added_or_changed", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_result_cache.rs::result_policy_rejects_moving_windows" },
    ParityClosure { source_line: 5406, source: "command_buffer_applies_on_flush", class: "adapt", owner: "Phase 3", moirai_proof: "tests/commands_deferred.rs::deferred_spawn_is_not_alive_until_flush" },
    ParityClosure { source_line: 5417, source: "command_buffer_flush_is_allocation_free_after_warmup", class: "preserve", owner: "Phase 6", moirai_proof: "tests/allocation.rs::command_flush_steady_state_is_allocation_free" },
    ParityClosure { source_line: 5446, source: "spawn_bundle_inserts_components_on_flush", class: "adapt", owner: "Phase 3", moirai_proof: "tests/world_table_bundle.rs::spawn_bundle_with_table_components" },
    ParityClosure { source_line: 5460, source: "bundle_insert_helpers_resolve_components", class: "adapt", owner: "Phase 3", moirai_proof: "tests/world_table_bundle.rs::spawn_bundle_with_table_components" },
    ParityClosure { source_line: 5474, source: "bundle_insert_helpers_return_false_when_component_missing", class: "adapt", owner: "Phase 3", moirai_proof: "tests/phase3_failure_contracts.rs::deferred_spawn_bundle_rolls_back_on_bundle_error" },
    ParityClosure { source_line: 5490, source: "command_buffer_despawn_removes_components", class: "adapt", owner: "Phase 3", moirai_proof: "tests/query.rs::query_skips_despawned_entities" },
    ParityClosure { source_line: 5504, source: "query_respects_without_list", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_respects_without_list" },
    ParityClosure { source_line: 5530, source: "event_gating_from_schedule_controls_dispatch", class: "adapt", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_event_roles_control_dispatch" },
    ParityClosure { source_line: 5545, source: "read_event_typed_downcasts_payload", class: "adapt", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source_line: 5569, source: "read_event_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_send_read_steady_state_is_allocation_free" },
    ParityClosure { source_line: 5595, source: "frame_events_are_dropped_after_update", class: "adapt", owner: "Phase 4", moirai_proof: "tests/events.rs::update_and_render_boundaries_clear_only_their_owned_frame_channels" },
    ParityClosure { source_line: 5604, source: "persistent_events_survive_update_when_unread", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::persistent_events_survive_update_until_read" },
    ParityClosure { source_line: 5617, source: "validate_schedule_via_world", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::missing_required_resource_is_rejected_at_build" },
    ParityClosure { source_line: 5628, source: "query_added_filters_by_tick", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_added_filters_by_tick" },
    ParityClosure { source_line: 5662, source: "query_changed_filters_by_tick", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_changed_filters_by_tick" },
    ParityClosure { source_line: 5700, source: "get_mut_marks_changed_tick", class: "preserve", owner: "Phase 3", moirai_proof: "tests/query.rs::query_changed_filters_by_tick" },
    ParityClosure { source_line: 5739, source: "query_cached_params_updates_last_tick", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::membership_applies_since_and_cursor_windows_at_execution" },
    ParityClosure { source_line: 5781, source: "query_cache_is_owner_scoped", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::membership_policies_are_owner_scoped" },
    ParityClosure { source_line: 5826, source: "register_component_records_metadata_and_events", class: "adapt", owner: "Phase 3", moirai_proof: "tests/component_lifecycle.rs::component_added_emitted_after_commit" },
    ParityClosure { source_line: 5842, source: "register_component_untyped_requires_tag", class: "adapt", owner: "Phase 2", moirai_proof: "src/component/registry/tests.rs::untyped_registration_requires_tag_options + tests/parity_gaps.rs::register_untyped_requires_tag" },
    ParityClosure { source_line: 5848, source: "component_events_emitted_on_insert_and_remove", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::component_removed_emitted_on_despawn" },
    ParityClosure { source_line: 5870, source: "component_add_not_emitted_on_replace", class: "preserve", owner: "Phase 3", moirai_proof: "tests/component_lifecycle.rs::replacement_does_not_emit_second_add" },
    ParityClosure { source_line: 5885, source: "component_events_enabled_on_read_when_gated", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::component_events_readable_after_registration" },
    ParityClosure { source_line: 5899, source: "component_event_dispatch_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::component_event_dispatch_is_allocation_free" },
    ParityClosure { source_line: 5948, source: "read_component_event_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::component_event_read_is_allocation_free" },
    ParityClosure { source_line: 5974, source: "query_ids_filters_with_and_without", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_respects_without_list" },
    ParityClosure { source_line: 6001, source: "query_ids_excludes_inactive_when_enabled", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::explicit_without_excludes_domain_marker_without_magic_name" },
    ParityClosure { source_line: 6040, source: "query_ids_cached_last_tick_updates_on_exhaust", class: "preserve", owner: "Phase 5", moirai_proof: "src/world/query/entities.rs::internal_entity_cursor_commits_only_after_full_iteration" },
    ParityClosure { source_line: 6070, source: "query_ids_cached_last_tick_skips_on_partial_iteration", class: "preserve", owner: "Phase 5", moirai_proof: "src/world/query/entities.rs::internal_entity_cursor_commits_only_after_full_iteration" },
    ParityClosure { source_line: 6098, source: "query_spec_from_names_reports_missing_components", class: "reject", owner: "Phase 5", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6107, source: "query_spec_from_names_builds_expected_spec", class: "reject", owner: "Phase 5", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6125, source: "query1_params_panics_on_unknown_component", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::query_unregistered_component_returns_error" },
    ParityClosure { source_line: 6139, source: "query_ids_panics_on_unknown_component", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::query_unregistered_component_returns_error" },
    ParityClosure { source_line: 6149, source: "resource_changed_ticks_update", class: "adapt", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source_line: 6169, source: "resource_named_ticks_and_changed_flags_update", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6203, source: "resource_scope_updates_and_marks_changed", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::resource_scope_mut_marks_changed" },
    ParityClosure { source_line: 6225, source: "resource_scope_returns_none_when_missing", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_scope_ref_reports_missing_without_mutation" },
    ParityClosure { source_line: 6232, source: "register_and_run_system_updates_registry", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6251, source: "by_id_component_helpers_respect_entity_lifecycle", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::dynamic_component_access_respects_lifecycle" },
    ParityClosure { source_line: 6285, source: "commands_run_system_executes_on_flush", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6300, source: "run_system_once_executes", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6318, source: "steady_state_update_is_allocation_free", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::app_update_steady_state_is_allocation_free" },
    ParityClosure { source_line: 6385, source: "steady_state_table_query_is_allocation_free", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::table_query_steady_state_is_allocation_free" },
    ParityClosure { source_line: 6451, source: "inactive_tag_sets_exclusion_and_accessor", class: "reject", owner: "Phase 5", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6471, source: "advance_tick_wraps_and_increments", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::app_runs_update_system_in_order + src/world/mod.rs::change_tick_exhaustion_poison_world_mutations" },
    ParityClosure { source_line: 6481, source: "apply_event_gating_reenables_when_disabled", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source_line: 6493, source: "world_render_runs_render_stage_systems", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::render_rejects_structural_commands_in_system" },
    ParityClosure { source_line: 6511, source: "query_cached2_handles_registered_and_missing_components", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query2_result_cache_handles_registered_and_missing" },
    ParityClosure { source_line: 6533, source: "get_mut_by_id_comp_fast_updates_value", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::dynamic_component_mut_updates_value" },
    ParityClosure { source_line: 6550, source: "steady_state_flush_reuses_command_buffer", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::command_buffer_reuses_capacity_after_warmup" },
];

fn cited_functions(proof: &str) -> impl Iterator<Item = (&str, &str)> {
    proof.split(" + ").map(|citation| {
        citation
            .rsplit_once("::")
            .unwrap_or_else(|| panic!("non-reject proof must use path::symbol syntax: {citation}"))
    })
}

fn function_definition_count(source: &str, symbol: &str) -> usize {
    let needle = format!("fn {symbol}");
    source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .find(&needle)
                .map(|index| &line.trim_start()[index + needle.len()..])
        })
        .filter(|tail| tail.starts_with('(') || tail.starts_with('<'))
        .count()
}

#[test]
fn parity_ledger_accounts_for_all_source_rows() {
    assert_eq!(PARITY_CLOSURES.len(), 151);
    let mut keys = PARITY_CLOSURES
        .iter()
        .map(|row| (row.source_line, row.source))
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 151, "duplicate source line/name key in ledger");
}

#[test]
fn parity_ledger_reject_rows_document_negative_contract() {
    for row in PARITY_CLOSURES {
        if row.class == "reject" {
            assert!(
                row.moirai_proof.starts_with("docs/parity.md reject-doc"),
                "reject row {}/{} must cite reject-doc proof",
                row.source_line,
                row.source
            );
        }
    }
}

#[test]
fn parity_ledger_citations_resolve_to_one_test_function() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in PARITY_CLOSURES {
        if row.class == "reject" {
            assert!(root.join("docs/parity.md").is_file());
            continue;
        }
        for (path, symbol) in cited_functions(row.moirai_proof) {
            let source = std::fs::read_to_string(root.join(path)).unwrap_or_else(|error| {
                panic!(
                    "proof path {path} for {}/{} is unreadable: {error}",
                    row.source_line, row.source
                )
            });
            assert_eq!(
                function_definition_count(&source, symbol),
                1,
                "proof {path}::{symbol} for {}/{} must resolve exactly once",
                row.source_line,
                row.source
            );
        }
    }
}

#[test]
fn parity_ledger_phase6_rows_map_to_allocation_or_diagnostics_tests() {
    for row in PARITY_CLOSURES {
        if row.owner == "Phase 6" {
            assert!(
                row.moirai_proof.contains("tests/allocation.rs")
                    || row.moirai_proof.contains("tests/parity_gaps.rs")
                    || row.moirai_proof.contains("tests/schedule_observer.rs"),
                "Phase 6 row {} must map to allocation/diagnostics proof: {}",
                row.source,
                row.moirai_proof
            );
        }
    }
}
