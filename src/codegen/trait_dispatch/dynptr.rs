//! `dyn Trait` fat-pointer global emission — pure-function helpers +
//! resolver-driven orchestrator.
//!
//! Stage 14.3 §14.4 architectural split: extracted from the historical
//! `codegen/trait_dispatch.rs` (962 LOC) along the vtable/dynptr/orchestrator
//! boundary. This module owns **dynptr global emission only** — producing
//! LLVM IR lines of the form:
//!
//! ```text
//! @.dynptr.<trait>.<type> = private unnamed_addr constant
//!     { ptr, ptr } { ptr @.data.<type>, ptr @.vtable.<trait>.<type> }
//! ```
//!
//! Per §16: every function below takes pre-built data
//! (`&TraitResolver` / raw strings) — no HIR access. Data flows downstream:
//! traits → codegen/trait_dispatch/dynptr → IR.
//!
//! Per API-naming-standard §3: `emit_` prefix is consistent across the
//! codegen module (`emit_dyn_trait_ptrs`, `emit_dynptr_global_text`,
//! `emit_dynptrs_from_resolver`). Naming is symmetric with the vtable
//! counterpart (`emit_vtables` / `emit_vtable_global_text` /
//! `emit_vtables_from_resolver`).

use crate::codegen::Emitter;
use lasso::Rodeo;

// ============================================================================
// Stage 5.48: Codegen dynptr global text helper
//
// Pure free function `emit_dynptr_global_text()` that takes
// `(global_name, data_symbol, vtable_symbol)` — the **exact same parameter
// signature** as `TextEmitter::emit_dyn_trait_const()` — and returns the
// LLVM IR text for one dyn Trait fat-pointer global.
//
// This is the **dynptr counterpart** of Stage 5.44's
// `emit_vtable_global_text()`. Stage 5.49 will refactor
// `TextEmitter::emit_dyn_trait_const()` to delegate here (trivial body
// change, same signature).
//
// Per API-naming-standard §3: `emit_dynptr_global_text` follows
// `<verb>_<noun>_<adj>_<noun>` pattern. The `_text` suffix indicates the
// function returns LLVM IR text (String), distinguishing it from the trait
// method's side-effect version. Naming symmetric with Stage 5.44's
// `emit_vtable_global_text` (vtable → dynptr).
//
// Per §16: pure function, input `(&str, &str, &str)`, output `String`. No
// `mir::ty` / `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`
// reference, no circular dependency.
// ============================================================================

/// Stage 5.48: Build the LLVM IR text for one `dyn Trait` fat-pointer global
/// from raw `(global_name, data_symbol, vtable_symbol)` parameters.
///
/// This is the **dynptr counterpart** of Stage 5.44's
/// `emit_vtable_global_text()`. Parameter signature matches
/// `TextEmitter::emit_dyn_trait_const()` exactly — Stage 5.49 delegation
/// is a trivial body change.
///
/// Produces a line like:
/// ```text
/// @<global_name> = private unnamed_addr constant
///     { ptr, ptr } { ptr @<data_symbol>, ptr @<vtable_symbol> }
/// ```
///
/// Example:
/// ```text
/// @.dynptr.Foo.S = private unnamed_addr constant
///     { ptr, ptr } { ptr @.data.S, ptr @.vtable.Foo.S }
/// ```
///
/// The output is **byte-for-byte identical** to what
/// `TextEmitter::emit_dyn_trait_const()` produces — verified by
/// `test_emit_dynptr_global_text_match_text_emitter` in
/// `tests/v0/stage5/plan/codegen_dynptr_text_tests.rs`.
///
/// Per API-naming-standard §3: `emit_dynptr_global_text` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern.
pub fn emit_dynptr_global_text(
    global_name: &str,
    data_symbol: &str,
    vtable_symbol: &str,
) -> String {
    // Build the LLVM initializer — mirrors
    // TextEmitter::emit_dyn_trait_const (text_emitter.rs:569-572).
    let init = format!(
        "{{ ptr, ptr }} {{ ptr @{}, ptr @{} }}",
        data_symbol, vtable_symbol
    );
    format!("@{} = internal unnamed_addr constant {}", global_name, init)
}

