//! Landin compiler — unified test entry point.
//!
//! Stage 13.27: Cleaned up — only behavioral test files are included.
//! All doc-existence tests removed per user feedback.
//! Stage 9 and Stage 10 test files were entirely doc-existence checks
//! (checking file existence, reading source files for content patterns,
//! checking Cargo.toml version strings) — all removed.

// Stage 18.326: Shared test helpers (run_program, assert_runtime) for all tests.
// Per §1.0 原則 6 (通解>特解): one shared helper for all 29+ test files.
#[path = "common/mod.rs"]
mod common;

// === Stage 0 ===
#[path = "v0/stage0/plan/ast_structure_tests.rs"]
mod ast_structure_tests;
#[path = "v0/stage0/plan/lexer_tests.rs"]
mod lexer_tests;
#[path = "v0/stage0/plan/parser_tests.rs"]
mod parser_tests;

// === Stage 1 ===
#[path = "v0/stage1/plan/hir_lowering_tests.rs"]
mod hir_lowering_tests;
#[path = "v0/stage1/plan/hir_resolution_tests.rs"]
mod hir_resolution_tests;
#[path = "v0/stage1/plan/hir_scope_resolution_tests.rs"]
mod hir_scope_resolution_tests;
#[path = "v0/stage1/plan/hir_structure_tests.rs"]
mod hir_structure_tests;

// === Stage 2 ===
#[path = "v0/stage2/plan/integration_tests.rs"]
mod integration_tests;
#[path = "v0/stage2/plan/mir_lowering_tests.rs"]
mod mir_lowering_tests;
#[path = "v0/stage2/plan/negative_cases_tests.rs"]
mod negative_cases_tests;
#[path = "v0/stage2/plan/typeck_tests.rs"]
mod typeck_tests;

// === Stage 3 ===
#[path = "v0/stage3/plan/codegen_tests.rs"]
mod codegen_tests;
#[path = "v0/stage3/plan/deep_inspection_tests.rs"]
mod deep_inspection_tests;

// === Stage 4 ===
#[path = "v0/stage4/plan/closure_call_tests.rs"]
mod closure_call_tests;
#[path = "v0/stage4/plan/closure_capture_tests.rs"]
mod closure_capture_tests;
#[path = "v0/stage4/plan/closure_full_call_tests.rs"]
mod closure_full_call_tests;
#[path = "v0/stage4/plan/macro_system_tests.rs"]
mod macro_system_tests;
#[path = "v0/stage4/plan/visibility_tests.rs"]
mod visibility_tests;

