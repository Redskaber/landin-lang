//! Trait-dispatch orchestration — combined vtable+dynptr emission, plan,
//! summary, and text-batch APIs.
//!
//! Stage 14.3 §14.4 architectural split: extracted from the historical
//! `codegen/trait_dispatch.rs` (962 LOC) along the vtable/dynptr/orchestrator
//! boundary. This module owns the **high-level orchestration** that combines
//! vtable emission (`super::vtable`) + dynptr emission (`super::dynptr`)
//! into a single entry point, plus the project-level `EmissionPlan` /
//! `EmissionSummary` aggregates.
//!
//! Per §16: every function below takes pre-built data
//! (`&TraitResolver` / `&CodegenTraitDispatchEmissionPlan`) — no HIR access.
//! Data flows downstream: traits → codegen/trait_dispatch/orchestrator → IR.
//!
//! Per API-naming-standard §3:
//! - `CodegenTraitDispatchEmissionSummary` follows `<Noun><Noun><Noun><Noun><Noun>` pattern.
//! - `CodegenTraitDispatchEmissionPlan` follows `<Noun><Noun><Noun><Noun><Noun>` pattern.
//! - `build_trait_dispatch_emission_summary` follows `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
//! - `emit_vtables_and_dynptrs_from_resolver` follows `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` pattern.

use crate::codegen::trait_dispatch::dynptr::{
    build_dynptr_global_specs, emit_dynptr_global_text, emit_dynptrs_from_resolver,
    StdlibDynptrGlobalSpec,
};
use crate::codegen::trait_dispatch::vtable::{
    build_vtable_global_specs, emit_vtable_global_text, emit_vtables_from_resolver,
    StdlibVtableGlobalSpec,
};
use crate::codegen::Emitter;
use lasso::Rodeo;

// ============================================================================
// Stage 5.51: Codegen vtable + dynptr combined emission orchestrator
//
// Single entry point that composes Stage 5.47's `emit_vtables_from_resolver()`
// + Stage 5.50's `emit_dynptrs_from_resolver()`. Emits ALL trait-dispatch
// globals (vtable + dynptr) in one call.
//
// Stage 5.52 will refactor driver/codegen to call this combined orchestrator
// instead of separately calling `emit_vtables()` + `emit_dyn_trait_ptrs()`.
//
// Per API-naming-standard §3: `emit_vtables_and_dynptrs_from_resolver`
// follows `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` pattern. The `_and_`
// conjunction connects the two noun phrases (vtables + dynptrs).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (same as
// `emit_vtables()` + `emit_dyn_trait_ptrs()`). No `mir::ty` reference, no
// circular dependency.
// ============================================================================

/// Stage 5.51: Emit ALL trait-dispatch globals (vtable + dynptr) by composing
/// `emit_vtables_from_resolver()` (Stage 5.47) + `emit_dynptrs_from_resolver()`
/// (Stage 5.50).
///
/// This is the **single entry point** for codegen to emit all trait-dispatch
/// globals. Stage 5.52 will refactor driver/codegen to call this combined
/// orchestrator instead of separately calling `emit_vtables()` +
/// `emit_dyn_trait_ptrs()`.
///
/// Behavior is **identical** to calling `emit_vtables()` + `emit_dyn_trait_ptrs()`
/// separately — verified by `test_emit_vtables_and_dynptrs_match_separate_calls`.
///
/// Per API-naming-standard §3: `emit_vtables_and_dynptrs_from_resolver`
/// follows `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` pattern. The `_and_`
/// conjunction connects the two noun phrases (vtables + dynptrs).
pub fn emit_vtables_and_dynptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    // Emit vtable globals first (Stage 5.47 orchestrator).
    emit_vtables_from_resolver(trait_resolver, interner, emitter);
    // Then emit dynptr globals (Stage 5.50 orchestrator).
    emit_dynptrs_from_resolver(trait_resolver, interner, emitter);
}

// ============================================================================
// Stage 5.52: Codegen trait-dispatch emission summary
//
// Project-level aggregate statistics for trait-dispatch global emission.
// Counts vtable + dynptr globals, collects deduplicated trait/type names,
// sums total method slots. This is the **codegen counterpart** of Stage
// 5.42's `stdlib_vtable_emission_summary()`, but computed from
// `TraitResolver` rather than from a list of `StdlibVtableEmission`.
//
// Stage 5.53 will use this for codegen diagnostic output ("emit N vtable
// globals, M dynptr globals, K total method slots").
//
// Per API-naming-standard §3:
//   - `CodegenTraitDispatchEmissionSummary` follows
//     `<Noun><Noun><Noun><Noun><Noun>` pattern.
//   - `build_trait_dispatch_emission_summary` follows
//     `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `CodegenTraitDispatchEmissionSummary`. No `mir::ty` / `Emitter`
// reference, no circular dependency.
// ============================================================================

