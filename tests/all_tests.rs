//! Landin compiler — unified test entry point.
//!
//! Per stage-committee-process.md v3.18 §17.1, all test files live under
//! `tests/v0/stage{N}/plan/`. This file is the single `[[test]]` target
//! registered in `Cargo.toml` (with `autotests = false`); it pulls in
//! every organized test file via `#[path]` module declarations.
//!
//! Benefits over per-file `[[test]]` entries:
//! - `Cargo.toml` stays compact (one `[[test]]`, not 19+)
//! - Single test binary → faster incremental compilation
//! - Each test file is still isolated in its own `mod` — no name conflicts
//! - Adding a new test file = one `#[path]` line here, no Cargo.toml edit
//!
//! To run all tests:  `cargo test`
//! To run one module: `cargo test --test all_tests -- lexer_tests`

// ============================================================
// Stage 0 — Lexer / Parser / AST
// ============================================================

#[path = "v0/stage0/plan/lexer_tests.rs"]
mod lexer_tests;

#[path = "v0/stage0/plan/parser_tests.rs"]
mod parser_tests;

#[path = "v0/stage0/plan/ast_structure_tests.rs"]
mod ast_structure_tests;

// ============================================================
// Stage 1 — HIR + Name Resolution
// ============================================================

#[path = "v0/stage1/plan/hir_structure_tests.rs"]
mod hir_structure_tests;

#[path = "v0/stage1/plan/hir_lowering_tests.rs"]
mod hir_lowering_tests;

#[path = "v0/stage1/plan/hir_resolution_tests.rs"]
mod hir_resolution_tests;

#[path = "v0/stage1/plan/hir_scope_resolution_tests.rs"]
mod hir_scope_resolution_tests;

// ============================================================
// Stage 2 — MIR + Typeck + Borrowck
// ============================================================

#[path = "v0/stage2/plan/mir_lowering_tests.rs"]
mod mir_lowering_tests;

#[path = "v0/stage2/plan/typeck_tests.rs"]
mod typeck_tests;

#[path = "v0/stage2/plan/integration_tests.rs"]
mod integration_tests;

#[path = "v0/stage2/plan/negative_cases_tests.rs"]
mod negative_cases_tests;

// ============================================================
// Stage 3 — LLVM Codegen
// ============================================================

#[path = "v0/stage3/plan/codegen_tests.rs"]
mod codegen_tests;

#[path = "v0/stage3/plan/deep_inspection_tests.rs"]
mod deep_inspection_tests;

// ============================================================
// Stage 4 — Modules / Closures / Macros / Visibility
// ============================================================

#[path = "v0/stage4/plan/closure_capture_tests.rs"]
mod closure_capture_tests;

#[path = "v0/stage4/plan/closure_call_tests.rs"]
mod closure_call_tests;

#[path = "v0/stage4/plan/closure_full_call_tests.rs"]
mod closure_full_call_tests;

#[path = "v0/stage4/plan/macro_system_tests.rs"]
mod macro_system_tests;

#[path = "v0/stage4/plan/visibility_tests.rs"]
mod visibility_tests;

// ============================================================
// Stage 5 — TraitResolver + Vtable
// ============================================================

#[path = "v0/stage5/plan/trait_resolver_tests.rs"]
mod trait_resolver_tests;

#[path = "v0/stage5/plan/trait_integration_tests.rs"]
mod trait_integration_tests;

#[path = "v0/stage5/plan/ty_is_copy_tests.rs"]
mod ty_is_copy_tests;

#[path = "v0/stage5/plan/def_id_name_map_tests.rs"]
mod def_id_name_map_tests;

#[path = "v0/stage5/plan/vtable_tests.rs"]
mod vtable_tests;

#[path = "v0/stage5/plan/vtable_codegen_tests.rs"]
mod vtable_codegen_tests;

#[path = "v0/stage5/plan/dyn_trait_ptr_tests.rs"]
mod dyn_trait_ptr_tests;

#[path = "v0/stage5/plan/builtin_traits_tests.rs"]
mod builtin_traits_tests;