// === Stage 5 ===
#[path = "v0/stage5/plan/builtin_clone_drop_tests.rs"]
mod builtin_clone_drop_tests;
#[path = "v0/stage5/plan/builtin_copy_activation_tests.rs"]
mod builtin_copy_activation_tests;
#[path = "v0/stage5/plan/builtin_traits_tests.rs"]
mod builtin_traits_tests;
#[path = "v0/stage5/plan/codegen_combined_orchestrator_tests.rs"]
mod codegen_combined_orchestrator_tests;
#[path = "v0/stage5/plan/codegen_dyn_trait_method_call_tests.rs"]
mod codegen_dyn_trait_method_call_tests;
#[path = "v0/stage5/plan/codegen_dynptr_orchestrator_tests.rs"]
mod codegen_dynptr_orchestrator_tests;
#[path = "v0/stage5/plan/codegen_dynptr_spec_builder_tests.rs"]
mod codegen_dynptr_spec_builder_tests;
#[path = "v0/stage5/plan/codegen_dynptr_text_tests.rs"]
mod codegen_dynptr_text_tests;
#[path = "v0/stage5/plan/codegen_plan_orchestrator_tests.rs"]
mod codegen_plan_orchestrator_tests;
#[path = "v0/stage5/plan/codegen_text_batch_from_resolver_tests.rs"]
mod codegen_text_batch_from_resolver_tests;
#[path = "v0/stage5/plan/codegen_text_batch_tests.rs"]
mod codegen_text_batch_tests;
#[path = "v0/stage5/plan/codegen_trait_dispatch_plan_tests.rs"]
mod codegen_trait_dispatch_plan_tests;
#[path = "v0/stage5/plan/codegen_trait_dispatch_summary_tests.rs"]
mod codegen_trait_dispatch_summary_tests;
#[path = "v0/stage5/plan/codegen_vtable_batch_tests.rs"]
mod codegen_vtable_batch_tests;
#[path = "v0/stage5/plan/codegen_vtable_emission_helper_tests.rs"]
mod codegen_vtable_emission_helper_tests;
#[path = "v0/stage5/plan/codegen_vtable_global_text_tests.rs"]
mod codegen_vtable_global_text_tests;
#[path = "v0/stage5/plan/codegen_vtable_orchestrator_tests.rs"]
mod codegen_vtable_orchestrator_tests;
#[path = "v0/stage5/plan/codegen_vtable_spec_builder_tests.rs"]
mod codegen_vtable_spec_builder_tests;
#[path = "v0/stage5/plan/copy_unification_tests.rs"]
mod copy_unification_tests;
#[path = "v0/stage5/plan/def_id_name_map_tests.rs"]
mod def_id_name_map_tests;
#[path = "v0/stage5/plan/driver_dyn_trait_plan_integration_tests.rs"]
mod driver_dyn_trait_plan_integration_tests;
#[path = "v0/stage5/plan/driver_stdlib_tests.rs"]
mod driver_stdlib_tests;
#[path = "v0/stage5/plan/driver_validation_tests.rs"]
mod driver_validation_tests;
#[path = "v0/stage5/plan/dyn_trait_e2e_integration_tests.rs"]
mod dyn_trait_e2e_integration_tests;
#[path = "v0/stage5/plan/dyn_trait_fat_ptr_batch_tests.rs"]
mod dyn_trait_fat_ptr_batch_tests;
#[path = "v0/stage5/plan/dyn_trait_fat_ptr_builder_tests.rs"]
mod dyn_trait_fat_ptr_builder_tests;
#[path = "v0/stage5/plan/dyn_trait_fat_ptr_from_resolver_tests.rs"]
mod dyn_trait_fat_ptr_from_resolver_tests;
#[path = "v0/stage5/plan/dyn_trait_fat_ptr_tests.rs"]
mod dyn_trait_fat_ptr_tests;
#[path = "v0/stage5/plan/dyn_trait_fat_ptr_text_tests.rs"]
mod dyn_trait_fat_ptr_text_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_batch_tests.rs"]
mod dyn_trait_method_call_batch_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_builder_tests.rs"]
mod dyn_trait_method_call_builder_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_from_resolver_tests.rs"]
mod dyn_trait_method_call_from_resolver_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_in_plan_by_method_tests.rs"]
mod dyn_trait_method_call_in_plan_by_method_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_in_plan_tests.rs"]
mod dyn_trait_method_call_in_plan_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_tests.rs"]
mod dyn_trait_method_call_tests;
#[path = "v0/stage5/plan/dyn_trait_method_call_text_tests.rs"]
mod dyn_trait_method_call_text_tests;
#[path = "v0/stage5/plan/dyn_trait_mir_plan_tests.rs"]
mod dyn_trait_mir_plan_tests;
#[path = "v0/stage5/plan/dyn_trait_mir_plan_text_tests.rs"]
mod dyn_trait_mir_plan_text_tests;
#[path = "v0/stage5/plan/dyn_trait_mir_summary_from_resolver_tests.rs"]
mod dyn_trait_mir_summary_from_resolver_tests;
#[path = "v0/stage5/plan/dyn_trait_mir_summary_tests.rs"]
mod dyn_trait_mir_summary_tests;
#[path = "v0/stage5/plan/dyn_trait_param_kinds_tests.rs"]
mod dyn_trait_param_kinds_tests;
#[path = "v0/stage5/plan/dyn_trait_ptr_tests.rs"]
mod dyn_trait_ptr_tests;
#[path = "v0/stage5/plan/dyn_trait_return_kind_tests.rs"]
mod dyn_trait_return_kind_tests;
#[path = "v0/stage5/plan/emit_dyn_trait_ptrs_delegation_tests.rs"]
mod emit_dyn_trait_ptrs_delegation_tests;
#[path = "v0/stage5/plan/emit_vtables_delegation_tests.rs"]
mod emit_vtables_delegation_tests;
#[path = "v0/stage5/plan/facade_integration_tests.rs"]
mod facade_integration_tests;
#[path = "v0/stage5/plan/impl_completeness_tests.rs"]
mod impl_completeness_tests;
#[path = "v0/stage5/plan/impl_validation_tests.rs"]
mod impl_validation_tests;
#[path = "v0/stage5/plan/is_stdlib_trait_tests.rs"]
mod is_stdlib_trait_tests;
#[path = "v0/stage5/plan/mini_cargo_tests.rs"]
mod mini_cargo_tests;
#[path = "v0/stage5/plan/mir_lower_dyn_trait_method_call_integration_tests.rs"]
mod mir_lower_dyn_trait_method_call_integration_tests;
#[path = "v0/stage5/plan/mir_lower_dyn_trait_plan_context_tests.rs"]
mod mir_lower_dyn_trait_plan_context_tests;
#[path = "v0/stage5/plan/primitive_copy_tests.rs"]
mod primitive_copy_tests;
#[path = "v0/stage5/plan/stdlib_alloc_tests.rs"]
mod stdlib_alloc_tests;
#[path = "v0/stage5/plan/stdlib_arithmetic_traits_tests.rs"]
mod stdlib_arithmetic_traits_tests;
#[path = "v0/stage5/plan/stdlib_core_traits_tests.rs"]
mod stdlib_core_traits_tests;
#[path = "v0/stage5/plan/stdlib_facade_tests.rs"]
mod stdlib_facade_tests;
#[path = "v0/stage5/plan/stdlib_io_unary_traits_tests.rs"]
mod stdlib_io_unary_traits_tests;
#[path = "v0/stage5/plan/stdlib_layer_tests.rs"]
mod stdlib_layer_tests;
#[path = "v0/stage5/plan/stdlib_layout_tests.rs"]
mod stdlib_layout_tests;
#[path = "v0/stage5/plan/stdlib_marker_traits_tests.rs"]
mod stdlib_marker_traits_tests;
#[path = "v0/stage5/plan/stdlib_mvp_tests.rs"]
mod stdlib_mvp_tests;
#[path = "v0/stage5/plan/stdlib_param_kinds_accuracy_tests.rs"]
mod stdlib_param_kinds_accuracy_tests;
#[path = "v0/stage5/plan/stdlib_std_tests.rs"]
mod stdlib_std_tests;
#[path = "v0/stage5/plan/stdlib_trait_count_tests.rs"]
mod stdlib_trait_count_tests;
#[path = "v0/stage5/plan/stdlib_trait_method_accessors_2_tests.rs"]
mod stdlib_trait_method_accessors_2_tests;
#[path = "v0/stage5/plan/stdlib_trait_method_accessors_tests.rs"]
mod stdlib_trait_method_accessors_tests;
#[path = "v0/stage5/plan/stdlib_trait_method_tests.rs"]
mod stdlib_trait_method_tests;
#[path = "v0/stage5/plan/stdlib_trait_methods_by_is_unsafe_tests.rs"]
mod stdlib_trait_methods_by_is_unsafe_tests;
#[path = "v0/stage5/plan/stdlib_trait_methods_by_param_count_tests.rs"]
mod stdlib_trait_methods_by_param_count_tests;
#[path = "v0/stage5/plan/stdlib_trait_methods_by_return_kind_tests.rs"]
mod stdlib_trait_methods_by_return_kind_tests;
#[path = "v0/stage5/plan/stdlib_trait_methods_by_self_kind_tests.rs"]
mod stdlib_trait_methods_by_self_kind_tests;
#[path = "v0/stage5/plan/stdlib_type_resolve_tests.rs"]
mod stdlib_type_resolve_tests;
#[path = "v0/stage5/plan/stdlib_vtable_emission_summary_tests.rs"]
mod stdlib_vtable_emission_summary_tests;
#[path = "v0/stage5/plan/stdlib_vtable_emission_tests.rs"]
mod stdlib_vtable_emission_tests;
#[path = "v0/stage5/plan/stdlib_vtable_layout_tests.rs"]
mod stdlib_vtable_layout_tests;
#[path = "v0/stage5/plan/stdlib_vtable_plan_tests.rs"]
mod stdlib_vtable_plan_tests;
#[path = "v0/stage5/plan/stdlib_vtable_size_tests.rs"]
mod stdlib_vtable_size_tests;
#[path = "v0/stage5/plan/stdlib_vtable_symbol_tests.rs"]
mod stdlib_vtable_symbol_tests;
#[path = "v0/stage5/plan/text_emitter_dynptr_delegation_tests.rs"]
mod text_emitter_dynptr_delegation_tests;
#[path = "v0/stage5/plan/text_emitter_vtable_delegation_tests.rs"]
mod text_emitter_vtable_delegation_tests;
#[path = "v0/stage5/plan/trait_coherence_tests.rs"]
mod trait_coherence_tests;
#[path = "v0/stage5/plan/trait_hierarchy_tests.rs"]
mod trait_hierarchy_tests;
#[path = "v0/stage5/plan/trait_impl_stats_tests.rs"]
mod trait_impl_stats_tests;
#[path = "v0/stage5/plan/trait_integration_tests.rs"]
mod trait_integration_tests;
#[path = "v0/stage5/plan/trait_method_query_tests.rs"]
mod trait_method_query_tests;
#[path = "v0/stage5/plan/trait_resolver_tests.rs"]
mod trait_resolver_tests;
#[path = "v0/stage5/plan/trait_summary_tests.rs"]
mod trait_summary_tests;
#[path = "v0/stage5/plan/ty_is_copy_tests.rs"]
mod ty_is_copy_tests;
#[path = "v0/stage5/plan/vtable_codegen_tests.rs"]
mod vtable_codegen_tests;
#[path = "v0/stage5/plan/vtable_method_resolve_tests.rs"]
mod vtable_method_resolve_tests;
#[path = "v0/stage5/plan/vtable_tests.rs"]
mod vtable_tests;