/// Stage 5.52: Project-level trait-dispatch emission statistics.
///
/// Aggregates vtable + dynptr global counts, deduplicated trait/type names,
/// and total method slots from `TraitResolver.vtables`. Useful for:
/// - Codegen diagnostics ("emit N vtable globals, M dynptr globals")
/// - Detecting trait-dispatch bloat (large `total_method_slots`)
/// - Identifying trait-heavy code (many distinct `trait_names`)
///
/// Per API-naming-standard §3: `CodegenTraitDispatchEmissionSummary` follows
/// `<Noun><Noun><Noun><Noun><Noun>` pattern. Field names follow
/// `<noun>_<noun>` / `<adj>_<noun>_<noun>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenTraitDispatchEmissionSummary {
    /// Number of vtable globals to emit (= `TraitResolver.vtables.len()`).
    pub vtable_count: u32,
    /// Number of dynptr globals to emit (= `TraitResolver.vtables.len()`,
    /// one dynptr per (trait, type) pair).
    pub dynptr_count: u32,
    /// Total global count (`vtable_count + dynptr_count`).
    pub total_global_count: u32,
    /// Deduplicated list of trait names involved (resolved via interner,
    /// or "Trait" default if unresolved).
    pub trait_names: Vec<String>,
    /// Deduplicated list of type names involved (resolved via interner,
    /// or "Type" default if unresolved).
    pub type_names: Vec<String>,
    /// Sum of `vtable.entries.len()` across all vtables — total method
    /// slots across all vtable globals.
    pub total_method_slots: u32,
}

/// Stage 5.52: Build a project-level trait-dispatch emission summary from
/// `TraitResolver.vtables`.
///
/// Given a `&TraitResolver` + `&Rodeo`, returns a
/// `CodegenTraitDispatchEmissionSummary` aggregating vtable + dynptr global
/// counts, deduplicated trait/type names, and total method slots.
///
/// Empty `TraitResolver.vtables` returns a summary with all-zero counts and
/// empty name lists.
///
/// Per API-naming-standard §3: `build_trait_dispatch_emission_summary`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn build_trait_dispatch_emission_summary(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> CodegenTraitDispatchEmissionSummary {
    let vtable_count = trait_resolver.vtables.len() as u32;
    let dynptr_count = vtable_count; // one dynptr per (trait, type) pair
    let total_global_count = vtable_count + dynptr_count;

    let mut trait_names: Vec<String> = Vec::new();
    let mut type_names: Vec<String> = Vec::new();
    let mut total_method_slots: u32 = 0;

    for ((trait_name, self_ty_name), vtable) in &trait_resolver.vtables {
        let trait_str = interner
            .try_resolve(trait_name)
            .unwrap_or("Trait")
            .to_string();
        let type_str = interner
            .try_resolve(self_ty_name)
            .unwrap_or("Type")
            .to_string();

        if !trait_names.contains(&trait_str) {
            trait_names.push(trait_str);
        }
        if !type_names.contains(&type_str) {
            type_names.push(type_str);
        }
        total_method_slots += vtable.entries.len() as u32;
    }

    CodegenTraitDispatchEmissionSummary {
        vtable_count,
        dynptr_count,
        total_global_count,
        trait_names,
        type_names,
        total_method_slots,
    }
}

// ============================================================================
// Stage 5.53: Codegen trait-dispatch emission plan (final aggregate)
//
// Single-call API that returns EVERYTHING codegen needs to emit all
// trait-dispatch globals:
//   - vtable_specs (from Stage 5.46 build_vtable_global_specs)
//   - dynptr_specs (from Stage 5.49 build_dynptr_global_specs)
//   - summary (from Stage 5.52 build_trait_dispatch_emission_summary)
//
// This is the **final aggregate API** — Stage 5.54 driver refactor will call
// this plan once, then iterate vtable_specs + dynptr_specs to emit globals,
// and use summary for diagnostic output.
//
// Per API-naming-standard §3:
//   - `CodegenTraitDispatchEmissionPlan` follows
//     `<Noun><Noun><Noun><Noun><Noun>` pattern.
//   - `build_trait_dispatch_emission_plan` follows
//     `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `CodegenTraitDispatchEmissionPlan`. No `mir::ty` / `Emitter`
// reference, no circular dependency.
// ============================================================================

