//! Stage 6.7: Trait dispatch emission — vtable/dynptr global generation.
//!
//! Architectural extraction from `codegen/mod.rs` (TD-017 step 1).
//! Contains all functions for generating vtable and dynptr global variables
//! from TraitResolver data, plus high-level orchestration APIs.
//!
//! Per §16: all functions take pre-built data (TraitResolver / StdlibVtableEmission)
//! — no HIR access. Data flows downstream: traits → codegen/trait_dispatch → LLVM IR.

use crate::codegen::Emitter;
use lasso::Rodeo;

pub fn emit_vtables(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    emitter: &mut dyn Emitter,
) {
    // Stage 5.59: delegate to emit_vtables_from_resolver() (Stage 5.47).
    // The old inline loop (Stage 5.6) has been replaced with a one-liner
    // delegation. Behavior is identical — verified by
    // test_emit_vtables_from_resolver_match_emit_vtables (Stage 5.47).
    emit_vtables_from_resolver(trait_resolver, interner, emitter)
}

/// Stage 5.7: Emit `dyn Trait` fat-pointer constant globals for every
/// (trait, type) pair in `TraitResolver.vtables`.
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
    // Stage 5.60: delegate to emit_dynptrs_from_resolver() (Stage 5.50).
    // The old inline loop (Stage 5.7) has been replaced with a one-liner
    // delegation. Behavior is identical — verified by
    // test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs (Stage 5.50).
    emit_dynptrs_from_resolver(trait_resolver, interner, emitter)
}

// Stage 3.56: Generate LLVM IR from pre-built MIR + metadata.
// This is the §16-compliant codegen entry point: takes only MIR data
// (no HIR, no re-lowering, no re-typeck).

// ============================================================================
// Stage 5.43: Codegen vtable emission helper (pure free function)
//
// New free function `emit_vtable_global_from_emission()` that takes a
// `&StdlibVtableEmission` and returns the LLVM IR text for one vtable global.
// This is the **pure-function counterpart** of
// `TextEmitter::emit_vtable_global()` — produces byte-for-byte identical
// IR, but doesn't require an `Emitter` trait object.
//
// This is the first Stage 5 sub-stage that modifies `src/codegen/`, but
// it does NOT modify the existing emission path:
//   - `emit_vtables()` (Stage 5.6) continues to iterate TraitResolver.vtables
//   - `TextEmitter::emit_vtable_global()` (Stage 5.6) continues to push to
//     `self.globals`
//
// The new function is parallel — Stage 5.44+ will refactor
// `TextEmitter::emit_vtable_global()` to delegate here, eliminating the
// duplicated LLVM IR formatting logic.
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
        "zeroinitializer".to_string()
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
        "@{} = private unnamed_addr constant {}",
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
        "zeroinitializer".to_string()
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

    format!("@{} = private unnamed_addr constant {}", global_name, init)
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
/// - `method_symbols = vtable.entries.iter().map(|e| e.fn_name.clone()).collect()`
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
        let method_symbols: Vec<String> =
            vtable.entries.iter().map(|e| e.fn_name.clone()).collect();

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
    format!("@{} = private unnamed_addr constant {}", global_name, init)
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