// === Stage 7 ===
#[path = "v0/stage7/plan/design_writeback_verification_tests.rs"]
mod design_writeback_verification_tests;
#[path = "v0/stage7/plan/deep_review_tests.rs"]
mod stage7_deep_review_tests;
#[path = "v0/stage7/plan/region_inference_tests.rs"]
mod stage7_region_inference_tests;
#[path = "v0/stage7/plan/systematic_review_v014_tests.rs"]
mod systematic_review_v014_tests;
#[path = "v0/stage7/plan/user_defined_trait_dyn_tests.rs"]
mod user_defined_trait_dyn_tests;

// === Stage 13 (behavioral tests only) ===
// Runtime verification tests — the only stage 13 tests that remain.
#[cfg(feature = "llvm-backend")]
#[path = "v0/stage13/plan/stage13_18_runtime_tests.rs"]
mod stage13_18_runtime_tests;

// === Stage 15 (v0.2: Ty interning prep + perf) ===
#[path = "v0/stage15/plan/const_ty_box_to_ty_tests.rs"]
mod stage15_const_ty_box_to_ty_tests;
#[path = "v0/stage15/plan/crate_adt_layouts_tests.rs"]
mod stage15_crate_adt_layouts_tests;
#[path = "v0/stage15/plan/driver_diagnostics_integration_tests.rs"]
mod stage15_driver_diagnostics_integration_tests;
#[path = "v0/stage15/plan/error_system_cleanup_tests.rs"]
mod stage15_error_system_cleanup_tests;
#[path = "v0/stage15/plan/method_return_type_cache_tests.rs"]
mod stage15_method_return_type_cache_tests;
// Stage 15.35 (HP-10): NLL fixpoint liveness analysis (v0.2 Phase 2 Task 7).
#[path = "v0/stage15/plan/nll_fixpoint_liveness_tests.rs"]
mod stage15_nll_fixpoint_liveness_tests;
// Stage 15.36 (HP-10 step 2): kill_expired_borrows_dataflow + check_mir_body_with_dataflow.
#[path = "v0/stage15/plan/kill_borrows_dataflow_tests.rs"]
mod stage15_kill_borrows_dataflow_tests;
// Stage 15.37 (HP-10 step 3 — DEFERRED): driver switch deferred + legacy deprecation + GAP-1 conflict.
#[path = "v0/stage15/plan/stage15_37_driver_switch_tests.rs"]
mod stage15_37_driver_switch_tests;
// Stage 15.38: Borrow-check comparison diagnostic tool (informs GAP-1 reconciliation).
#[path = "v0/stage15/plan/borrowck_comparison_diagnostic_tests.rs"]
mod stage15_borrowck_comparison_diagnostic_tests;
// Stage 15.39 (HP-10 step 4 — Option B): "was ever read" check preserves GAP-1 in dataflow path.
#[path = "v0/stage15/plan/option_b_implementation_tests.rs"]
mod stage15_option_b_implementation_tests;
// Stage 15.40 (HP-10 — COMPLETE): kill-on-redefinition + driver switch (false positive fixed).
#[path = "v0/stage15/plan/stage15_40_driver_switch_tests.rs"]
mod stage15_40_driver_switch_tests;
// Stage 15.41 (HP-10 — cleanup): legacy `check_mir_body` delegates to dataflow path; dead code removed.
#[path = "v0/stage15/plan/stage15_41_legacy_delegation_tests.rs"]
mod stage15_41_legacy_delegation_tests;
// Stage 15.43 (HP-12 step 2): ty_needs_drop analysis — drop elaboration foundation.
#[path = "v0/stage15/plan/ty_needs_drop_integration_tests.rs"]
mod stage15_ty_needs_drop_integration_tests;
// Stage 15.44 (HP-12 step 3): elaborate_drops pass — insert Drop terminators.
#[path = "v0/stage15/plan/elaborate_drops_integration_tests.rs"]
mod stage15_elaborate_drops_integration_tests;
// Stage 15.46 (HP-12 step 5): Drop elaboration integration — wired into driver pipeline.
#[path = "v0/stage15/plan/drop_elaboration_integration_tests.rs"]
mod stage15_drop_elaboration_integration_tests;
// Stage 15.52 (HP-5 step 5): Region allocation integration tests + gate review.
#[path = "v0/stage15/plan/region_allocation_integration_tests.rs"]
mod stage15_region_allocation_integration_tests;
// Stage 15.58 (HP-12 step 8): impl Drop conformance + integration tests.
#[path = "v0/stage15/plan/impl_drop_conformance_tests.rs"]
mod stage15_impl_drop_conformance_tests;
// Stage 15.61 (HP-12 fix): impl Drop end-to-end tests (crash + borrowck fix).
#[path = "v0/stage15/plan/impl_drop_e2e_tests.rs"]
mod stage15_impl_drop_e2e_tests;
// Stage 15.62 (HP-12 fix): Drop order + double-drop prevention tests.
#[path = "v0/stage15/plan/impl_drop_order_tests.rs"]
mod stage15_impl_drop_order_tests;
// Stage 15.63 (HP-12 fix): Recursive drop (fields with Drop) tests.
#[path = "v0/stage15/plan/recursive_drop_tests.rs"]
mod stage15_recursive_drop_tests;
// Stage 15.64 (HP-12 fix): Struct literal Copy→Move + field-copy drop prevention.
#[path = "v0/stage15/plan/struct_literal_copy_move_tests.rs"]
mod stage15_struct_literal_copy_move_tests;
// Stage 15.66 (HP-12 fix): Recursive drop for enums (SwitchInt in drop glue).
#[path = "v0/stage15/plan/enum_recursive_drop_tests.rs"]
mod stage15_enum_recursive_drop_tests;
#[path = "v0/stage15/plan/substs_ref_rc_tests.rs"]
mod stage15_substs_ref_rc_tests;
#[path = "v0/stage15/plan/ty_interner_integration_tests.rs"]
mod stage15_ty_interner_integration_tests;
#[path = "v0/stage15/plan/vtable_interning_and_trait_error_tests.rs"]
mod stage15_vtable_interning_and_trait_error_tests;
#[path = "v0/stage15/plan/writeback_consolidation_tests.rs"]
mod stage15_writeback_consolidation_tests;
#[path = "v0/stage16/plan/stage16_05_field_not_found_error_tests.rs"]
mod stage16_05_field_not_found_error_tests;
#[path = "v0/stage16/plan/stage16_06_sound_copy_derivation_tests.rs"]
mod stage16_06_sound_copy_derivation_tests;
#[path = "v0/stage16/plan/stage16_07_def_id_keyed_lookup_tests.rs"]
mod stage16_07_def_id_keyed_lookup_tests;
#[path = "v0/stage16/plan/stage16_08_builtin_trait_migration_tests.rs"]
mod stage16_08_builtin_trait_migration_tests;
#[path = "v0/stage16/plan/stage16_09_deep_review_gap_closure_tests.rs"]
mod stage16_09_deep_review_gap_closure_tests;
#[path = "v0/stage16/plan/stage16_10_vtable_def_id_lookup_tests.rs"]
mod stage16_10_vtable_def_id_lookup_tests;
#[path = "v0/stage16/plan/stage16_11_spur_deprecation_tests.rs"]
mod stage16_11_spur_deprecation_tests;
#[path = "v0/stage16/plan/stage16_12_deep_review_round2_tests.rs"]
mod stage16_12_deep_review_round2_tests;
#[path = "v0/stage16/plan/stage16_13_synthesized_closure_infrastructure_tests.rs"]
mod stage16_13_synthesized_closure_infrastructure_tests;
#[path = "v0/stage16/plan/stage16_14_synthesized_closure_mir_body_tests.rs"]
mod stage16_14_synthesized_closure_mir_body_tests;
#[path = "v0/stage16/plan/stage16_15_deep_review_round3_tests.rs"]
mod stage16_15_deep_review_round3_tests;
#[path = "v0/stage16/plan/stage16_18_deep_review_round4_tests.rs"]
mod stage16_18_deep_review_round4_tests;
#[path = "v0/stage16/plan/stage16_19_design_writeback_tests.rs"]
mod stage16_19_design_writeback_tests;
#[path = "v0/stage16/plan/stage16_25_deep_review_round5_tests.rs"]
mod stage16_25_deep_review_round5_tests;
#[path = "v0/stage16/plan/stage16_29_typeck_on_closure_mir_tests.rs"]
mod stage16_29_typeck_on_closure_mir_tests;
#[path = "v0/stage16/plan/stage16_30_closure_call_codegen_tests.rs"]
mod stage16_30_closure_call_codegen_tests;
#[path = "v0/stage16/plan/stage16_31_borrowck_on_closure_mir_tests.rs"]
mod stage16_31_borrowck_on_closure_mir_tests;
#[path = "v0/stage16/plan/stage16_32_triple_nested_closure_tests.rs"]
mod stage16_32_triple_nested_closure_tests;
#[path = "v0/stage16/plan/stage16_33_deep_review_round6_tests.rs"]
mod stage16_33_deep_review_round6_tests;
#[path = "v0/stage16/plan/stage16_34_cleanup_inline_path_tests.rs"]
mod stage16_34_cleanup_inline_path_tests;
#[path = "v0/stage16/plan/stage16_35_codegen_refactoring_tests.rs"]
mod stage16_35_codegen_refactoring_tests;
#[path = "v0/stage16/plan/stage16_36_emitter_cleanup_tests.rs"]
mod stage16_36_emitter_cleanup_tests;
#[path = "v0/stage16/plan/stage16_37_unified_pipeline_tests.rs"]
mod stage16_37_unified_pipeline_tests;
#[path = "v0/stage16/plan/stage16_38_emitter_split_attempt_tests.rs"]
mod stage16_38_emitter_split_attempt_tests;
#[path = "v0/stage16/plan/stage16_39_deep_review_round7_tests.rs"]
mod stage16_39_deep_review_round7_tests;
#[path = "v0/stage16/plan/stage16_40_dead_code_sweep_tests.rs"]
mod stage16_40_dead_code_sweep_tests;
#[path = "v0/stage16/plan/stage16_41_codegen_docs_tests.rs"]
mod stage16_41_codegen_docs_tests;
#[path = "v0/stage16/plan/stage16_42_cleanup_imports_tests.rs"]
mod stage16_42_cleanup_imports_tests;
#[path = "v0/stage16/plan/stage16_43_deep_review_round8_tests.rs"]
mod stage16_43_deep_review_round8_tests;
#[path = "v0/stage16/plan/stage16_44_design_writeback_tests.rs"]
mod stage16_44_design_writeback_tests;
#[path = "v0/stage16/plan/stage16_45_dead_code_audit_tests.rs"]
mod stage16_45_dead_code_audit_tests;
#[path = "v0/stage16/plan/stage16_46_final_cleanup_tests.rs"]
mod stage16_46_final_cleanup_tests;
#[path = "v0/stage16/plan/stage16_47_graph_diagrams_tests.rs"]
mod stage16_47_graph_diagrams_tests;
#[path = "v0/stage16/plan/stage16_48_final_verification_tests.rs"]
mod stage16_48_final_verification_tests;
#[path = "v0/stage16/plan/stage16_49_generic_investigation_tests.rs"]
mod stage16_49_generic_investigation_tests;
#[path = "v0/stage16/plan/stage16_52_aggregate_substs_tests.rs"]
mod stage16_52_aggregate_substs_tests;
#[path = "v0/stage16/plan/stage16_53_substitute_tests.rs"]
mod stage16_53_substitute_tests;
#[path = "v0/stage16/plan/stage16_54_monomorphize_tests.rs"]
mod stage16_54_monomorphize_tests;
#[path = "v0/stage16/plan/stage16_56_nested_generics_tests.rs"]
mod stage16_56_nested_generics_tests;
#[path = "v0/stage16/plan/stage16_58_codegen_integration_tests.rs"]
mod stage16_58_codegen_integration_tests;
#[path = "v0/stage16/plan/stage16_60_design_writeback_tests.rs"]
mod stage16_60_design_writeback_tests;
#[path = "v0/stage16/plan/stage16_65_object_safety_driver_tests.rs"]
mod stage16_65_object_safety_driver_tests;
#[path = "v0/stage16/plan/stage16_69_assoc_type_driver_tests.rs"]
mod stage16_69_assoc_type_driver_tests;

