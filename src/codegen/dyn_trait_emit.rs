//! dyn Trait LLVM IR text emission — relocated from `mir::dyn_trait` per Stage 13.1 TD-028.
//!
//! ## §16 Interface Isolation Compliance (Stage 13.1 TD-028 fix)
//!
//! **Before Stage 13.1**: 7 `emit_dyn_trait_*` functions lived in `src/mir/dyn_trait.rs`
//! and called `crate::codegen::emit_dynptr_global_text()`. This was a §16 violation —
//! MIR (upstream) producing codegen (downstream) output is a reverse-direction dependency.
//!
//! **After Stage 13.1**: The 7 functions are relocated here (to `codegen::dyn_trait_emit`).
//! The data flow is now strictly one-directional:
//!
//! ```text
//! MIR data structures (DynTraitFatPtr, DynTraitMethodCall, DynTraitMIRPlan)
//!     ↓ (read-only access)
//! codegen::dyn_trait_emit::emit_dyn_trait_*  →  LLVM IR text (String)
//!     ↓ (composes with)
//! codegen::trait_dispatch::emit_dynptr_global_text  →  LLVM IR text (String)
//! ```
//!
//! ## Function Inventory
//!
//! | Function | Origin | Stage | Purpose |
//! |----------|--------|-------|---------|
//! | `emit_dyn_trait_fat_ptr_text` | mir::dyn_trait | 5.63 | Single fat ptr → IR text |
//! | `emit_dyn_trait_fat_ptrs_text_batch` | mir::dyn_trait | 5.64 | Batch fat ptrs → Vec<IR text> |
//! | `emit_dyn_trait_fat_ptrs_text_batch_from_resolver` | mir::dyn_trait | 5.65 | Resolver → all fat ptr IR text |
//! | `emit_dyn_trait_method_call_text` | mir::dyn_trait | 5.67 | Single method call → IR text |
//! | `emit_dyn_trait_method_calls_text_batch` | mir::dyn_trait | 5.69 | Batch method calls → Vec<IR text> |
//! | `emit_dyn_trait_method_calls_text_batch_from_resolver` | mir::dyn_trait | 5.70 | Resolver → all method call IR text |
//! | `emit_dyn_trait_mir_plan_text` | mir::dyn_trait | 5.74 | Full MIR plan → IR text |
//!
//! ## §14.4 Refactor Governance (J1-J6)
//!
//! - **J1 Architecture alignment**: ✅ Restores §16 data flow单向 (MIR → codegen, no reverse)
//! - **J2 Single responsibility**: ✅ MIR no longer produces codegen text; codegen owns all IR emission
//! - **J3 Single direction flow**: ✅ Eliminates mir → codegen reverse dependency
//! - **J4 Compilation expression complete**: ✅ All 7 functions relocated as-is (no semantic change)
//! - **J5 Stage division clear**: ✅ ≤5 files affected (mir/dyn_trait.rs, mir/mod.rs, codegen/mod.rs, codegen/dyn_trait_emit.rs, lib.rs)
//! - **J6 Scientific granularity**: ✅ No impact on other modules; pure relocation
//!
//! ## Verification
//!
//! After relocation, the following grep must return ZERO matches (except comments):
//! ```bash
//! grep -rn "crate::codegen" src/mir/dyn_trait.rs
//! ```

use crate::mir::dyn_trait::{
    build_dyn_trait_fat_ptrs_from_resolver, build_dyn_trait_method_calls_from_fat_ptrs,
    DynTraitFatPtr, DynTraitMIRPlan, DynTraitMethodCall,
};
use crate::traits::TraitResolver;

// ============================================================================
// Stage 5.63: emit_dyn_trait_fat_ptr_text — single fat ptr → IR text
//
// Converts a DynTraitFatPtr (MIR-level dyn Trait fat pointer representation)
// to LLVM IR text defining the dynptr global.
//
// Internally delegates to Stage 5.48's emit_dynptr_global_text().
//
// Per API-naming-standard §3: `emit_dyn_trait_fat_ptr_text` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.63: Convert a `DynTraitFatPtr` to LLVM IR text.
///
/// Produces one `@.dynptr.<trait>.<type>` global definition line referencing
/// the data symbol and vtable symbol.
///
/// Internally delegates to Stage 5.48's `emit_dynptr_global_text()`.
///
/// Per API-naming-standard §3: `emit_dyn_trait_fat_ptr_text` follows
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_dyn_trait_fat_ptr_text(fat_ptr: &DynTraitFatPtr) -> String {
    crate::codegen::emit_dynptr_global_text(
        &fat_ptr.dynptr_symbol,
        &fat_ptr.data_symbol,
        &fat_ptr.vtable_symbol,
    )
}