#[path = "v0/stage5/plan/builtin_copy_activation_tests.rs"]
mod builtin_copy_activation_tests;

#[path = "v0/stage5/plan/builtin_clone_drop_tests.rs"]
mod builtin_clone_drop_tests;

#[path = "v0/stage5/plan/primitive_copy_tests.rs"]
mod primitive_copy_tests;

#[path = "v0/stage5/plan/copy_unification_tests.rs"]
mod copy_unification_tests;

#[path = "v0/stage5/plan/trait_impl_stats_tests.rs"]
mod trait_impl_stats_tests;

#[path = "v0/stage5/plan/trait_method_query_tests.rs"]
mod trait_method_query_tests;

#[path = "v0/stage5/plan/trait_hierarchy_tests.rs"]
mod trait_hierarchy_tests;

#[path = "v0/stage5/plan/trait_summary_tests.rs"]
mod trait_summary_tests;

#[path = "v0/stage5/plan/vtable_method_resolve_tests.rs"]
mod vtable_method_resolve_tests;

#[path = "v0/stage5/plan/trait_coherence_tests.rs"]
mod trait_coherence_tests;

#[path = "v0/stage5/plan/impl_completeness_tests.rs"]
mod impl_completeness_tests;

#[path = "v0/stage5/plan/impl_validation_tests.rs"]
mod impl_validation_tests;

#[path = "v0/stage5/plan/driver_validation_tests.rs"]
mod driver_validation_tests;

#[path = "v0/stage5/plan/mini_cargo_tests.rs"]
mod mini_cargo_tests;

#[path = "v0/stage5/plan/stdlib_mvp_tests.rs"]
mod stdlib_mvp_tests;

#[path = "v0/stage5/plan/driver_stdlib_tests.rs"]
mod driver_stdlib_tests;

#[path = "v0/stage5/plan/stdlib_alloc_tests.rs"]
mod stdlib_alloc_tests;

#[path = "v0/stage5/plan/stdlib_layer_tests.rs"]
mod stdlib_layer_tests;

#[path = "v0/stage5/plan/stdlib_std_tests.rs"]
mod stdlib_std_tests;

#[path = "v0/stage5/plan/stdlib_facade_tests.rs"]
mod stdlib_facade_tests;

#[path = "v0/stage5/plan/facade_integration_tests.rs"]
mod facade_integration_tests;

#[path = "v0/stage5/plan/stdlib_type_resolve_tests.rs"]
mod stdlib_type_resolve_tests;

#[path = "v0/stage5/plan/stdlib_layout_tests.rs"]
mod stdlib_layout_tests;

#[path = "v0/stage5/plan/stdlib_trait_method_tests.rs"]
mod stdlib_trait_method_tests;

#[path = "v0/stage5/plan/stdlib_vtable_layout_tests.rs"]
mod stdlib_vtable_layout_tests;

#[path = "v0/stage5/plan/stdlib_vtable_size_tests.rs"]
mod stdlib_vtable_size_tests;

#[path = "v0/stage5/plan/stdlib_vtable_plan_tests.rs"]
mod stdlib_vtable_plan_tests;

#[path = "v0/stage5/plan/stdlib_vtable_symbol_tests.rs"]
mod stdlib_vtable_symbol_tests;

#[path = "v0/stage5/plan/stdlib_vtable_emission_tests.rs"]
mod stdlib_vtable_emission_tests;

#[path = "v0/stage5/plan/stdlib_vtable_emission_summary_tests.rs"]
mod stdlib_vtable_emission_summary_tests;

#[path = "v0/stage5/plan/codegen_vtable_emission_helper_tests.rs"]
mod codegen_vtable_emission_helper_tests;

#[path = "v0/stage5/plan/codegen_vtable_global_text_tests.rs"]
mod codegen_vtable_global_text_tests;

#[path = "v0/stage5/plan/codegen_vtable_batch_tests.rs"]
mod codegen_vtable_batch_tests;

#[path = "v0/stage5/plan/codegen_vtable_spec_builder_tests.rs"]
mod codegen_vtable_spec_builder_tests;

