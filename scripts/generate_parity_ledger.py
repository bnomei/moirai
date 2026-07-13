#!/usr/bin/env python3
"""Regenerate and validate ``tests/parity_ledger.rs`` from ``docs/parity.md``."""

from __future__ import annotations

import argparse
import difflib
import re
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PARITY = ROOT / "docs" / "parity.md"
OUT = ROOT / "tests" / "parity_ledger.rs"

# Canonical closure evidence, keyed by the source ledger line and source test name.
# The line key deliberately permits the source suite's cross-phase duplicate names
# while making accidental row insertion, deletion, or reordering fail loudly.
PROOF_ROWS = r"""112|entity_id_roundtrip|src/entity/id.rs::entity_id_carries_private_owner_and_packed_position
120|entity_id_from_u64_roundtrip|src/entity/allocator/tests.rs::deterministic_initial_allocation_order
128|allocates_sequential_indices|src/entity/allocator/tests.rs::deterministic_initial_allocation_order
139|free_reuses_index_with_bumped_generation|src/entity/allocator/tests.rs::reuse_bumps_generation
149|free_rejects_stale_ids|tests/hostile.rs::stale_entity_id_is_not_alive
157|is_alive_tracks_generation|src/entity/allocator/tests.rs::freed_but_not_reallocated_is_dead + tests/hostile.rs::stale_entity_id_is_not_alive
170|allocator_default_marks_unknown_ids_dead|tests/hostile.rs::freed_slot_is_not_alive
215|registry_returns_stable_ids|tests/events.rs::duplicate_event_registration_requires_matching_options
228|registry_resolves_by_type|tests/public_api.rs::phase_2_root_and_namespace_paths_compile
237|registry_resolves_by_name|tests/parity_gaps.rs::register_tag_resolves_by_name
247|registry_tracks_storage_and_tags|tests/query.rs::query_with_tag_filter
256|registry_tracks_storage_kind|tests/hostile.rs::conflicting_registration_leaves_registry_unchanged
201|insert_get_remove|tests/resources.rs::resource_insert_get_remove_round_trip
215|insert_replaces_existing|tests/resources.rs::resource_insert_get_remove_round_trip
224|remove_swaps_dense_tail|src/storage/sparse/tests.rs::swap_remove_repairs_reverse_indices
240|iterates_all_entities|tests/query.rs::query1_returns_all_entities_with_component
138|bundle_collects_components_in_order|tests/world_table_bundle.rs::spawn_bundle_with_table_components
152|commands_queue_and_take_ops|tests/commands_deferred.rs::deferred_spawn_is_not_alive_until_flush
208|commands_restore_reuses_queue_buffer|tests/allocation.rs::command_buffer_reuses_capacity_after_warmup
220|commands_drain_clears_queue|tests/commands_deferred.rs::discard_releases_reserved_entities
285|insert_get_remove|tests/resources.rs::resource_insert_get_remove_round_trip
296|insert_replaces_existing|tests/resources.rs::resource_insert_get_remove_round_trip
305|get_mut_updates_resource|tests/parity_gaps.rs::resource_mut_updates_value
315|tracks_added_and_changed_ticks|tests/resources.rs::resource_insert_get_remove_round_trip + tests/parity_gaps.rs::resource_added_tick_updates
328|insert_named_replaces_by_name|docs/parity.md reject-doc
342|set_named_prefers_name_and_sets_type|docs/parity.md reject-doc
353|set_named_falls_back_to_type_and_sets_name|docs/parity.md reject-doc
364|get_mut_named_updates_ticks|docs/parity.md reject-doc
376|set_inserts_when_missing_and_updates_when_present|tests/resources.rs::resource_insert_get_remove_round_trip
388|take_and_restore_entry_roundtrip|tests/resources.rs::resource_scope_updates_value
86|new_initializes_state|tests/schedule.rs::in_state_gates_execution
94|set_updates_current_and_previous|tests/parity_gaps.rs::state_set_updates_current_and_previous
102|push_and_pop_manage_stack|docs/parity.md reject-doc downstream-host
117|pop_without_stack_is_noop|docs/parity.md reject-doc downstream-host
126|clear_stack_drops_saved_states|docs/parity.md reject-doc downstream-host
36|new_initializes_step_and_accumulator|tests/schedule.rs::fixed_update_respects_accumulator_and_cap
43|default_initializes_zeroed_time|tests/schedule.rs::fixed_update_without_config_is_rejected_at_build
239|profiler_record_is_allocation_free_after_warmup|tests/allocation.rs::diagnostics_observer_steady_state_is_allocation_free
546|event_id_conversions_roundtrip|docs/parity.md reject-doc
554|initializes_empty|tests/events.rs::events_send_and_read_in_order
563|defaults_construct_empty_structs|tests/events.rs::events_send_and_read_in_order
576|sends_event_to_queue|tests/events.rs::events_send_and_read_in_order
583|queues_multiple_events_in_order|tests/events.rs::events_send_and_read_in_order
600|default_reader_id_is_shared|docs/parity.md reject-doc
610|separate_reader_ids_have_separate_cursors|tests/parity_gaps.rs::separate_event_readers_advance_independently
627|pooled_events_reuse_payloads|tests/allocation.rs::event_pool_reuses_payload_after_warmup
654|compact_handles_empty_and_no_readers|tests/parity_gaps.rs::event_compact_handles_empty_queue
664|compact_releases_only_consumed_events|tests/parity_gaps.rs::event_compact_releases_consumed_payloads
682|compact_noops_when_reader_cursor_is_zero|tests/parity_gaps.rs::event_compact_retains_unread_payloads
697|registry_returns_stable_ids|tests/events.rs::duplicate_event_registration_requires_matching_options
710|registry_is_empty_tracks_entries|tests/parity_gaps.rs::event_registry_tracks_entries
718|registry_debug_includes_type_name|tests/parity_gaps.rs::registration_error_includes_component_name
727|gating_defaults_off|docs/parity.md reject-doc
735|gating_blocks_until_enabled|docs/parity.md reject-doc
746|gating_can_be_disabled|docs/parity.md reject-doc
757|pooled_gating_blocks_until_enabled|docs/parity.md reject-doc
780|enable_by_id_requires_gating_and_expands_enabled|docs/parity.md reject-doc
794|send_pooled_by_id_respects_gating_and_unknown_ids|docs/parity.md reject-doc
813|read_next_by_id_returns_none_for_missing_queue|docs/parity.md reject-doc
821|pooled_send_and_read_are_allocation_free_after_warmup|tests/allocation.rs::event_send_read_steady_state_is_allocation_free
865|event_queue_compact_is_allocation_free_after_warmup|tests/allocation.rs::event_compact_steady_state_is_allocation_free
831|validates_ordered_producer_consumer|tests/parity_gaps.rs::schedule_validates_ordered_event_roles
843|rejects_missing_order_for_same_stage_event|tests/parity_gaps.rs::schedule_rejects_missing_same_stage_event_order
854|rejects_missing_event_producer|tests/parity_gaps.rs::schedule_rejects_missing_event_producer
864|ignores_component_event_missing_producer|tests/parity_gaps.rs::schedule_accepts_intrinsic_component_event_producer
871|ignores_cross_stage_event_order|tests/parity_gaps.rs::schedule_accepts_ordered_cross_stage_event_roles
879|rejects_cross_stage_dependency_links|tests/schedule.rs::cross_stage_system_edge_is_rejected_at_build
890|rejects_missing_resources|tests/schedule.rs::missing_required_resource_is_rejected_at_build
900|rejects_duplicate_labels|tests/schedule.rs::duplicate_system_labels_are_rejected_at_build
911|startup_stage_runs_once|tests/schedule.rs::startup_runs_once_across_updates
930|stage_flush_updates_are_allocation_free_after_warmup|tests/allocation.rs::stage_flush_steady_state_is_allocation_free
959|fixed_update_steps_respect_accumulator|tests/schedule.rs::fixed_update_respects_accumulator_and_cap
980|flush_mode_stage_applies_commands_between_stages|tests/schedule.rs::flush_mode_stage_makes_commands_visible_between_stages
1013|flush_mode_end_defers_commands_until_update_end|tests/schedule.rs::flush_mode_final_defers_commands_until_update_end
1046|system_in_state_gates_execution|tests/schedule.rs::in_state_gates_execution
1071|system_state_changed_runs_on_transition|tests/schedule.rs::state_changed_runs_after_explicit_apply
1098|state_transition_update_is_allocation_free_after_warmup|tests/allocation.rs::state_transition_steady_state_is_allocation_free
1128|system_interval_buffers_time|docs/parity.md reject-doc downstream-host
1148|system_pipe_passes_values_between_piped_systems|docs/parity.md reject-doc
1187|system_pipe_without_payload_is_allocation_free_after_warmup|docs/parity.md reject-doc
1231|system_run_if_gates_execution|tests/schedule.rs::run_if_skips_system_body
1255|set_run_if_gates_execution_once_per_stage|tests/schedule.rs::set_run_if_gates_all_members_once_per_stage
1305|rejects_unknown_set|tests/schedule.rs::unknown_system_set_is_rejected_at_build
1315|rejects_duplicate_set_labels|tests/schedule.rs::duplicate_set_labels_are_rejected_at_build
1326|set_and_system_conditions_are_both_required|tests/schedule.rs::set_and_system_conditions_compose_with_and_semantics
1362|run_if_and_set_are_allocation_free_after_warmup|tests/allocation.rs::run_if_and_set_steady_state_is_allocation_free
4963|query1_returns_all_entities_with_component|tests/query.rs::query1_returns_all_entities_with_component
4981|query2_returns_intersection|tests/query.rs::query2_returns_intersection
5000|table_component_insert_get_query|tests/query.rs::table_component_insert_get_query
5022|table_component_migration_preserves_ticks|tests/world_lifecycle_state.rs::archetype_move_preserves_retained_component
5084|query2_mixed_table_and_sparse_components|tests/query.rs::query2_mixed_table_and_sparse_components
5114|query_cached_reuses_plan|tests/query_cache.rs::query_cache_cold_and_hot_hit
5125|query_cached_results_updates_on_add_remove|tests/query_cache.rs::query_cache_updates_on_spawn
5156|query_cached_results_respects_without|tests/parity_gaps.rs::query_cache_respects_without
5203|query_excludes_inactive_when_enabled|tests/query.rs::explicit_without_excludes_domain_marker_without_magic_name
5236|query_cached_results_respects_inactive_changes|tests/parity_gaps.rs::query_cache_respects_inactive_changes
5282|query_cached_results_clear_rebuilds|tests/query_result_cache.rs::result_cache_invalidate_and_rebuild
5312|query_cached_results_enables_events_when_gated|tests/parity_gaps.rs::query_cache_survives_frame_event_clear
5338|apply_event_gating_is_allocation_free_after_warmup|tests/allocation.rs::event_dispatch_steady_state_is_allocation_free
5359|query_cached_results_is_allocation_free_after_warmup|tests/allocation.rs::query_result_cache_hit_is_allocation_free
5391|query_cached_results_rejects_added_or_changed|tests/query_cache.rs::query_result_cache_rejects_added_or_changed
5406|command_buffer_applies_on_flush|tests/commands_deferred.rs::deferred_spawn_is_not_alive_until_flush
5417|command_buffer_flush_is_allocation_free_after_warmup|tests/allocation.rs::command_flush_steady_state_is_allocation_free
5446|spawn_bundle_inserts_components_on_flush|tests/world_table_bundle.rs::spawn_bundle_with_table_components
5460|bundle_insert_helpers_resolve_components|tests/world_table_bundle.rs::spawn_bundle_with_table_components
5474|bundle_insert_helpers_return_false_when_component_missing|tests/phase3_failure_contracts.rs::deferred_spawn_bundle_rolls_back_on_bundle_error
5490|command_buffer_despawn_removes_components|tests/query.rs::query_skips_despawned_entities
5504|query_respects_without_list|tests/query.rs::query_respects_without_list
5530|event_gating_from_schedule_controls_dispatch|tests/parity_gaps.rs::schedule_event_roles_control_dispatch
5545|read_event_typed_downcasts_payload|tests/events.rs::events_send_and_read_in_order
5569|read_event_is_allocation_free_after_warmup|tests/allocation.rs::event_send_read_steady_state_is_allocation_free
5595|frame_events_are_dropped_after_update|tests/events.rs::update_and_render_boundaries_clear_only_their_owned_frame_channels
5604|persistent_events_survive_update_when_unread|tests/schedule.rs::persistent_events_survive_update_until_read
5617|validate_schedule_via_world|tests/schedule.rs::missing_required_resource_is_rejected_at_build
5628|query_added_filters_by_tick|tests/query.rs::query_added_filters_by_tick
5662|query_changed_filters_by_tick|tests/query.rs::query_changed_filters_by_tick
5700|get_mut_marks_changed_tick|tests/query.rs::query_changed_filters_by_tick
5739|query_cached_params_updates_last_tick|tests/query_cache.rs::membership_cache_stores_structural_members_for_added_queries
5781|query_cache_is_owner_scoped|tests/query_cache.rs::query_cache_is_owner_scoped
5826|register_component_records_metadata_and_events|tests/component_lifecycle.rs::component_added_emitted_after_commit
5842|register_component_untyped_requires_tag|src/component/registry/tests.rs::untyped_registration_requires_tag_options + tests/parity_gaps.rs::register_untyped_requires_tag
5848|component_events_emitted_on_insert_and_remove|tests/parity_gaps.rs::component_removed_emitted_on_despawn
5870|component_add_not_emitted_on_replace|tests/component_lifecycle.rs::replacement_does_not_emit_second_add
5885|component_events_enabled_on_read_when_gated|tests/parity_gaps.rs::component_events_readable_after_registration
5899|component_event_dispatch_is_allocation_free_after_warmup|tests/allocation.rs::component_event_dispatch_is_allocation_free
5948|read_component_event_is_allocation_free_after_warmup|tests/allocation.rs::component_event_read_is_allocation_free
5974|query_ids_filters_with_and_without|tests/query.rs::dynamic_component_ids_apply_explicit_without_filters
6001|query_ids_excludes_inactive_when_enabled|tests/query.rs::dynamic_component_ids_apply_explicit_without_filters
6040|query_ids_cached_last_tick_updates_on_exhaust|tests/query.rs::entity_cursor_forks_and_commits_only_after_observed_exhaustion
6070|query_ids_cached_last_tick_skips_on_partial_iteration|tests/query.rs::entity_cursor_forks_and_commits_only_after_observed_exhaustion
6098|query_spec_from_names_reports_missing_components|docs/parity.md reject-doc
6107|query_spec_from_names_builds_expected_spec|docs/parity.md reject-doc
6125|query1_params_panics_on_unknown_component|tests/query.rs::query_unregistered_component_returns_error
6139|query_ids_panics_on_unknown_component|tests/query.rs::dynamic_component_ids_are_checked_with_or_windows
6149|resource_changed_ticks_update|tests/resources.rs::resource_insert_get_remove_round_trip
6169|resource_named_ticks_and_changed_flags_update|docs/parity.md reject-doc
6203|resource_scope_updates_and_marks_changed|tests/parity_gaps.rs::resource_scope_marks_changed
6225|resource_scope_returns_none_when_missing|tests/resources.rs::resource_scope_reports_missing_without_mutation
6232|register_and_run_system_updates_registry|docs/parity.md reject-doc
6251|by_id_component_helpers_respect_entity_lifecycle|tests/parity_gaps.rs::dynamic_component_access_respects_lifecycle
6285|commands_run_system_executes_on_flush|docs/parity.md reject-doc
6300|run_system_once_executes|docs/parity.md reject-doc
6318|steady_state_update_is_allocation_free|tests/allocation.rs::app_update_steady_state_is_allocation_free
6385|steady_state_table_query_is_allocation_free|tests/allocation.rs::table_query_steady_state_is_allocation_free
6451|inactive_tag_sets_exclusion_and_accessor|docs/parity.md reject-doc
6471|advance_tick_wraps_and_increments|tests/schedule.rs::app_runs_update_system_in_order + src/world/mod.rs::change_tick_exhaustion_poison_world_mutations
6481|apply_event_gating_reenables_when_disabled|docs/parity.md reject-doc
6493|world_render_runs_render_stage_systems|tests/schedule.rs::render_rejects_structural_commands_in_system
6511|query_cached2_handles_registered_and_missing_components|tests/parity_gaps.rs::query2_result_cache_handles_registered_and_missing
6533|get_mut_by_id_comp_fast_updates_value|tests/parity_gaps.rs::dynamic_component_mut_updates_value
6550|steady_state_flush_reuses_command_buffer|tests/allocation.rs::command_buffer_reuses_capacity_after_warmup"""