// === Stage 18: Systematic testing ===
#[path = "v0/stage18/plan/stage18_49_stability_tests.rs"]
mod stage18_49_stability_tests;
#[path = "v0/stage18/plan/stage18_50_phase_integration_tests.rs"]
mod stage18_50_phase_integration_tests;
#[path = "v0/stage18/plan/stage18_51_fuzz_tests.rs"]
mod stage18_51_fuzz_tests;
#[path = "v0/stage18/plan/stage18_52_gats_tests.rs"]
mod stage18_52_gats_tests;
#[path = "v0/stage18/plan/stage18_53_gats_phase2_tests.rs"]
mod stage18_53_gats_phase2_tests;
#[path = "v0/stage18/plan/stage18_54_generic_param_tests.rs"]
mod stage18_54_generic_param_tests;
#[path = "v0/stage18/plan/stage18_55_gats_phase3_e2e_tests.rs"]
mod stage18_55_gats_phase3_e2e_tests;
#[path = "v0/stage18/plan/stage18_56_pipeline_audit_fixes_tests.rs"]
mod stage18_56_pipeline_audit_fixes_tests;
#[path = "v0/stage18/plan/stage18_57_span_dummy_cleanup_tests.rs"]
mod stage18_57_span_dummy_cleanup_tests;
#[path = "v0/stage18/plan/stage18_58_error_code_refinement_tests.rs"]
mod stage18_58_error_code_refinement_tests;