#[path = "v0/stage5/plan/codegen_vtable_orchestrator_tests.rs"]
mod codegen_vtable_orchestrator_tests;

#[path = "v0/stage5/plan/codegen_dynptr_text_tests.rs"]
mod codegen_dynptr_text_tests;

#[path = "v0/stage5/plan/codegen_dynptr_spec_builder_tests.rs"]
mod codegen_dynptr_spec_builder_tests;

#[path = "v0/stage5/plan/codegen_dynptr_orchestrator_tests.rs"]
mod codegen_dynptr_orchestrator_tests;

#[path = "v0/stage5/plan/codegen_combined_orchestrator_tests.rs"]
mod codegen_combined_orchestrator_tests;

#[path = "v0/stage5/plan/codegen_trait_dispatch_summary_tests.rs"]
mod codegen_trait_dispatch_summary_tests;

#[path = "v0/stage5/plan/codegen_trait_dispatch_plan_tests.rs"]
mod codegen_trait_dispatch_plan_tests;

#[path = "v0/stage5/plan/codegen_plan_orchestrator_tests.rs"]
mod codegen_plan_orchestrator_tests;

#[path = "v0/stage5/plan/codegen_text_batch_tests.rs"]
mod codegen_text_batch_tests;

#[path = "v0/stage5/plan/codegen_text_batch_from_resolver_tests.rs"]
mod codegen_text_batch_from_resolver_tests;

#[path = "v0/stage5/plan/text_emitter_vtable_delegation_tests.rs"]
mod text_emitter_vtable_delegation_tests;

#[path = "v0/stage5/plan/text_emitter_dynptr_delegation_tests.rs"]
mod text_emitter_dynptr_delegation_tests;

#[path = "v0/stage5/plan/emit_vtables_delegation_tests.rs"]
mod emit_vtables_delegation_tests;

#[path = "v0/stage5/plan/emit_dyn_trait_ptrs_delegation_tests.rs"]
mod emit_dyn_trait_ptrs_delegation_tests;

#[path = "v0/stage5/plan/dyn_trait_fat_ptr_tests.rs"]
mod dyn_trait_fat_ptr_tests;

#[path = "v0/stage5/plan/dyn_trait_fat_ptr_builder_tests.rs"]
mod dyn_trait_fat_ptr_builder_tests;

#[path = "v0/stage5/plan/dyn_trait_fat_ptr_text_tests.rs"]
mod dyn_trait_fat_ptr_text_tests;

#[path = "v0/stage5/plan/dyn_trait_fat_ptr_batch_tests.rs"]
mod dyn_trait_fat_ptr_batch_tests;

#[path = "v0/stage5/plan/dyn_trait_fat_ptr_from_resolver_tests.rs"]
mod dyn_trait_fat_ptr_from_resolver_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_tests.rs"]
mod dyn_trait_method_call_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_text_tests.rs"]
mod dyn_trait_method_call_text_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_builder_tests.rs"]
mod dyn_trait_method_call_builder_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_batch_tests.rs"]
mod dyn_trait_method_call_batch_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_from_resolver_tests.rs"]
mod dyn_trait_method_call_from_resolver_tests;

#[path = "v0/stage5/plan/dyn_trait_mir_summary_tests.rs"]
mod dyn_trait_mir_summary_tests;

#[path = "v0/stage5/plan/dyn_trait_mir_summary_from_resolver_tests.rs"]
mod dyn_trait_mir_summary_from_resolver_tests;

#[path = "v0/stage5/plan/dyn_trait_mir_plan_tests.rs"]
mod dyn_trait_mir_plan_tests;

#[path = "v0/stage5/plan/dyn_trait_mir_plan_text_tests.rs"]
mod dyn_trait_mir_plan_text_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_in_plan_tests.rs"]
mod dyn_trait_method_call_in_plan_tests;

#[path = "v0/stage5/plan/mir_lower_dyn_trait_plan_context_tests.rs"]
mod mir_lower_dyn_trait_plan_context_tests;