// ============================================================================
// Stage 5.49: Codegen dynptr spec builder
//
// Pure free function that extracts the "construct dynptr spec list" logic
// from `emit_dyn_trait_ptrs()` into a standalone function. Takes
// `&TraitResolver` + `&Rodeo` (same inputs as `emit_dyn_trait_ptrs()`),
// returns `Vec<StdlibDynptrGlobalSpec>`.
//
// This is the **dynptr counterpart** of Stage 5.46's
// `build_vtable_global_specs()`. Stage 5.50 will refactor
// `emit_dyn_trait_ptrs()` to call this builder + per-spec
// `Emitter::emit_dyn_trait_const()` calls.
//
// Per API-naming-standard §3: `build_dynptr_global_specs` follows
// `<verb>_<noun>_<adj>_<noun>` pattern. The `build_` prefix indicates a
// constructor function (input data → output data, no side effects).
// Naming symmetric with Stage 5.46's `build_vtable_global_specs` (vtable → dynptr).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_dyn_trait_ptrs()`),
// returns `Vec<StdlibDynptrGlobalSpec>`. No `mir::ty` / `Emitter` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.49: Specification for one `dyn Trait` fat-pointer global — the
/// inputs needed by `emit_dynptr_global_text()` (Stage 5.48) packaged as a
/// struct for batch processing.
///
/// This is the **dynptr counterpart** of Stage 5.45's
/// `StdlibVtableGlobalSpec`. Codegen constructs a
/// `Vec<StdlibDynptrGlobalSpec>` (one per (trait, type) pair in
/// `TraitResolver.vtables`), then in Stage 5.50 will call
/// `emit_dynptr_global_text()` per spec to generate all IR lines.
///
/// Per API-naming-standard §3: `StdlibDynptrGlobalSpec` follows
/// `<Noun><Noun><Noun><Noun>` pattern. Naming symmetric with
/// `StdlibVtableGlobalSpec` (vtable → dynptr). Field names follow
/// `<noun>_<noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibDynptrGlobalSpec {
    /// LLVM global name (e.g. `.dynptr.Foo.S` — without leading `@`).
    pub global_name: String,
    /// Data symbol (e.g. `.data.S` — references the per-type data global).
    pub data_symbol: String,
    /// Vtable symbol (e.g. `.vtable.Foo.S` — references the vtable global).
    pub vtable_symbol: String,
}

/// Stage 5.49: Build the list of `StdlibDynptrGlobalSpec` from
/// `TraitResolver.vtables`.
///
/// For each `(trait_name, self_ty_name)` key in `trait_resolver.vtables`,
/// constructs a `StdlibDynptrGlobalSpec` with:
/// - `global_name = format!(".dynptr.{trait_str}.{type_str}")`
/// - `data_symbol = format!(".data.{type_str}")`
/// - `vtable_symbol = format!(".vtable.{trait_str}.{type_str}")`
///
/// where `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
/// and `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`.
///
/// This is the **pure-function extraction** of the spec-construction logic
/// currently inlined in `emit_dyn_trait_ptrs()` (Stage 5.7). Stage 5.50
/// will refactor `emit_dyn_trait_ptrs()` to call this builder + per-spec
/// `Emitter::emit_dyn_trait_const()` calls.
///
/// Per API-naming-standard §3: `build_dynptr_global_specs` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern. Naming symmetric with Stage 5.46's
/// `build_vtable_global_specs` (vtable → dynptr).
pub fn build_dynptr_global_specs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<StdlibDynptrGlobalSpec> {
    let mut specs: Vec<StdlibDynptrGlobalSpec> = Vec::new();
    for (trait_name, self_ty_name) in trait_resolver.vtables.keys() {
        // Global names: matches the naming convention from emit_vtables.
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");

        let global_name = format!(".dynptr.{trait_str}.{type_str}");
        let data_symbol = format!(".data.{type_str}");
        let vtable_symbol = format!(".vtable.{trait_str}.{type_str}");

        specs.push(StdlibDynptrGlobalSpec {
            global_name,
            data_symbol,
            vtable_symbol,
        });
    }
    specs
}