// === Stage 18.98-18.103: Monomorphization Tests (relocated from stage2) ===
#[path = "v0/stage18/plan/stage18_98_103_monomorphization_tests.rs"]
mod stage18_98_103_monomorphization_tests;

// === Stage 18.152: Multi-file Module Loader Tests (TD-SINGLE-FILE Phase 1) ===
#[path = "v0/stage18/plan/stage18_152_module_loader_tests.rs"]
mod stage18_152_module_loader_tests;

// === Stage 18.153: Cross-file Name Resolution Tests (TD-SINGLE-FILE Phase 2) ===
#[path = "v0/stage18/plan/stage18_153_cross_file_resolution_tests.rs"]
mod stage18_153_cross_file_resolution_tests;

// === Stage 18.154: landinc CLI Logic Tests (TD-SINGLE-FILE Phase 3) ===
#[path = "v0/stage18/plan/stage18_154_landinc_cli_tests.rs"]
mod stage18_154_landinc_cli_tests;

// === Stage 18.155: mini-cargo Deficiency Fix Tests (TD-SINGLE-FILE Phase 4) ===
#[path = "v0/stage18/plan/stage18_155_deficiency_fix_tests.rs"]
mod stage18_155_deficiency_fix_tests;

// === Stage 18.156: landinc build --bin Tests (缺陷1 fix) ===
#[path = "v0/stage18/plan/stage18_156_build_bin_tests.rs"]
mod stage18_156_build_bin_tests;