#[path = "v0/stage5/plan/dyn_trait_method_call_in_plan_by_method_tests.rs"]
mod dyn_trait_method_call_in_plan_by_method_tests;

#[path = "v0/stage5/plan/mir_lower_dyn_trait_method_call_integration_tests.rs"]
mod mir_lower_dyn_trait_method_call_integration_tests;

#[path = "v0/stage5/plan/codegen_dyn_trait_method_call_tests.rs"]
mod codegen_dyn_trait_method_call_tests;

#[path = "v0/stage5/plan/driver_dyn_trait_plan_integration_tests.rs"]
mod driver_dyn_trait_plan_integration_tests;

#[path = "v0/stage5/plan/dyn_trait_return_kind_tests.rs"]
mod dyn_trait_return_kind_tests;

#[path = "v0/stage5/plan/dyn_trait_e2e_integration_tests.rs"]
mod dyn_trait_e2e_integration_tests;

#[path = "v0/stage5/plan/dyn_trait_param_kinds_tests.rs"]
mod dyn_trait_param_kinds_tests;

#[path = "v0/stage5/plan/is_stdlib_trait_tests.rs"]
mod is_stdlib_trait_tests;

#[path = "v0/stage5/plan/stdlib_trait_count_tests.rs"]
mod stdlib_trait_count_tests;

#[path = "v0/stage5/plan/stdlib_marker_traits_tests.rs"]
mod stdlib_marker_traits_tests;

#[path = "v0/stage5/plan/stdlib_arithmetic_traits_tests.rs"]
mod stdlib_arithmetic_traits_tests;

#[path = "v0/stage5/plan/stdlib_core_traits_tests.rs"]
mod stdlib_core_traits_tests;

#[path = "v0/stage5/plan/stdlib_io_unary_traits_tests.rs"]
mod stdlib_io_unary_traits_tests;

#[path = "v0/stage5/plan/stdlib_param_kinds_accuracy_tests.rs"]
mod stdlib_param_kinds_accuracy_tests;

#[path = "v0/stage5/plan/stdlib_trait_method_accessors_tests.rs"]
mod stdlib_trait_method_accessors_tests;

#[path = "v0/stage5/plan/stdlib_trait_method_accessors_2_tests.rs"]
mod stdlib_trait_method_accessors_2_tests;

#[path = "v0/stage5/plan/stdlib_trait_methods_by_self_kind_tests.rs"]
mod stdlib_trait_methods_by_self_kind_tests;

#[path = "v0/stage5/plan/stdlib_trait_methods_by_return_kind_tests.rs"]
mod stdlib_trait_methods_by_return_kind_tests;

#[path = "v0/stage5/plan/stdlib_trait_methods_by_is_unsafe_tests.rs"]
mod stdlib_trait_methods_by_is_unsafe_tests;

#[path = "v0/stage5/plan/stdlib_trait_methods_by_param_count_tests.rs"]
mod stdlib_trait_methods_by_param_count_tests;

// Stage 7 (TD-015): Region inference tests
#[path = "v0/stage7/plan/region_inference_tests.rs"]
mod region_inference_tests;

// Stage 7.6 (TD-018): User-defined trait dyn support tests
#[path = "v0/stage7/plan/user_defined_trait_dyn_tests.rs"]
mod user_defined_trait_dyn_tests;

// Stage 7.7 (§25.8): Design writeback verification tests
#[path = "v0/stage7/plan/design_writeback_verification_tests.rs"]
mod design_writeback_verification_tests;

// Stage 7.8 (§25): Deep review verification tests
#[path = "v0/stage7/plan/deep_review_tests.rs"]
mod deep_review_tests;

// Stage 7.9: Systematic review verification tests (v0.14.8)
#[path = "v0/stage7/plan/systematic_review_v014_tests.rs"]
mod systematic_review_v014_tests;

// Stage 8.1: Lifetime elision tests
#[path = "v0/stage8/plan/lifetime_elision_tests.rs"]
mod lifetime_elision_tests;

