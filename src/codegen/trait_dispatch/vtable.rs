//! Vtable global emission — pure-function helpers + resolver-driven orchestrator.
//!
//! Stage 14.3 §14.4 architectural split: extracted from the historical
//! `codegen/trait_dispatch.rs` (962 LOC) along the vtable/dynptr/orchestrator
//! boundary. This module owns **vtable global emission only** — producing
//! LLVM IR lines of the form:
//!
//! ```text
//! @.vtable.<trait>.<type> = private unnamed_addr constant [N x ptr] [ptr @sym1, ...]
//! ```
//!
//! Per §16: every function below takes pre-built data
//! (`&TraitResolver` / `&StdlibVtableEmission` / raw strings) — no HIR
//! access. Data flows downstream: traits → codegen/trait_dispatch/vtable → IR.
//!
//! Per API-naming-standard §3: `emit_` prefix is consistent across the
//! codegen module (`emit_vtables`, `emit_vtable_global_text`,
//! `emit_vtables_from_resolver`).

use crate::codegen::Emitter;
use lasso::Rodeo;

// ============================================================================
// Stage 5.43: Codegen vtable emission helper (pure free function)
//
// New free function `emit_vtable_global_from_emission()` that takes a
// `&StdlibVtableEmission` and returns the LLVM IR text for one vtable global.
// This is the **pure-function counterpart** of
// `TextEmitter::emit_vtable_global()` — produces byte-for-byte identical
// IR, but doesn't require an `Emitter` trait object.
//
// Per API-naming-standard §3: `emit_vtable_global_from_emission` follows
// `<verb>_<noun>_<adj>_<prep>_<noun>` pattern. The `emit_` prefix is
// consistent with the rest of the codegen module (`emit_vtables`,
// `emit_dyn_trait_ptrs`, `emit_fat_ptr_type`).
//
// Per §16: takes `&StdlibVtableEmission` (stdlib-internal type) and returns
// `String`. No `mir::ty` / `traits::TraitResolver` / `Emitter` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.43: Build the LLVM IR text for one vtable global from a
/// `StdlibVtableEmission`.
///
/// Produces a line like:
/// ```text
/// @.vtable.<trait>.<type> = private unnamed_addr constant [N x ptr] [ptr @sym1, ptr @sym2, ...]
/// ```
///
/// Edge cases:
/// - `method_symbols.is_empty()` (marker trait) → `... constant zeroinitializer`
/// - `method_symbols = ["null", ...]` → `ptr null` literal in the initializer
///
/// The output is **byte-for-byte identical** to what
/// `TextEmitter::emit_vtable_global()` produces — verified by
/// `test_emit_vtable_global_from_emission_match_text_emitter` in
/// `tests/v0/stage5/plan/codegen_vtable_emission_helper_tests.rs`.
///
/// Per API-naming-standard §3: `emit_vtable_global_from_emission` follows
/// `<verb>_<noun>_<adj>_<prep>_<noun>` pattern.
pub fn emit_vtable_global_from_emission(emission: &crate::stdlib::StdlibVtableEmission) -> String {
    // Build the LLVM initializer expression — mirrors
    // TextEmitter::emit_vtable_global (text_emitter.rs:538-546).
    //
    // Stage 5.43: the `method_symbols` entries may be either:
    //   - a real symbol name like "landin_S_clone" → emit as `ptr @landin_S_clone`
    //   - the literal string "null" (from `stdlib_vtable_method_symbols` when
    //     a slot is not provided) → emit as `ptr null` (no `@` prefix)
    let init = if emission.method_symbols.is_empty() {
        // Stage 18.326 B3 (P1 soundness fix): typed initializer.
        "[0 x ptr] zeroinitializer".to_string()
    } else {
        let entries: Vec<String> = emission
            .method_symbols
            .iter()
            .map(|sym| {
                if sym == "null" {
                    "ptr null".to_string()
                } else {
                    format!("ptr @{}", sym)
                }
            })
            .collect();
        format!(
            "[{} x ptr] [{}]",
            emission.method_symbols.len(),
            entries.join(", ")
        )
    };

    format!(
        "@{} = internal unnamed_addr constant {}",
        emission.global_name, init
    )
}