// === Stage 18.160: Negative Test Coverage (TD-NEGATIVE-TEST-COVERAGE) ===
#[path = "v0/stage18/plan/stage18_160_codegen_negative_tests.rs"]
mod stage18_160_codegen_negative_tests;
#[path = "v0/stage18/plan/stage18_160_module_loader_negative_tests.rs"]
mod stage18_160_module_loader_negative_tests;
#[path = "v0/stage18/plan/stage18_160_parser_lexer_negative_tests.rs"]
mod stage18_160_parser_lexer_negative_tests;
#[path = "v0/stage18/plan/stage18_160_typeck_negative_tests.rs"]
mod stage18_160_typeck_negative_tests;

// === Stage 18.161: Extended Negative Test Coverage ===
#[path = "v0/stage18/plan/stage18_161_borrowck_negative_tests.rs"]
mod stage18_161_borrowck_negative_tests;
#[path = "v0/stage18/plan/stage18_161_hir_lower_negative_tests.rs"]
mod stage18_161_hir_lower_negative_tests;
#[path = "v0/stage18/plan/stage18_161_mir_lower_negative_tests.rs"]
mod stage18_161_mir_lower_negative_tests;
#[path = "v0/stage18/plan/stage18_161_trait_resolve_negative_tests.rs"]
mod stage18_161_trait_resolve_negative_tests;

// === Stage 18.162: Stdlib + Codegen + Attribute/Macro Negative Tests ===
#[path = "v0/stage18/plan/stage18_162_attribute_macro_negative_tests.rs"]
mod stage18_162_attribute_macro_negative_tests;
#[path = "v0/stage18/plan/stage18_162_codegen_llvm_negative_tests.rs"]
mod stage18_162_codegen_llvm_negative_tests;
#[path = "v0/stage18/plan/stage18_162_stdlib_negative_tests.rs"]
mod stage18_162_stdlib_negative_tests;

// === Stage 18.164: Vtable/Closure/Generics Negative Tests ===
#[path = "v0/stage18/plan/stage18_164_closure_negative_tests.rs"]
mod stage18_164_closure_negative_tests;
#[path = "v0/stage18/plan/stage18_164_generics_mono_negative_tests.rs"]
mod stage18_164_generics_mono_negative_tests;
#[path = "v0/stage18/plan/stage18_164_vtable_negative_tests.rs"]
mod stage18_164_vtable_negative_tests;

// === Stage 18.178: Heap Allocation Infrastructure (TD-HEAP-ALLOC) ===
#[path = "v0/stage18/plan/stage18_178_heap_alloc_tests.rs"]
mod stage18_178_heap_alloc_tests;

// === Stage 18.179: Box<T> MVP (TD-HEAP-ALLOC continued) ===
#[path = "v0/stage18/plan/stage18_179_box_mvp_tests.rs"]
mod stage18_179_box_mvp_tests;

// === Stage 18.180: Real String type (TD-STRING-AS-STR-ALIAS fix) ===
#[path = "v0/stage18/plan/stage18_180_real_string_tests.rs"]
mod stage18_180_real_string_tests;

// === Stage 18.182: Array index codegen fix (TD-ARRAY-INDEX-CODEGEN P0) ===
#[path = "v0/stage18/plan/stage18_182_array_index_tests.rs"]
mod stage18_182_array_index_tests;

// === Stage 18.183: Fat pointer Index projection (TD-FAT-PTR-INDEX-PROJ) ===
#[path = "v0/stage18/plan/stage18_183_fat_ptr_index_tests.rs"]
mod stage18_183_fat_ptr_index_tests;

// === Stage 18.184: str methods runtime fix (TD-STR-METHODS-RUNTIME) ===
#[path = "v0/stage18/plan/stage18_184_str_methods_tests.rs"]
mod stage18_184_str_methods_tests;

// === Stage 18.185: String intrinsics (TD-STRING-INTRINSICS) ===
#[path = "v0/stage18/plan/stage18_185_string_intrinsics_tests.rs"]
mod stage18_185_string_intrinsics_tests;

// === Stage 18.186: format! macro MVP (TD-FORMAT-MACRO) ===
#[path = "v0/stage18/plan/stage18_186_format_macro_tests.rs"]
mod stage18_186_format_macro_tests;

// === Stage 18.188: String::new + function redefine bug fix ===
#[path = "v0/stage18/plan/stage18_188_string_new_tests.rs"]
mod stage18_188_string_new_tests;

// === Stage 18.189: Box::new + String::as_str ===
#[path = "v0/stage18/plan/stage18_189_box_new_as_str_tests.rs"]
mod stage18_189_box_new_as_str_tests;

// === Stage 18.194: Realloc infrastructure ===
#[path = "v0/stage18/plan/stage18_194_realloc_tests.rs"]
mod stage18_194_realloc_tests;

// === Stage 18.195: Vec<T> MVP ===
#[path = "v0/stage18/plan/stage18_195_vec_mvp_tests.rs"]
mod stage18_195_vec_mvp_tests;

// === Stage 18.197: Vec::push ===
#[path = "v0/stage18/plan/stage18_197_vec_push_tests.rs"]
mod stage18_197_vec_push_tests;

// === Stage 18.198: String::push_str ===
#[path = "v0/stage18/plan/stage18_198_push_str_tests.rs"]
mod stage18_198_push_str_tests;

// === Stage 18.200: Vec::get ===
#[path = "v0/stage18/plan/stage18_200_vec_get_tests.rs"]
mod stage18_200_vec_get_tests;

// === Stage 18.203: unified elem_size inference ===
#[path = "v0/stage18/plan/stage18_203_elem_size_tests.rs"]
mod stage18_203_elem_size_tests;

// === Stage 18.205: TD-FUNCTION-REDEFINE-PARAMS fix (format! method calls) ===
#[path = "v0/stage18/plan/stage18_205_format_method_tests.rs"]
mod stage18_205_format_method_tests;

// === Stage 18.206: ABI contract tests for C runtime helpers ===
#[path = "v0/stage18/plan/stage18_206_abi_contract_tests.rs"]
mod stage18_206_abi_contract_tests;

// === Stage 18.208: TD-VEC-GET-TYPE-INFERENCE fix (Vec<Point>::get field access) ===
#[path = "v0/stage18/plan/stage18_208_vec_get_type_tests.rs"]
mod stage18_208_vec_get_type_tests;