// ============================================================================
// Stage 5.64: Batch version of emit_dyn_trait_fat_ptr_text
//
// Batch-converts a list of DynTraitFatPtr to Vec<String> (LLVM IR text).
// This is the batch counterpart of Stage 5.63's emit_dyn_trait_fat_ptr_text().
//
// Per API-naming-standard §3: `emit_dyn_trait_fat_ptrs_text_batch` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.64: Batch-convert a list of `DynTraitFatPtr` to LLVM IR text.
///
/// For each `DynTraitFatPtr` in the input slice, calls
/// `emit_dyn_trait_fat_ptr_text()` (Stage 5.63) and collects the results.
///
/// Returns `Vec<String>` where each element is one dynptr global definition.
/// Empty input returns an empty Vec.
///
/// Per API-naming-standard §3: `emit_dyn_trait_fat_ptrs_text_batch` follows
/// `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_dyn_trait_fat_ptrs_text_batch(fat_ptrs: &[DynTraitFatPtr]) -> Vec<String> {
    fat_ptrs.iter().map(emit_dyn_trait_fat_ptr_text).collect()
}

// ============================================================================
// Stage 5.65: Convenience entry point — resolver → all fat ptr IR text in one call
//
// Composes Stage 5.62's build_dyn_trait_fat_ptrs_from_resolver() + Stage 5.64's
// emit_dyn_trait_fat_ptrs_text_batch(). Single function from resolver to all
// dyn Trait fat ptr LLVM IR text.
//
// Per API-naming-standard §3: `emit_dyn_trait_fat_ptrs_text_batch_from_resolver`
// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
// ============================================================================

/// Stage 5.65: Generate LLVM IR text for all dyn Trait fat pointers directly
/// from `(&TraitResolver, &Rodeo)` in one call — **convenience entry point**.
///
/// Internally:
/// 1. `build_dyn_trait_fat_ptrs_from_resolver(trait_resolver, interner)` (Stage 5.62)
/// 2. `emit_dyn_trait_fat_ptrs_text_batch(&fat_ptrs)` (Stage 5.64)
///
/// Per API-naming-standard §3: `emit_dyn_trait_fat_ptrs_text_batch_from_resolver`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn emit_dyn_trait_fat_ptrs_text_batch_from_resolver(
    trait_resolver: &TraitResolver,
    interner: &lasso::Rodeo,
) -> Vec<String> {
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(trait_resolver, interner);
    emit_dyn_trait_fat_ptrs_text_batch(&fat_ptrs)
}

// ============================================================================
// Stage 5.67: emit_dyn_trait_method_call_text — single method call → IR text
//
// Converts a DynTraitMethodCall (MIR-level dyn Trait method call representation)
// to LLVM IR text performing the vtable indirect call.
//
// Per API-naming-standard §3: `emit_dyn_trait_method_call_text` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.67: Convert a `DynTraitMethodCall` to LLVM IR text.
///
/// Produces the IR for a dynamic dispatch method call:
/// 1. Extract vtable pointer from fat pointer (second field, index 1)
/// 2. Load method function pointer from vtable at slot index
/// 3. Call method function with self + args
///
/// Example output (for `dyn Trait::method` with 1 arg):
/// ```text
/// ; dyn Trait.Type::method (slot=0, params=2)
/// %vtable_ptr = getelementptr { ptr, ptr }, ptr %dynptr, i32 0, i32 1
/// %method_fn = load ptr, ptr %vtable_ptr, i32 0
/// %result = call ptr %method_fn(ptr %self, ptr %arg0)
/// ```
///
/// Per API-naming-standard §3: `emit_dyn_trait_method_call_text` follows
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_dyn_trait_method_call_text(call: &DynTraitMethodCall) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Comment line for diagnostics
    lines.push(format!(
        "; dyn {}.{}::{} (slot={}, params={})",
        call.trait_name, call.type_name, call.method_name, call.slot_index, call.param_count
    ));

    // Extract vtable pointer from fat pointer (second field, index 1)
    lines.push("%vtable_ptr = getelementptr { ptr, ptr }, ptr %dynptr, i32 0, i32 1".to_string());

    // Load method function pointer from vtable at slot index
    lines.push(format!(
        "%method_fn = load ptr, ptr %vtable_ptr, i32 {}",
        call.slot_index
    ));

    // Build parameter list: self + args
    let mut params: Vec<String> = vec!["ptr %self".to_string()];
    for i in 0..call.param_count {
        params.push(format!("ptr %arg{i}"));
    }
    lines.push(format!(
        "%result = call ptr %method_fn({})",
        params.join(", ")
    ));

    lines.join("\n")
}