// ============================================================================
// Stage 5.50: Codegen dynptr emission orchestrator
//
// Composes Stage 5.49's `build_dynptr_global_specs()` + per-spec
// `Emitter::emit_dyn_trait_const()` calls. This is the "pure-function +
// side-effect" combination version of `emit_dyn_trait_ptrs()` current
// inline loop.
//
// This is the **dynptr counterpart** of Stage 5.47's
// `emit_vtables_from_resolver()`. Stage 5.51 will refactor
// `emit_dyn_trait_ptrs()` to delegate to this orchestrator — its body
// becomes a one-liner.
//
// Per API-naming-standard §3: `emit_dynptrs_from_resolver` follows
// `<verb>_<noun>_<prep>_<noun>` pattern. The `emit_` prefix indicates
// side-effect (push to emitter). `_from_resolver` indicates the input source.
// Naming symmetric with Stage 5.47's `emit_vtables_from_resolver`
// (vtables → dynptrs).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (same as
// `emit_dyn_trait_ptrs()`). No `mir::ty` reference, no circular dependency.
// ============================================================================

/// Stage 5.50: Emit `dyn Trait` fat-pointer globals by composing
/// `build_dynptr_global_specs()` + per-spec `Emitter::emit_dyn_trait_const()`
/// calls.
///
/// This is the **orchestrator** that combines:
/// 1. Stage 5.49's `build_dynptr_global_specs()` — construct spec list
/// 2. Per-spec `Emitter::emit_dyn_trait_const()` — push IR to emitter
///
/// Behavior is **identical** to `emit_dyn_trait_ptrs()` (Stage 5.7) current
/// inline loop — verified by `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs`.
///
/// Stage 5.51 will refactor `emit_dyn_trait_ptrs()` to delegate to this
/// orchestrator:
/// ```text
/// pub fn emit_dyn_trait_ptrs(resolver, interner, emitter) {
///     emit_dynptrs_from_resolver(resolver, interner, emitter)
/// }
/// ```
///
/// Per API-naming-standard §3: `emit_dynptrs_from_resolver` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern. Naming symmetric with Stage 5.47's
/// `emit_vtables_from_resolver` (vtables → dynptrs).
pub fn emit_dynptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    let specs = build_dynptr_global_specs(trait_resolver, interner);
    for spec in &specs {
        emitter.emit_dyn_trait_const(&spec.global_name, &spec.data_symbol, &spec.vtable_symbol);
    }
}

/// Stage 5.7 / 5.51: Emit `dyn Trait` fat-pointer constant globals for every
/// `(trait, type)` pair in `TraitResolver.vtables`.
///
/// Each `dyn Trait` fat pointer becomes a module-level global:
///
/// ```text
/// @.dynptr.<trait>.<type> = private unnamed_addr constant
///     { ptr, ptr } { ptr @.data.<type>, ptr @.vtable.<trait>.<type> }
/// ```
///
/// The fat pointer is `{ ptr (data), ptr (vtable) }` — the data pointer
/// references a per-type data global (`@.data.<type>`), and the vtable
/// pointer references the vtable global emitted by `emit_vtables` (Stage 5.6).
///
/// Historical entry point (Stage 5.7) — now delegates to
/// `emit_dynptrs_from_resolver()` (Stage 5.50). Behavior is identical; the
/// inline loop was replaced with a one-liner delegation, verified by
/// `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs`.
///
/// Per API-naming-standard §3: `emit_` prefix consistent with
/// `emit_vtables`, `emit_fat_ptr_type`, etc.
///
/// Per §16: takes `&TraitResolver` (pre-built data) + `&Rodeo` (interner)
/// — no HIR access.
pub fn emit_dyn_trait_ptrs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    emit_dynptrs_from_resolver(trait_resolver, interner, emitter)
}