// === Stage 18.212: TD-TUPLE-CTOR-TYPECK fix (Box<T> element type) ===
#[path = "v0/stage18/plan/stage18_212_box_typeck_tests.rs"]
mod stage18_212_box_typeck_tests;

// === Stage 18.234: TD-METHOD-RESOLVE-STRICT fix (deferred method resolution) ===
#[path = "v0/stage18/plan/stage18_234_method_resolve_tests.rs"]
mod stage18_234_method_resolve_tests;

// === Stage 18.236: Pointer arithmetic language feature ===
#[path = "v0/stage18/plan/stage18_236_ptr_arith_tests.rs"]
mod stage18_236_ptr_arith_tests;

// === Stage 18.241: str method resolution (primitive type impl MVP) ===
#[path = "v0/stage18/plan/stage18_241_str_method_resolve_tests.rs"]
mod stage18_241_str_method_resolve_tests;

// === Stage 18.284: TD-INTRINSIC-OVERUSE Phase 2-A — primitive type method resolution ===
#[path = "v0/stage18/plan/stage18_284_primitive_intrinsics_tests.rs"]
mod stage18_284_primitive_intrinsics_tests;

// === Stage 18.285: TD-INTRINSIC-OVERUSE Phase 2-A continuation — primitive impl generality ===
#[path = "v0/stage18/plan/stage18_285_primitive_impl_generality_tests.rs"]
mod stage18_285_primitive_impl_generality_tests;

// === Stage 18.286: TD-IF-RETURN-VALUE-CODEGEN fix — const_prop merge point ===
#[path = "v0/stage18/plan/stage18_286_const_prop_merge_tests.rs"]
mod stage18_286_const_prop_merge_tests;

// === Stage 18.287: TD-NEGOVERFLOW-I32 + TD-BINOP-SELF-SEGFAULT fix — typed const emit ===
#[path = "v0/stage18/plan/stage18_287_typed_const_emit_tests.rs"]
mod stage18_287_typed_const_emit_tests;

// === Stage 18.288: §17.6 audit — TD-DIVZERO-CONST-TYPE + TD-SHIFTOVERFLOW-CONST-TYPE ===
#[path = "v0/stage18/plan/stage18_288_audit_div_shl_const_type_tests.rs"]
mod stage18_288_audit_div_shl_const_type_tests;

// === Stage 18.292: 类 Rust 架构修正 — inherent impl 冲突检测 (不允许覆盖) ===
#[path = "v0/stage18/plan/stage18_292_rust_model_inherent_conflict_tests.rs"]
mod stage18_292_rust_model_inherent_conflict_tests;

// === Stage 18.296: trait impl for primitive types — 正负测试 ===
#[path = "v0/stage18/plan/stage18_296_trait_impl_primitive_tests.rs"]
mod stage18_296_trait_impl_primitive_tests;

// === Stage 18.255: TD-TUPLE-CTOR-TYPECK Phase 1 fix + Phase 2 design ===
#[path = "v0/stage18/plan/stage18_255_td_tuple_ctor_typeck_regression_tests.rs"]
mod stage18_255_td_tuple_ctor_typeck_regression_tests;

// === Stage 18.256: Phase 2a — expected_ty param scaffolding (additive) ===
#[path = "v0/stage18/plan/stage18_256_phase2a_scaffolding_tests.rs"]
mod stage18_256_phase2a_scaffolding_tests;

// === Stage 18.259: TD-UNIFY-ARG-ORDER batch fix (5 sites in typeck/check.rs) ===
#[path = "v0/stage18/plan/stage18_259_td_unify_arg_order_regression_tests.rs"]
mod stage18_259_td_unify_arg_order_regression_tests;

// === Stage 18.260: Phase 2d-2f gap analysis (verify soundness hole fully closed) ===
#[path = "v0/stage18/plan/stage18_260_phase2d_2f_gap_analysis_tests.rs"]
mod stage18_260_phase2d_2f_gap_analysis_tests;

// === Stage 18.262: TD-TUPLE-CTOR-CALL-ARG Phase 2e fix (fn_sigs in MIR lower) ===
#[path = "v0/stage18/plan/stage18_262_phase2e_regression_tests.rs"]
mod stage18_262_phase2e_regression_tests;

// === Stage 18.264: Holistic soundness audit (per §17.6 — similar bugs hide together) ===
#[path = "v0/stage18/plan/stage18_264_holistic_soundness_audit_tests.rs"]
mod stage18_264_holistic_soundness_audit_tests;

// === Stage 18.264: Struct literal field + Box::new expected-ty regression tests ===
#[path = "v0/stage18/plan/stage18_264_struct_literal_and_box_new_regression_tests.rs"]
mod stage18_264_struct_literal_and_box_new_regression_tests;

// === Stage 18.267: Continued holistic audit per §17.6 (直到审查不出问题为止) ===
#[path = "v0/stage18/plan/stage18_267_continued_holistic_audit_tests.rs"]
mod stage18_267_continued_holistic_audit_tests;

// === Stage 18.267: Extended holistic audit — generic enum variants (Option/Result) ===
#[path = "v0/stage18/plan/stage18_267_generic_enum_audit_tests.rs"]
mod stage18_267_generic_enum_audit_tests;

// === Stage 18.267: TD-ENUM-VARIANT-CTOR-EXPECTED-TY regression tests ===
#[path = "v0/stage18/plan/stage18_267_enum_variant_ctor_regression_tests.rs"]
mod stage18_267_enum_variant_ctor_regression_tests;

// === Stage 18.268: Continued holistic audit Round 3 (per §17.6) ===
#[path = "v0/stage18/plan/stage18_268_audit_round3_tests.rs"]
mod stage18_268_audit_round3_tests;

// === Stage 18.270: TD-GENERIC-FN-RETURN-EXPECTED-TY Phase 2d complete fix ===
#[path = "v0/stage18/plan/stage18_270_fn_return_expected_ty_regression_tests.rs"]
mod stage18_270_fn_return_expected_ty_regression_tests;

