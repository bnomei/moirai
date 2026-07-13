//! Parity ledger closure proofs for all 151 pd-asteroids characterization tests.
//! Generated from docs/parity.md — do not hand-edit; regenerate via scripts/generate_parity_ledger.py

struct ParityClosure {
    source: &'static str,
    class: &'static str,
    owner: &'static str,
    moirai_proof: &'static str,
}

const PARITY_CLOSURES: &[ParityClosure] = &[
    ParityClosure { source: "entity_id_roundtrip", class: "adapt", owner: "Phase 2", moirai_proof: "src/entity/id.rs + allocator/tests (private layout)" },
    ParityClosure { source: "entity_id_from_u64_roundtrip", class: "adapt", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs (private packing)" },
    ParityClosure { source: "allocates_sequential_indices", class: "preserve", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs::deterministic_initial_allocation_order" },
    ParityClosure { source: "free_reuses_index_with_bumped_generation", class: "preserve", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs::reuse_bumps_generation" },
    ParityClosure { source: "free_rejects_stale_ids", class: "preserve", owner: "Phase 2", moirai_proof: "tests/hostile.rs::stale_entity_id_is_not_alive" },
    ParityClosure { source: "is_alive_tracks_generation", class: "preserve", owner: "Phase 2", moirai_proof: "src/entity/allocator/tests.rs + tests/hostile.rs" },
    ParityClosure { source: "allocator_default_marks_unknown_ids_dead", class: "preserve", owner: "Phase 2", moirai_proof: "tests/hostile.rs::freed_slot_is_not_alive" },
    ParityClosure { source: "registry_returns_stable_ids", class: "adapt", owner: "Phase 2", moirai_proof: "tests/events.rs::duplicate_event_registration_requires_matching_options" },
    ParityClosure { source: "registry_resolves_by_type", class: "preserve", owner: "Phase 2", moirai_proof: "tests/public_api.rs + WorldBuilder registration" },
    ParityClosure { source: "registry_resolves_by_name", class: "preserve", owner: "Phase 2", moirai_proof: "tests/parity_gaps.rs::register_tag_resolves_by_name" },
    ParityClosure { source: "registry_tracks_storage_and_tags", class: "preserve", owner: "Phase 2", moirai_proof: "tests/query.rs::query_with_tag_filter" },
    ParityClosure { source: "registry_tracks_storage_kind", class: "preserve", owner: "Phase 2", moirai_proof: "tests/hostile.rs::conflicting_registration_leaves_registry_unchanged" },
    ParityClosure { source: "insert_get_remove", class: "preserve", owner: "Phase 2", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source: "insert_replaces_existing", class: "preserve", owner: "Phase 2", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source: "remove_swaps_dense_tail", class: "preserve", owner: "Phase 2", moirai_proof: "src/storage/sparse/tests.rs::swap_remove_repairs_reverse_indices" },
    ParityClosure { source: "iterates_all_entities", class: "preserve", owner: "Phase 2", moirai_proof: "tests/query.rs::query1_returns_all_entities_with_component" },
    ParityClosure { source: "bundle_collects_components_in_order", class: "adapt", owner: "Phase 3", moirai_proof: "tests/world_table_bundle.rs::spawn_bundle_with_table_components" },
    ParityClosure { source: "commands_queue_and_take_ops", class: "adapt", owner: "Phase 3", moirai_proof: "tests/commands_deferred.rs" },
    ParityClosure { source: "commands_restore_reuses_queue_buffer", class: "preserve", owner: "Phase 3", moirai_proof: "tests/allocation.rs::command_buffer_reuses_capacity_after_warmup" },
    ParityClosure { source: "commands_drain_clears_queue", class: "preserve", owner: "Phase 3", moirai_proof: "tests/commands_deferred.rs::discard_releases_reserved_entities" },
    ParityClosure { source: "insert_get_remove", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source: "insert_replaces_existing", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source: "get_mut_updates_resource", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::resource_mut_updates_value" },
    ParityClosure { source: "tracks_added_and_changed_ticks", class: "adapt", owner: "Phase 3", moirai_proof: "tests/resources.rs + tests/parity_gaps.rs::resource_added_tick_updates" },
    ParityClosure { source: "insert_named_replaces_by_name", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "set_named_prefers_name_and_sets_type", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "set_named_falls_back_to_type_and_sets_name", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "get_mut_named_updates_ticks", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "set_inserts_when_missing_and_updates_when_present", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source: "take_and_restore_entry_roundtrip", class: "adapt", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_scope_*" },
    ParityClosure { source: "new_initializes_state", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::in_state_gates_execution" },
    ParityClosure { source: "set_updates_current_and_previous", class: "adapt", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::state_set_updates_current_and_previous" },
    ParityClosure { source: "push_and_pop_manage_stack", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source: "pop_without_stack_is_noop", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source: "clear_stack_drops_saved_states", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source: "new_initializes_step_and_accumulator", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::fixed_update_respects_accumulator_and_cap" },
    ParityClosure { source: "default_initializes_zeroed_time", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::fixed_update_without_config_is_rejected_at_build" },
    ParityClosure { source: "profiler_record_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::diagnostics_observer_steady_state_is_allocation_free" },
    ParityClosure { source: "event_id_conversions_roundtrip", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "initializes_empty", class: "preserve", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source: "defaults_construct_empty_structs", class: "adapt", owner: "Phase 3", moirai_proof: "tests/events.rs" },
    ParityClosure { source: "sends_event_to_queue", class: "preserve", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source: "queues_multiple_events_in_order", class: "preserve", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source: "default_reader_id_is_shared", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "separate_reader_ids_have_separate_cursors", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::separate_event_readers_advance_independently" },
    ParityClosure { source: "pooled_events_reuse_payloads", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_pool_reuses_payload_after_warmup" },
    ParityClosure { source: "compact_handles_empty_and_no_readers", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_compact_handles_empty_queue" },
    ParityClosure { source: "compact_releases_only_consumed_events", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_compact_releases_consumed_payloads" },
    ParityClosure { source: "compact_noops_when_reader_cursor_is_zero", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_compact_retains_unread_payloads" },
    ParityClosure { source: "registry_returns_stable_ids", class: "adapt", owner: "Phase 3", moirai_proof: "tests/events.rs::duplicate_event_registration_requires_matching_options" },
    ParityClosure { source: "registry_is_empty_tracks_entries", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::event_registry_tracks_entries" },
    ParityClosure { source: "registry_debug_includes_type_name", class: "adapt", owner: "Phase 6", moirai_proof: "tests/parity_gaps.rs::registration_error_includes_component_name" },
    ParityClosure { source: "gating_defaults_off", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "gating_blocks_until_enabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "gating_can_be_disabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "pooled_gating_blocks_until_enabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "enable_by_id_requires_gating_and_expands_enabled", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "send_pooled_by_id_respects_gating_and_unknown_ids", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "read_next_by_id_returns_none_for_missing_queue", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "pooled_send_and_read_are_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_send_read_steady_state_is_allocation_free" },
    ParityClosure { source: "event_queue_compact_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_compact_steady_state_is_allocation_free" },
    ParityClosure { source: "validates_ordered_producer_consumer", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_validates_ordered_event_roles" },
    ParityClosure { source: "rejects_missing_order_for_same_stage_event", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_rejects_missing_same_stage_event_order" },
    ParityClosure { source: "rejects_missing_event_producer", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_rejects_missing_event_producer" },
    ParityClosure { source: "ignores_component_event_missing_producer", class: "adapt", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_ignores_component_event_missing_producer" },
    ParityClosure { source: "ignores_cross_stage_event_order", class: "preserve", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_ignores_cross_stage_event_order" },
    ParityClosure { source: "rejects_cross_stage_dependency_links", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::cross_stage_system_edge_is_rejected_at_build" },
    ParityClosure { source: "rejects_missing_resources", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::missing_required_resource_is_rejected_at_build" },
    ParityClosure { source: "rejects_duplicate_labels", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::duplicate_system_labels_are_rejected_at_build" },
    ParityClosure { source: "startup_stage_runs_once", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::startup_runs_once_across_updates" },
    ParityClosure { source: "stage_flush_updates_are_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::stage_flush_steady_state_is_allocation_free" },
    ParityClosure { source: "fixed_update_steps_respect_accumulator", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::fixed_update_respects_accumulator_and_cap" },
    ParityClosure { source: "flush_mode_stage_applies_commands_between_stages", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::flush_mode_stage_makes_commands_visible_between_stages" },
    ParityClosure { source: "flush_mode_end_defers_commands_until_update_end", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::flush_mode_final_defers_commands_until_update_end" },
    ParityClosure { source: "system_in_state_gates_execution", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::in_state_gates_execution" },
    ParityClosure { source: "system_state_changed_runs_on_transition", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::state_changed_runs_after_explicit_apply" },
    ParityClosure { source: "state_transition_update_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::state_transition_steady_state_is_allocation_free" },
    ParityClosure { source: "system_interval_buffers_time", class: "reject", owner: "Downstream host", moirai_proof: "docs/parity.md reject-doc downstream-host" },
    ParityClosure { source: "system_pipe_passes_values_between_piped_systems", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "system_pipe_without_payload_is_allocation_free_after_warmup", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "system_run_if_gates_execution", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::run_if_skips_system_body" },
    ParityClosure { source: "set_run_if_gates_execution_once_per_stage", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::set_run_if_gates_all_members_once_per_stage" },
    ParityClosure { source: "rejects_unknown_set", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::unknown_system_set_is_rejected_at_build" },
    ParityClosure { source: "rejects_duplicate_set_labels", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::duplicate_set_labels_are_rejected_at_build" },
    ParityClosure { source: "set_and_system_conditions_are_both_required", class: "preserve", owner: "Phase 4", moirai_proof: "tests/schedule.rs::set_and_system_conditions_compose_with_and_semantics" },
    ParityClosure { source: "run_if_and_set_are_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::run_if_and_set_steady_state_is_allocation_free" },
    ParityClosure { source: "query1_returns_all_entities_with_component", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query1_returns_all_entities_with_component" },
    ParityClosure { source: "query2_returns_intersection", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query2_returns_intersection" },
    ParityClosure { source: "table_component_insert_get_query", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::table_component_insert_get_query" },
    ParityClosure { source: "table_component_migration_preserves_ticks", class: "preserve", owner: "Phase 2", moirai_proof: "tests/world_lifecycle_state.rs::archetype_move_preserves_retained_component" },
    ParityClosure { source: "query2_mixed_table_and_sparse_components", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query2_mixed_table_and_sparse_components" },
    ParityClosure { source: "query_cached_reuses_plan", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::query_cache_cold_and_hot_hit" },
    ParityClosure { source: "query_cached_results_updates_on_add_remove", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::query_cache_updates_on_spawn" },
    ParityClosure { source: "query_cached_results_respects_without", class: "preserve", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query_cache_respects_without" },
    ParityClosure { source: "query_excludes_inactive_when_enabled", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::explicit_without_excludes_domain_marker_without_magic_name" },
    ParityClosure { source: "query_cached_results_respects_inactive_changes", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query_cache_respects_inactive_changes" },
    ParityClosure { source: "query_cached_results_clear_rebuilds", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_result_cache.rs::result_cache_invalidate_and_rebuild" },
    ParityClosure { source: "query_cached_results_enables_events_when_gated", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query_cache_survives_frame_event_clear" },
    ParityClosure { source: "apply_event_gating_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_dispatch_steady_state_is_allocation_free" },
    ParityClosure { source: "query_cached_results_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::query_result_cache_hit_is_allocation_free" },
    ParityClosure { source: "query_cached_results_rejects_added_or_changed", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::query_result_cache_rejects_added_or_changed" },
    ParityClosure { source: "command_buffer_applies_on_flush", class: "adapt", owner: "Phase 3", moirai_proof: "tests/commands_deferred.rs::deferred_spawn_is_not_alive_until_flush" },
    ParityClosure { source: "command_buffer_flush_is_allocation_free_after_warmup", class: "preserve", owner: "Phase 6", moirai_proof: "tests/allocation.rs::command_flush_steady_state_is_allocation_free" },
    ParityClosure { source: "spawn_bundle_inserts_components_on_flush", class: "adapt", owner: "Phase 3", moirai_proof: "tests/world_table_bundle.rs::spawn_bundle_with_table_components" },
    ParityClosure { source: "bundle_insert_helpers_resolve_components", class: "adapt", owner: "Phase 3", moirai_proof: "tests/world_table_bundle.rs::spawn_bundle_with_table_components" },
    ParityClosure { source: "bundle_insert_helpers_return_false_when_component_missing", class: "adapt", owner: "Phase 3", moirai_proof: "tests/phase3_failure_contracts.rs::deferred_spawn_bundle_rolls_back_on_bundle_error" },
    ParityClosure { source: "command_buffer_despawn_removes_components", class: "adapt", owner: "Phase 3", moirai_proof: "tests/query.rs::query_skips_despawned_entities" },
    ParityClosure { source: "query_respects_without_list", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_respects_without_list" },
    ParityClosure { source: "event_gating_from_schedule_controls_dispatch", class: "adapt", owner: "Phase 4", moirai_proof: "tests/parity_gaps.rs::schedule_event_roles_control_dispatch" },
    ParityClosure { source: "read_event_typed_downcasts_payload", class: "adapt", owner: "Phase 3", moirai_proof: "tests/events.rs::events_send_and_read_in_order" },
    ParityClosure { source: "read_event_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::event_send_read_steady_state_is_allocation_free" },
    ParityClosure { source: "frame_events_are_dropped_after_update", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::frame_events_clear_per_operation_boundary" },
    ParityClosure { source: "persistent_events_survive_update_when_unread", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::persistent_events_survive_update_until_read" },
    ParityClosure { source: "validate_schedule_via_world", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::missing_required_resource_is_rejected_at_build" },
    ParityClosure { source: "query_added_filters_by_tick", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_added_filters_by_tick" },
    ParityClosure { source: "query_changed_filters_by_tick", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_changed_filters_by_tick" },
    ParityClosure { source: "get_mut_marks_changed_tick", class: "preserve", owner: "Phase 3", moirai_proof: "tests/query.rs::query_changed_filters_by_tick" },
    ParityClosure { source: "query_cached_params_updates_last_tick", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::membership_cache_stores_structural_members_for_added_queries" },
    ParityClosure { source: "query_cache_is_owner_scoped", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query_cache.rs::query_cache_is_owner_scoped" },
    ParityClosure { source: "register_component_records_metadata_and_events", class: "adapt", owner: "Phase 3", moirai_proof: "tests/component_lifecycle.rs::component_added_emitted_after_commit" },
    ParityClosure { source: "register_component_untyped_requires_tag", class: "adapt", owner: "Phase 2", moirai_proof: "src/component/registry/tests.rs::untyped_registration_requires_tag_options + tests/parity_gaps.rs::register_untyped_requires_tag" },
    ParityClosure { source: "component_events_emitted_on_insert_and_remove", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::component_removed_emitted_on_despawn" },
    ParityClosure { source: "component_add_not_emitted_on_replace", class: "preserve", owner: "Phase 3", moirai_proof: "tests/component_lifecycle.rs::replacement_does_not_emit_second_add" },
    ParityClosure { source: "component_events_enabled_on_read_when_gated", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::component_events_readable_after_registration" },
    ParityClosure { source: "component_event_dispatch_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::component_event_dispatch_is_allocation_free" },
    ParityClosure { source: "read_component_event_is_allocation_free_after_warmup", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::component_event_read_is_allocation_free" },
    ParityClosure { source: "query_ids_filters_with_and_without", class: "preserve", owner: "Phase 5", moirai_proof: "tests/query.rs::query_respects_without_list" },
    ParityClosure { source: "query_ids_excludes_inactive_when_enabled", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::explicit_without_excludes_domain_marker_without_magic_name" },
    ParityClosure { source: "query_ids_cached_last_tick_updates_on_exhaust", class: "preserve", owner: "Phase 5", moirai_proof: "src/query/cursor.rs::query_cursor_commits_on_exhaustion" },
    ParityClosure { source: "query_ids_cached_last_tick_skips_on_partial_iteration", class: "preserve", owner: "Phase 5", moirai_proof: "src/query/cursor.rs::query_cursor_skips_commit_on_partial_iteration" },
    ParityClosure { source: "query_spec_from_names_reports_missing_components", class: "preserve", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::dynamic_bundle_reports_missing_component" },
    ParityClosure { source: "query_spec_from_names_builds_expected_spec", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::dynamic_bundle_resolves_registered_components" },
    ParityClosure { source: "query1_params_panics_on_unknown_component", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::query_unregistered_component_returns_error" },
    ParityClosure { source: "query_ids_panics_on_unknown_component", class: "adapt", owner: "Phase 5", moirai_proof: "tests/query.rs::query_unregistered_component_returns_error" },
    ParityClosure { source: "resource_changed_ticks_update", class: "adapt", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_insert_get_remove_round_trip" },
    ParityClosure { source: "resource_named_ticks_and_changed_flags_update", class: "reject", owner: "Phase 3", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "resource_scope_updates_and_marks_changed", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::resource_scope_marks_changed" },
    ParityClosure { source: "resource_scope_returns_none_when_missing", class: "preserve", owner: "Phase 3", moirai_proof: "tests/resources.rs::resource_scope_reports_missing_without_mutation" },
    ParityClosure { source: "register_and_run_system_updates_registry", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "by_id_component_helpers_respect_entity_lifecycle", class: "preserve", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::dynamic_component_access_respects_lifecycle" },
    ParityClosure { source: "commands_run_system_executes_on_flush", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "run_system_once_executes", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "steady_state_update_is_allocation_free", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::app_update_steady_state_is_allocation_free" },
    ParityClosure { source: "steady_state_table_query_is_allocation_free", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::table_query_steady_state_is_allocation_free" },
    ParityClosure { source: "inactive_tag_sets_exclusion_and_accessor", class: "reject", owner: "Phase 5", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "advance_tick_wraps_and_increments", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::app_runs_update_system_in_order + src/world/mod.rs::change_tick_exhaustion_poison_world_mutations" },
    ParityClosure { source: "apply_event_gating_reenables_when_disabled", class: "reject", owner: "Phase 4", moirai_proof: "docs/parity.md reject-doc" },
    ParityClosure { source: "world_render_runs_render_stage_systems", class: "adapt", owner: "Phase 4", moirai_proof: "tests/schedule.rs::render_rejects_structural_commands_in_system" },
    ParityClosure { source: "query_cached2_handles_registered_and_missing_components", class: "adapt", owner: "Phase 5", moirai_proof: "tests/parity_gaps.rs::query2_result_cache_handles_registered_and_missing" },
    ParityClosure { source: "get_mut_by_id_comp_fast_updates_value", class: "adapt", owner: "Phase 3", moirai_proof: "tests/parity_gaps.rs::dynamic_component_mut_updates_value" },
    ParityClosure { source: "steady_state_flush_reuses_command_buffer", class: "adapt", owner: "Phase 6", moirai_proof: "tests/allocation.rs::command_buffer_reuses_capacity_after_warmup" },
];

#[test]
fn parity_ledger_accounts_for_all_source_tests() {
    assert_eq!(PARITY_CLOSURES.len(), 151);
    let mut names = PARITY_CLOSURES
        .iter()
        .map(|row| row.source)
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    assert!(
        names.len() < PARITY_CLOSURES.len(),
        "ledger should include cross-phase duplicate source names"
    );
}

#[test]
fn parity_ledger_reject_rows_document_negative_contract() {
    for row in PARITY_CLOSURES {
        if row.class == "reject" {
            assert!(
                row.moirai_proof.contains("reject-doc"),
                "reject row {} must cite reject-doc proof",
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