/// Stage 5.44: Build the LLVM IR text for one vtable global from raw
/// `(global_name, method_symbols)` parameters.
///
/// This is the **bridge function** between Stage 5.43's
/// `emit_vtable_global_from_emission()` (high-level, takes
/// `StdlibVtableEmission`) and the future Stage 5.45 refactor where
/// `TextEmitter::emit_vtable_global()` will delegate here.
///
/// Parameter signature matches `TextEmitter::emit_vtable_global()` exactly
/// — `(global_name: &str, method_symbols: &[String])` — so the Stage 5.45
/// delegation is a trivial body change.
///
/// Produces a line like:
/// ```text
/// @<global_name> = private unnamed_addr constant [N x ptr] [ptr @sym1, ptr @sym2, ...]
/// ```
///
/// Edge cases:
/// - `method_symbols.is_empty()` → `... constant zeroinitializer`
/// - `method_symbols = ["null", ...]` → `ptr null` literal (no `@` prefix)
///
/// Per API-naming-standard §3: `emit_vtable_global_text` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern. The `_text` suffix indicates the
/// function returns LLVM IR text (String), distinguishing it from the
/// trait method's side-effect version.
///
/// Per §16: pure function, input `(&str, &[String])`, output `String`. No
/// `mir::ty` / `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`
/// reference, no circular dependency.
pub fn emit_vtable_global_text(global_name: &str, method_symbols: &[String]) -> String {
    // Build the LLVM initializer expression.
    //
    // Stage 5.44: handles `"null"` strings in `method_symbols` → `ptr null`
    // literal (no `@` prefix). This matches the behavior of
    // `emit_vtable_global_from_emission()` (Stage 5.43) and prepares for
    // Stage 5.45 where `TextEmitter::emit_vtable_global()` will delegate
    // here.
    let init = if method_symbols.is_empty() {
        // Stage 18.326 B3 (P1 soundness fix): empty vtable needs typed
        // initializer. `zeroinitializer` alone is invalid LLVM IR — must
        // be `[0 x ptr] zeroinitializer`. Per §2.2 + §12: root-cause fix.
        // Per Rust design: rustc always emits typed initializers.
        "[0 x ptr] zeroinitializer".to_string()
    } else {
        let entries: Vec<String> = method_symbols
            .iter()
            .map(|sym| {
                if sym == "null" {
                    "ptr null".to_string()
                } else {
                    format!("ptr @{}", sym)
                }
            })
            .collect();
        format!("[{} x ptr] [{}]", method_symbols.len(), entries.join(", "))
    };

    format!("@{} = internal unnamed_addr constant {}", global_name, init)
}

// ============================================================================
// Stage 5.45: Codegen vtable emission batch helper
//
// Batch version of `emit_vtable_global_text()` (Stage 5.44). Takes a slice
// of `StdlibVtableGlobalSpec` and returns `Vec<String>` — one LLVM IR line
// per spec. Prepares for Stage 5.46 refactor where `emit_vtables()` will
// construct the spec list once, call this batch helper, and push all IR
// lines to the emitter in one pass.
//
// Per API-naming-standard §3:
//   - `StdlibVtableGlobalSpec` follows `<Noun><Noun><Noun><Noun>` pattern.
//   - `emit_vtable_globals_batch` follows `<verb>_<noun>_<adj>_<noun>`
//     pattern. `_batch` suffix indicates batch version; `_globals` (plural)
//     distinguishes from Stage 5.44's `emit_vtable_global_text` (singular).
//
// Per §16: uses only String + Vec<String> — no `mir::ty` /
// `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.45: Specification for one vtable global — the inputs needed by
/// `emit_vtable_global_text()` packaged as a struct for batch processing.
///
/// Codegen constructs a `Vec<StdlibVtableGlobalSpec>` (one per (trait, type)
/// pair in `TraitResolver.vtables`), then calls
/// `emit_vtable_globals_batch()` to generate all IR lines in one pass.
///
/// Per API-naming-standard §3: `StdlibVtableGlobalSpec` follows
/// `<Noun><Noun><Noun><Noun>` pattern. Field names follow `<noun>_<noun>`
/// pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibVtableGlobalSpec {
    /// LLVM global name (e.g. `.vtable.Clone.S` — without leading `@`).
    pub global_name: String,
    /// Ordered method symbol list — each entry is either a real symbol
    /// (e.g. `landin_S_clone`) or the literal `"null"` for missing slots.
    pub method_symbols: Vec<String>,
}

/// Stage 5.45: Build LLVM IR text for multiple vtable globals in one call.
///
/// Given a slice of `StdlibVtableGlobalSpec`, returns `Vec<String>` where
/// each element is one vtable global definition (one LLVM IR line). The
/// output order matches the input order — no sorting or deduplication.
///
/// Each output line is identical to what `emit_vtable_global_text()` (Stage
/// 5.44) produces for the corresponding spec — verified by
/// `test_emit_vtable_globals_batch_matches_individual`.
///
/// Empty input returns an empty Vec.
///
/// Per API-naming-standard §3: `emit_vtable_globals_batch` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern.
pub fn emit_vtable_globals_batch(specs: &[StdlibVtableGlobalSpec]) -> Vec<String> {
    specs
        .iter()
        .map(|spec| emit_vtable_global_text(&spec.global_name, &spec.method_symbols))
        .collect()
}

// ============================================================================
// Stage 5.46: Codegen vtable spec builder
//
// Pure free function that extracts the "construct spec list" logic from
// `emit_vtables()` into a standalone function. Takes `&TraitResolver` +
// `&Rodeo` (same inputs as `emit_vtables()`), returns
// `Vec<StdlibVtableGlobalSpec>`.
//
// Stage 5.47 will refactor `emit_vtables()` to call this builder +
// `emit_vtable_globals_batch()` + push all IR lines to emitter in one pass.
//
// Per API-naming-standard §3: `build_vtable_global_specs` follows
// `<verb>_<noun>_<adj>_<noun>` pattern. The `build_` prefix indicates a
// constructor function (input data → output data, no side effects).
//
// Per §16: takes `&TraitResolver` + `&Rodeo` (same as `emit_vtables()`),
// returns `Vec<StdlibVtableGlobalSpec>`. No `mir::ty` / `Emitter` reference,
// no circular dependency.
// ============================================================================