// === Stage 18.271: Final comprehensive soundness audit (per §17.6) ===
#[path = "v0/stage18/plan/stage18_271_final_comprehensive_audit_tests.rs"]
mod stage18_271_final_comprehensive_audit_tests;

// === Stage 18.282: TD-DROP-MOVED-LOCALS full — flow-sensitive move tracking ===
#[path = "v0/stage18/plan/stage18_282_moved_state_tests.rs"]
mod stage18_282_moved_state_tests;

// === Stage 18.323: TD-CODEGEN-NEGATIVE — codegen negative test coverage expansion ===
#[path = "v0/stage18/plan/stage18_323_codegen_negative_coverage_tests.rs"]
mod stage18_323_codegen_negative_coverage_tests;

// === Stage 18.324: TD-CODEGEN-NEGATIVE continued — +30 codegen negative tests ===
#[path = "v0/stage18/plan/stage18_324_codegen_negative_expansion_tests.rs"]
mod stage18_324_codegen_negative_expansion_tests;

// === Stage 18.325: TD-CODEGEN-NEGATIVE final push — +60 codegen negative tests ===
#[path = "v0/stage18/plan/stage18_325_codegen_negative_final_push_tests.rs"]
mod stage18_325_codegen_negative_final_push_tests;

// === Stage 18.332: P1 soundness fix — sret ABI support for LLVMSysEmitter ===
#[path = "v0/stage18/plan/stage18_332_sret_abi_tests.rs"]
mod stage18_332_sret_abi_tests;

// === Stage 18.333: P1 soundness fix — byval ABI support for large struct/array params ===
#[path = "v0/stage18/plan/stage18_333_byval_abi_tests.rs"]
mod stage18_333_byval_abi_tests;

// === Stage 18.334: P1 soundness fix — TextEmitter sret syntax + sret load + variadic detection + llvm-as smoke test ===
#[path = "v0/stage18/plan/stage18_334_text_ir_tests.rs"]
mod stage18_334_text_ir_tests;

// === Stage 18.335: P1 soundness fix — ZST param skip + __landin_eprintf declare + drop_glue declare removal ===
#[path = "v0/stage18/plan/stage18_335_zst_drop_eprintf_tests.rs"]
mod stage18_335_zst_drop_eprintf_tests;

// === Stage 18.336: P1 soundness fix — ZST nested aggregate Void leak + typeck return/trait gaps ===
#[path = "v0/stage18/plan/stage18_336_zst_aggregate_typeck_tests.rs"]
mod stage18_336_zst_aggregate_typeck_tests;

// === Stage 18.337: P1 soundness fix — Recursive struct stack overflow + pointer-to-Adt GEP ===
#[path = "v0/stage18/plan/stage18_337_recursive_struct_tests.rs"]
mod stage18_337_recursive_struct_tests;

// === Stage 18.347: P2 soundness fix — Generic struct field access type substitution ===
#[path = "v0/stage18/plan/stage18_347_generic_struct_field_access_tests.rs"]
mod stage18_347_generic_struct_field_access_tests;

// === Stage 18.348: P2 soundness fix — Pre-codegen param_check diagnostic pass ===
#[path = "v0/stage18/plan/stage18_348_param_check_tests.rs"]
mod stage18_348_param_check_tests;

// === Stage 18.351: P2 soundness fix — Recursive Param detection + typeck subst ===
#[path = "v0/stage18/plan/stage18_351_recursive_param_tests.rs"]
mod stage18_351_recursive_param_tests;

// === Stage 18.416: §20 iterative audit — BitAnd/BitOr/BitXor type check ===
#[path = "v0/stage18/plan/stage18_416_bitwise_type_check_tests.rs"]
mod stage18_416_bitwise_type_check_tests;

// === Stage 18.420: §20 iterative audit — Field access syntax validation ===
#[path = "v0/stage18/plan/stage18_420_field_access_syntax_tests.rs"]
mod stage18_420_field_access_syntax_tests;

// === Stage 18.422: §20 iterative audit — &str indexing rejection ===
#[path = "v0/stage18/plan/stage18_422_str_index_rejection_tests.rs"]
mod stage18_422_str_index_rejection_tests;

// === Stage 18.424-18.425: §20 iterative audit — Index typeck + assignment path ===
#[path = "v0/stage18/plan/stage18_424_425_index_typeck_tests.rs"]
mod stage18_424_425_index_typeck_tests;

// === Stage 18.426: §20 iterative audit — Cast validity check ===
#[path = "v0/stage18/plan/stage18_426_cast_validity_tests.rs"]
mod stage18_426_cast_validity_tests;

// === Stage 18.428: §20 iterative audit — Deref validity check ===
#[path = "v0/stage18/plan/stage18_428_deref_validity_tests.rs"]
mod stage18_428_deref_validity_tests;

// === Stage 18.432: §20 iterative audit — Non-exhaustive match check ===
#[path = "v0/stage18/plan/stage18_432_non_exhaustive_match_tests.rs"]
mod stage18_432_non_exhaustive_match_tests;

// === Stage 18.85: Fuzz/Stress Tests ===
#[path = "fuzz/fuzz_harness.rs"]
mod fuzz_harness;

// === Stage 21.1: v0.5 GATs P2 Phase 1 — E2E tests ===
#[path = "v0/stage21/plan/stage21_01_gats_e2e_tests.rs"]
mod stage21_01_gats_e2e_tests;

// === Stage 30.2 (v0.13 TD-STUB-LIFETIME-ELISION-NOOP): Rule 4 enforcement ===
#[path = "v0/stage30/plan/stage30_2_lifetime_elision_rule4_tests.rs"]
mod stage30_2_lifetime_elision_rule4_tests;

// === Stage 30.3 (v0.13 TD-STUB-DROP-ELABORATION-NOOP): Reclassification ===
#[path = "v0/stage30/plan/stage30_3_drop_elaboration_reclassification_tests.rs"]
mod stage30_3_drop_elaboration_reclassification_tests;