ROW_RE = re.compile(
    r"^\| (\d+) \| `([^`]+)` \| (preserve|adapt|reject) \| ([^|]+) \|"
)
SYMBOL_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


@dataclass(frozen=True)
class ParityRow:
    source_line: int
    source: str
    classification: str
    owner: str


def load_rows() -> list[ParityRow]:
    rows: list[ParityRow] = []
    for line in PARITY.read_text(encoding="utf-8").splitlines():
        match = ROW_RE.match(line)
        if match:
            rows.append(
                ParityRow(
                    source_line=int(match.group(1)),
                    source=match.group(2),
                    classification=match.group(3),
                    owner=match.group(4).strip(),
                )
            )
    if len(rows) != 151:
        raise SystemExit(f"expected 151 parity rows, found {len(rows)}")
    keys = [(row.source_line, row.source) for row in rows]
    if len(set(keys)) != len(keys):
        raise SystemExit("duplicate parity source-line/name key in docs/parity.md")
    return rows


def load_proofs() -> dict[tuple[int, str], str]:
    proofs: dict[tuple[int, str], str] = {}
    for number, raw in enumerate(PROOF_ROWS.splitlines(), start=1):
        if not raw:
            continue
        parts = raw.split("|", 2)
        if len(parts) != 3:
            raise SystemExit(f"invalid canonical proof row {number}: {raw}")
        source_line_text, source, proof = parts
        try:
            key = (int(source_line_text), source)
        except ValueError as error:
            raise SystemExit(f"invalid source line in canonical proof row {number}") from error
        if key in proofs:
            raise SystemExit(f"duplicate canonical proof key: {key}")
        proofs[key] = proof
    return proofs