// ============================================================================
// Stage 5.69: Batch version of emit_dyn_trait_method_call_text
//
// Batch-converts a list of DynTraitMethodCall to Vec<String> (LLVM IR text).
// This is the batch counterpart of Stage 5.67's emit_dyn_trait_method_call_text().
//
// Per API-naming-standard §3: `emit_dyn_trait_method_calls_text_batch` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.69: Batch-convert a list of `DynTraitMethodCall` to LLVM IR text.
///
/// For each `DynTraitMethodCall` in the input slice, calls
/// `emit_dyn_trait_method_call_text()` (Stage 5.67) and collects the results.
///
/// Returns `Vec<String>` where each element is the LLVM IR text for one
/// vtable indirect method call (multiple lines per call).
/// Empty input returns an empty Vec.
///
/// Per API-naming-standard §3: `emit_dyn_trait_method_calls_text_batch`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_dyn_trait_method_calls_text_batch(calls: &[DynTraitMethodCall]) -> Vec<String> {
    calls.iter().map(emit_dyn_trait_method_call_text).collect()
}

// ============================================================================
// Stage 5.70: emit_dyn_trait_method_calls_text_batch_from_resolver
//
// Convenience entry point composing Stage 5.62 + 5.68 + 5.69. One call
// from resolver to all dyn Trait method call IR text.
//
// Per API-naming-standard §3: `emit_dyn_trait_method_calls_text_batch_from_resolver`
// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
// ============================================================================

/// Stage 5.70: Generate LLVM IR text for all dyn Trait method calls directly
/// from `(&TraitResolver, &Rodeo)` in one call — **convenience entry point**.
///
/// Internally:
/// 1. `build_dyn_trait_fat_ptrs_from_resolver(trait_resolver, interner)` (Stage 5.62)
/// 2. `build_dyn_trait_method_calls_from_fat_ptrs(&fat_ptrs)` (Stage 5.68)
/// 3. `emit_dyn_trait_method_calls_text_batch(&calls)` (Stage 5.69)
///
/// Per API-naming-standard §3: `emit_dyn_trait_method_calls_text_batch_from_resolver`
/// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn emit_dyn_trait_method_calls_text_batch_from_resolver(
    trait_resolver: &TraitResolver,
    interner: &lasso::Rodeo,
) -> Vec<String> {
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(trait_resolver, interner);
    let calls = build_dyn_trait_method_calls_from_fat_ptrs(&fat_ptrs);
    emit_dyn_trait_method_calls_text_batch(&calls)
}

// ============================================================================
// Stage 5.74: emit_dyn_trait_mir_plan_text — full MIR plan → IR text
//
// Composes Stage 5.71 (DynTraitMIRSummary) + 5.63 (fat ptr text) + 5.67
// (method call text) into a single IR text output for the whole plan.
//
// Per API-naming-standard §3: `emit_dyn_trait_mir_plan_text` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.74: Convert a full `DynTraitMIRPlan` to LLVM IR text.
///
/// Produces:
/// 1. Summary comment line
/// 2. All fat ptr global definitions (from `emit_dyn_trait_fat_ptr_text`)
/// 3. All method call IR blocks (from `emit_dyn_trait_method_call_text`)
///
/// Per API-naming-standard §3: `emit_dyn_trait_mir_plan_text` follows
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
pub fn emit_dyn_trait_mir_plan_text(plan: &DynTraitMIRPlan) -> String {
    let mut sections: Vec<String> = Vec::new();

    // Summary comment
    sections.push(format!(
        "; DynTraitMIRSummary: {} fat ptrs, {} method calls, {} slots",
        plan.summary.fat_ptr_count, plan.summary.method_call_count, plan.summary.total_slots
    ));

    // Fat ptr globals
    if !plan.fat_ptrs.is_empty() {
        let mut fat_ptr_lines: Vec<String> = Vec::new();
        for fp in &plan.fat_ptrs {
            fat_ptr_lines.push(emit_dyn_trait_fat_ptr_text(fp));
        }
        sections.push(fat_ptr_lines.join("\n"));
    }

    // Method call IR
    if !plan.method_calls.is_empty() {
        let mut call_lines: Vec<String> = Vec::new();
        for call in &plan.method_calls {
            call_lines.push(emit_dyn_trait_method_call_text(call));
        }
        sections.push(call_lines.join("\n"));
    }

    sections.join("\n\n")
}