/// Stage 5.53: Everything codegen needs to emit all trait-dispatch globals
/// in one struct.
///
/// Combines:
/// - `vtable_specs` (from Stage 5.46 `build_vtable_global_specs()`)
/// - `dynptr_specs` (from Stage 5.49 `build_dynptr_global_specs()`)
/// - `summary` (from Stage 5.52 `build_trait_dispatch_emission_summary()`)
///
/// Stage 5.54 driver refactor will call `build_trait_dispatch_emission_plan()`
/// once, then iterate `vtable_specs` + `dynptr_specs` to emit globals, and
/// use `summary` for diagnostic output.
///
/// Per API-naming-standard §3: `CodegenTraitDispatchEmissionPlan` follows
/// `<Noun><Noun><Noun><Noun><Noun>` pattern. Field names follow
/// `<noun>_<noun>` / `<noun>` patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenTraitDispatchEmissionPlan {
    /// Vtable global specs — one per (trait, type) pair in
    /// `TraitResolver.vtables`. Used by codegen to emit `@.vtable.*` globals.
    pub vtable_specs: Vec<StdlibVtableGlobalSpec>,
    /// Dynptr global specs — one per (trait, type) pair. Used by codegen to
    /// emit `@.dynptr.*` globals.
    pub dynptr_specs: Vec<StdlibDynptrGlobalSpec>,
    /// Project-level summary — counts + deduplicated names + total slots.
    /// Used by codegen for diagnostic output.
    pub summary: CodegenTraitDispatchEmissionSummary,
}

/// Stage 5.53: Build a complete trait-dispatch emission plan from
/// `TraitResolver.vtables`.
///
/// Given a `&TraitResolver` + `&Rodeo`, returns a
/// `CodegenTraitDispatchEmissionPlan` containing:
/// - `vtable_specs` (from `build_vtable_global_specs()` — Stage 5.46)
/// - `dynptr_specs` (from `build_dynptr_global_specs()` — Stage 5.49)
/// - `summary` (from `build_trait_dispatch_emission_summary()` — Stage 5.52)
///
/// This is the **final aggregate API** — one call returns everything codegen
/// needs to emit all trait-dispatch globals.
///
/// Per API-naming-standard §3: `build_trait_dispatch_emission_plan` follows
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn build_trait_dispatch_emission_plan(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> CodegenTraitDispatchEmissionPlan {
    CodegenTraitDispatchEmissionPlan {
        vtable_specs: build_vtable_global_specs(trait_resolver, interner),
        dynptr_specs: build_dynptr_global_specs(trait_resolver, interner),
        summary: build_trait_dispatch_emission_summary(trait_resolver, interner),
    }
}

// ============================================================================
// Stage 5.54: Codegen trait-dispatch emission orchestrator (plan-based)
//
// First **plan-based orchestrator** — takes a `&CodegenTraitDispatchEmissionPlan`
// (from Stage 5.53) + `&mut dyn Emitter`, emits all trait-dispatch globals
// (vtable + dynptr) by iterating the plan's specs.
//
// Stage 5.55 driver refactor will call `build_trait_dispatch_emission_plan()`
// + this orchestrator, replacing separate `emit_vtables()` +
// `emit_dyn_trait_ptrs()` calls.
//
// Per API-naming-standard §3: `emit_trait_dispatch_globals_from_plan`
// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern. The `_from_plan`
// suffix indicates the input source (plan, not resolver).
//
// Per §16: takes `&CodegenTraitDispatchEmissionPlan` + `&mut dyn Emitter`. No
// `mir::ty` / `TraitResolver` / `Rodeo` reference, no circular dependency.
// ============================================================================

/// Stage 5.54: Emit all trait-dispatch globals (vtable + dynptr) from a
/// pre-built `CodegenTraitDispatchEmissionPlan`.
///
/// This is the **first plan-based orchestrator**. Stage 5.55 driver refactor
/// will call `build_trait_dispatch_emission_plan()` (Stage 5.53) + this
/// orchestrator, replacing separate `emit_vtables()` + `emit_dyn_trait_ptrs()`
/// calls.
///
/// Behavior is **identical** to `emit_vtables_and_dynptrs_from_resolver()`
/// (Stage 5.51) when given the plan from the same resolver — verified by
/// `test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator`.
///
/// Per API-naming-standard §3: `emit_trait_dispatch_globals_from_plan`
/// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn emit_trait_dispatch_globals_from_plan(
    plan: &CodegenTraitDispatchEmissionPlan,
    emitter: &mut dyn Emitter,
) {
    // Emit vtable globals first (matching emit_vtables order).
    for spec in &plan.vtable_specs {
        emitter.emit_vtable_global(&spec.global_name, &spec.method_symbols);
    }
    // Then emit dynptr globals (matching emit_dyn_trait_ptrs order).
    for spec in &plan.dynptr_specs {
        emitter.emit_dyn_trait_const(&spec.global_name, &spec.data_symbol, &spec.vtable_symbol);
    }
}