def function_definitions(source: str, symbol: str) -> int:
    pattern = re.compile(
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+{re.escape(symbol)}\s*(?:<|\()"
    )
    return len(pattern.findall(source))


def validate_proof(row: ParityRow, proof: str) -> None:
    if row.classification == "reject":
        if not proof.startswith("docs/parity.md reject-doc"):
            raise SystemExit(
                f"reject row {row.source_line}/{row.source} must cite docs/parity.md reject-doc"
            )
        if not PARITY.is_file():
            raise SystemExit("reject documentation path does not exist: docs/parity.md")
        return

    for citation in proof.split(" + "):
        if "::" not in citation:
            raise SystemExit(
                f"proof for {row.source_line}/{row.source} is not path::symbol: {citation}"
            )
        relative_path, symbol = citation.rsplit("::", 1)
        if not SYMBOL_RE.fullmatch(symbol):
            raise SystemExit(
                f"proof for {row.source_line}/{row.source} has invalid symbol: {symbol}"
            )
        path = ROOT / relative_path
        if not path.is_file():
            raise SystemExit(
                f"proof for {row.source_line}/{row.source} cites missing path: {relative_path}"
            )
        count = function_definitions(path.read_text(encoding="utf-8"), symbol)
        if count != 1:
            raise SystemExit(
                f"proof for {row.source_line}/{row.source} must resolve exactly once: "
                f"{citation} (found {count})"
            )


def resolved_rows() -> list[tuple[ParityRow, str]]:
    rows = load_rows()
    proofs = load_proofs()
    row_keys = {(row.source_line, row.source) for row in rows}
    proof_keys = set(proofs)
    missing = sorted(row_keys - proof_keys)
    unknown = sorted(proof_keys - row_keys)
    if missing or unknown:
        raise SystemExit(
            f"canonical proof keys differ from docs/parity.md; missing={missing}, unknown={unknown}"
        )
    resolved = [(row, proofs[(row.source_line, row.source)]) for row in rows]
    for row, proof in resolved:
        validate_proof(row, proof)
    return resolved


def rust_string(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def render() -> str:
    lines = [
        "//! Parity ledger closure proofs for all 151 pd-asteroids characterization tests.",
        "//! Generated from docs/parity.md — do not hand-edit; regenerate via scripts/generate_parity_ledger.py",
        "",
        "struct ParityClosure {",
        "    source_line: usize,",
        "    source: &'static str,",
        "    class: &'static str,",
        "    owner: &'static str,",
        "    moirai_proof: &'static str,",
        "}",
        "",
        "const PARITY_CLOSURES: &[ParityClosure] = &[",
    ]
    for row, proof in resolved_rows():
        lines.append(
            "    ParityClosure { "
            f"source_line: {row.source_line}, source: \"{rust_string(row.source)}\", "
            f"class: \"{row.classification}\", owner: \"{rust_string(row.owner)}\", "
            f"moirai_proof: \"{rust_string(proof)}\" "
            "},"
        )
    lines.extend(
        [
            "];",
            "",
            "fn cited_functions(proof: &str) -> impl Iterator<Item = (&str, &str)> {",
            "    proof.split(\" + \").map(|citation| {",
            "        citation",
            "            .rsplit_once(\"::\")",
            "            .unwrap_or_else(|| panic!(\"non-reject proof must use path::symbol syntax: {citation}\"))",
            "    })",
            "}",
            "",
            "fn function_definition_count(source: &str, symbol: &str) -> usize {",
            "    let needle = format!(\"fn {symbol}\");",
            "    source",
            "        .lines()",
            "        .filter_map(|line| {",
            "            line.trim_start()",
            "                .find(&needle)",
            "                .map(|index| &line.trim_start()[index + needle.len()..])",
            "        })",
            "        .filter(|tail| tail.starts_with('(') || tail.starts_with('<'))",
            "        .count()",
            "}",
            "",
            "#[test]",
            "fn parity_ledger_accounts_for_all_source_rows() {",
            "    assert_eq!(PARITY_CLOSURES.len(), 151);",
            "    let mut keys = PARITY_CLOSURES",
            "        .iter()",
            "        .map(|row| (row.source_line, row.source))",
            "        .collect::<Vec<_>>();",
            "    keys.sort_unstable();",
            "    keys.dedup();",
            "    assert_eq!(keys.len(), 151, \"duplicate source line/name key in ledger\");",
            "}",
            "",
            "#[test]",
            "fn parity_ledger_reject_rows_document_negative_contract() {",
            "    for row in PARITY_CLOSURES {",
            "        if row.class == \"reject\" {",
            "            assert!(",
            "                row.moirai_proof.starts_with(\"docs/parity.md reject-doc\"),",
            "                \"reject row {}/{} must cite reject-doc proof\",",
            "                row.source_line,",
            "                row.source",
            "            );",
            "        }",
            "    }",
            "}",
            "",
            "#[test]",
            "fn parity_ledger_citations_resolve_to_one_test_function() {",
            "    let root = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"));",
            "    for row in PARITY_CLOSURES {",
            "        if row.class == \"reject\" {",
            "            assert!(root.join(\"docs/parity.md\").is_file());",
            "            continue;",
            "        }",
            "        for (path, symbol) in cited_functions(row.moirai_proof) {",
            "            let source = std::fs::read_to_string(root.join(path)).unwrap_or_else(|error| {",
            "                panic!(",
            "                    \"proof path {path} for {}/{} is unreadable: {error}\",",
            "                    row.source_line, row.source",
            "                )",
            "            });",
            "            assert_eq!(",
            "                function_definition_count(&source, symbol),",
            "                1,",
            "                \"proof {path}::{symbol} for {}/{} must resolve exactly once\",",
            "                row.source_line,",
            "                row.source",
            "            );",
            "        }",
            "    }",
            "}",
            "",
            "#[test]",
            "fn parity_ledger_phase6_rows_map_to_allocation_or_diagnostics_tests() {",
            "    for row in PARITY_CLOSURES {",
            "        if row.owner == \"Phase 6\" {",
            "            assert!(",
            "                row.moirai_proof.contains(\"tests/allocation.rs\")",
            "                    || row.moirai_proof.contains(\"tests/parity_gaps.rs\")",
            "                    || row.moirai_proof.contains(\"tests/schedule_observer.rs\"),",
            "                \"Phase 6 row {} must map to allocation/diagnostics proof: {}\",",
            "                row.source,",
            "                row.moirai_proof",
            "            );",
            "        }",
            "    }",
            "}",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate evidence and fail if the checked-in ledger is not current",
    )
    args = parser.parse_args()
    generated = render()
    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != generated:
            diff = "".join(
                difflib.unified_diff(
                    current.splitlines(keepends=True),
                    generated.splitlines(keepends=True),
                    fromfile=str(OUT),
                    tofile="generated parity ledger",
                )
            )
            raise SystemExit(f"{OUT} is stale; regenerate it\n{diff}")
        print(f"checked {OUT}")
        return
    OUT.write_text(generated, encoding="utf-8")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
