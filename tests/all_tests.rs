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
