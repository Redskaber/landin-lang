//! Landin compiler — unified test entry point.
//!
//! Stage 13.27: Cleaned up — only behavioral test files are included.
//! All doc-existence tests removed per user feedback.
//! Stage 9 and Stage 10 test files were entirely doc-existence checks
//! (checking file existence, reading source files for content patterns,
//! checking Cargo.toml version strings) — all removed.

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