/// Stage 5.46: Build the list of `StdlibVtableGlobalSpec` from
/// `TraitResolver.vtables`.
///
/// For each `((trait_name, self_ty_name), vtable)` entry in
/// `trait_resolver.vtables`, constructs a `StdlibVtableGlobalSpec` with:
/// - `global_name = format!(".vtable.{trait_str}.{type_str}")`
///   where `trait_str = interner.try_resolve(trait_name).unwrap_or("Trait")`
///   and `type_str = interner.try_resolve(self_ty_name).unwrap_or("Type")`
/// - `method_symbols = vtable.entries.iter().map(|e| interner.try_resolve(&e.fn_name).unwrap_or("fn")).map(String::from).collect()`
///
/// Stage 15.9: `VtableEntry.fn_name` is now an interned `Spur` (was `String`),
/// so we resolve it via the interner at the point of consumption.
///
/// This is the **pure-function extraction** of the spec-construction logic
/// currently inlined in `emit_vtables()` (Stage 5.6). Stage 5.47 will
/// refactor `emit_vtables()` to call this builder + `emit_vtable_globals_batch()`
/// + push all IR lines to emitter in one pass.
///
/// Per API-naming-standard §3: `build_vtable_global_specs` follows
/// `<verb>_<noun>_<adj>_<noun>` pattern.
pub fn build_vtable_global_specs(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
) -> Vec<StdlibVtableGlobalSpec> {
    let mut specs: Vec<StdlibVtableGlobalSpec> = Vec::new();
    for ((trait_name, self_ty_name), vtable) in &trait_resolver.vtables {
        // Build the global name: `.vtable.<trait>.<type>`.
        // LLVM global names use `.` as a private-name separator.
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");
        let global_name = format!(".vtable.{trait_str}.{type_str}");

        // Collect the resolved method symbol names from VtableEntry.
        // Stage 15.9: VtableEntry.fn_name is now Spur, resolve via interner.
        let method_symbols: Vec<String> = vtable
            .entries
            .iter()
            .map(|e| {
                interner
                    .try_resolve(&e.fn_name)
                    .map(String::from)
                    .unwrap_or_else(|| "fn".to_string())
            })
            .collect();

        specs.push(StdlibVtableGlobalSpec {
            global_name,
            method_symbols,
        });
    }
    specs
}

// ============================================================================
// Stage 5.47: Codegen vtable emission orchestrator
//
// Composes Stage 5.46's `build_vtable_global_specs()` + per-spec
// `Emitter::emit_vtable_global()` calls. This is the "pure-function +
// side-effect" combination version of `emit_vtables()` current inline loop.
//
// Stage 5.48 will refactor `emit_vtables()` to delegate to this orchestrator
// — its body becomes a one-liner.
//
// Per API-naming-standard §3: `emit_vtables_from_resolver` follows
// `<verb>_<noun>_<prep>_<noun>` pattern. The `emit_` prefix indicates
// side-effect (push to emitter). `_from_resolver` indicates the input source.
//
// Per §16: takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (same as
// `emit_vtables()`). No `mir::ty` reference, no circular dependency.
// ============================================================================

/// Stage 5.47: Emit vtable globals by composing `build_vtable_global_specs()`
/// + per-spec `Emitter::emit_vtable_global()` calls.
///
/// This is the **orchestrator** that combines:
/// 1. Stage 5.46's `build_vtable_global_specs()` — construct spec list
/// 2. Per-spec `Emitter::emit_vtable_global()` — push IR to emitter
///
/// Behavior is **identical** to `emit_vtables()` (Stage 5.6) current inline
/// loop — verified by `test_emit_vtables_from_resolver_match_emit_vtables`.
///
/// Stage 5.48 will refactor `emit_vtables()` to delegate to this orchestrator:
/// ```text
/// pub fn emit_vtables(resolver, interner, emitter) {
///     emit_vtables_from_resolver(resolver, interner, emitter)
/// }
/// ```
///
/// Per API-naming-standard §3: `emit_vtables_from_resolver` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern.
pub fn emit_vtables_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    let specs = build_vtable_global_specs(trait_resolver, interner);
    for spec in &specs {
        emitter.emit_vtable_global(&spec.global_name, &spec.method_symbols);
    }
}

/// Stage 5.6 / 5.48: Emit all vtable globals for every `(trait, type)` pair
/// in `TraitResolver.vtables`.
///
/// Historical entry point (Stage 5.6) — now delegates to
/// `emit_vtables_from_resolver()` (Stage 5.47). Behavior is identical; the
/// inline loop was replaced with a one-liner delegation, verified by
/// `test_emit_vtables_from_resolver_match_emit_vtables`.
///
/// Per §16: takes `&TraitResolver` (pre-built data) + `&Rodeo` (interner)
/// — no HIR access.
pub fn emit_vtables(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    emit_vtables_from_resolver(trait_resolver, interner, emitter)
}