// Stage 8.2: Object safety tests
#[path = "v0/stage8/plan/object_safety_tests.rs"]
mod object_safety_tests;

// Stage 8.3: extern "C" ABI tests
#[path = "v0/stage8/plan/extern_c_abi_tests.rs"]
mod extern_c_abi_tests;

// Stage 8.4: Drop elaboration tests
#[path = "v0/stage8/plan/drop_elaboration_tests.rs"]
mod drop_elaboration_tests;

// Stage 8.5: async/await tests
#[path = "v0/stage8/plan/async_await_tests.rs"]
mod async_await_tests;

// Stage 8.6: §25.8 design writeback + §25 deep review tests
#[path = "v0/stage8/plan/deep_review_tests.rs"]
mod stage8_6_deep_review_tests;

// ============================================================
// Stage 9.1 — Systematic Review + v0.1 Conformance Kickoff
// ============================================================

// Stage 9.1: systematic review verification (D1-D7 + stage9 setup)
#[path = "v0/stage9/plan/systematic_review_v0156_tests.rs"]
mod stage9_1_systematic_review_v0156_tests;

// Stage 9.2: Operators + Pratt precedence verification
#[path = "v0/stage9/plan/operators_tests.rs"]
mod stage9_2_operators_tests;

// Stage 9.3: Control flow conformance expansion verification
#[path = "v0/stage9/plan/control_flow_tests.rs"]
mod stage9_3_control_flow_tests;

// Stage 9.4: Patterns conformance expansion verification
#[path = "v0/stage9/plan/patterns_tests.rs"]
mod stage9_4_patterns_tests;

// Stage 9.5: Types conformance expansion verification
#[path = "v0/stage9/plan/types_tests.rs"]
mod stage9_5_types_tests;

// Stage 9.6: Attributes conformance expansion verification
#[path = "v0/stage9/plan/attributes_tests.rs"]
mod stage9_6_attributes_tests;

// Stage 9.7: Generics conformance expansion verification
#[path = "v0/stage9/plan/generics_tests.rs"]
mod stage9_7_generics_tests;

// Stage 9.8: Closures conformance expansion verification
#[path = "v0/stage9/plan/closures_tests.rs"]
mod stage9_8_closures_tests;

// Stage 9.9: Modules conformance expansion verification
#[path = "v0/stage9/plan/modules_tests.rs"]
mod stage9_9_modules_tests;

// Stage 9.10: Error recovery conformance expansion verification
#[path = "v0/stage9/plan/error_recovery_tests.rs"]
mod stage9_10_error_recovery_tests;

// Stage 9.11: Realistic programs conformance expansion verification
#[path = "v0/stage9/plan/realistic_programs_tests.rs"]
mod stage9_11_realistic_programs_tests;

// Stage 9.12: v0.1 release candidate verification (§25 deep review)
#[path = "v0/stage9/plan/deep_review_v01_rc_tests.rs"]
mod stage9_12_deep_review_v01_rc_tests;

// v0.1 Gap Analysis verification
#[path = "v0/stage9/plan/v0.1_gap_analysis_tests.rs"]
mod v0_1_gap_analysis_tests;

// Stage 10.0: CLI upgrade + Runner upgrade verification
#[path = "v0/stage10/plan/stage10_0_tests.rs"]
mod stage10_0_tests;

// Stage 10.1: 01-typecheck conformance verification
#[path = "v0/stage10/plan/stage10_1_tests.rs"]
mod stage10_1_tests;

// Stage 10.2: 02-borrowck conformance verification
#[path = "v0/stage10/plan/stage10_2_tests.rs"]
mod stage10_2_tests;

// Stage 10.3: 03-codegen conformance verification
#[path = "v0/stage10/plan/stage10_3_tests.rs"]
mod stage10_3_tests;

// Stage 10.4: 04-e2e conformance verification
#[path = "v0/stage10/plan/stage10_4_tests.rs"]
mod stage10_4_tests;

// Stage 10.5: 05-soundness conformance verification
#[path = "v0/stage10/plan/stage10_5_tests.rs"]
mod stage10_5_tests;