// ============================================================================
// Stage 5.55: Codegen trait-dispatch emission text batch (plan-based)
//
// Text-based batch generation of all trait-dispatch globals (vtable + dynptr)
// WITHOUT needing an `Emitter` trait object. This is the **plan-based
// counterpart** of Stage 5.45's `emit_vtable_globals_batch()`, extended to
// both vtable + dynptr globals.
//
// Useful for:
// - Testing (assert IR text directly, no Emitter construction needed)
// - Future codegen paths that push pre-formatted text to emitter.globals
// - Diagnostics (inspect the IR lines before emission)
//
// Per API-naming-standard §3: `emit_trait_dispatch_globals_text_batch`
// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern. The
// `_text_batch` suffix indicates LLVM IR text batch (no Emitter).
//
// Per §16: takes `&CodegenTraitDispatchEmissionPlan`, returns `Vec<String>`.
// No `mir::ty` / `Emitter` / `TraitResolver` / `Rodeo` reference, no
// circular dependency.
// ============================================================================

/// Stage 5.55: Generate LLVM IR text for all trait-dispatch globals (vtable
/// and dynptr) from a pre-built `CodegenTraitDispatchEmissionPlan`, WITHOUT
/// needing an `Emitter` trait object.
///
/// This is the **plan-based counterpart** of Stage 5.45's
/// `emit_vtable_globals_batch()`, extended to both vtable + dynptr globals.
/// Each output element is one LLVM IR line (one global definition).
///
/// Returns `Vec<String>` where:
/// - First N elements are vtable global definitions (from `plan.vtable_specs`)
/// - Next M elements are dynptr global definitions (from `plan.dynptr_specs`)
/// - N == M == `plan.vtable_specs.len()` (one vtable + one dynptr per spec)
///
/// Each vtable line is identical to what `emit_vtable_global_text()` (Stage
/// 5.44) produces. Each dynptr line is identical to what
/// `emit_dynptr_global_text()` (Stage 5.48) produces.
///
/// Per API-naming-standard §3: `emit_trait_dispatch_globals_text_batch`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_trait_dispatch_globals_text_batch(
    plan: &CodegenTraitDispatchEmissionPlan,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    // Vtable global IR text (Stage 5.44)
    for spec in &plan.vtable_specs {
        lines.push(emit_vtable_global_text(
            &spec.global_name,
            &spec.method_symbols,
        ));
    }
    // Dynptr global IR text (Stage 5.48)
    for spec in &plan.dynptr_specs {
        lines.push(emit_dynptr_global_text(
            &spec.global_name,
            &spec.data_symbol,
            &spec.vtable_symbol,
        ));
    }
    lines
}

// ============================================================================
// Stage 5.56: Codegen trait-dispatch emission text batch from resolver
//
// **Convenience entry point** — one call from `(&TraitResolver, &Rodeo)`
// to `Vec<String>` (all trait-dispatch global IR text). Composes Stage 5.53's
// `build_trait_dispatch_emission_plan()` + Stage 5.55's
// `emit_trait_dispatch_globals_text_batch()`.
//
// This is the final piece before Stage 5.57 driver delegation — codegen can
// call this single function to get all trait-dispatch IR text without
// needing an Emitter or a separate plan-building step.
//
// Per API-naming-standard §3: `emit_trait_dispatch_globals_text_batch_from_resolver`
// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `Vec<String>`. No `mir::ty` / `Emitter` reference, no circular
// dependency.
// ============================================================================

/// Stage 5.56: Generate LLVM IR text for all trait-dispatch globals (vtable
/// and dynptr) directly from `(&TraitResolver, &Rodeo)` in one call —
/// **convenience entry point** combining plan-building + text-batch generation.
///
/// This is the final piece before Stage 5.57 driver delegation. Codegen can
/// call this single function to get all trait-dispatch IR text without
/// needing an Emitter or a separate plan-building step.
///
/// Internally:
/// 1. `build_trait_dispatch_emission_plan(trait_resolver, interner)` (Stage 5.53)
/// 2. `emit_trait_dispatch_globals_text_batch(&plan)` (Stage 5.55)
///
/// Behavior is **identical** to calling `emit_vtables()` +
/// `emit_dyn_trait_ptrs()` separately — verified by
/// `test_match_separate_emit_vtables_and_dyn_trait_ptrs`.
///
/// Per API-naming-standard §3: `emit_trait_dispatch_globals_text_batch_from_resolver`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn emit_trait_dispatch_globals_text_batch_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<String> {
    let plan = build_trait_dispatch_emission_plan(trait_resolver, interner);
    emit_trait_dispatch_globals_text_batch(&plan)
}
