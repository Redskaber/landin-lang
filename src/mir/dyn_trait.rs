//! MIR-level representation of `dyn Trait` fat pointer values.
//!
//! Stage 5.61: Foundation for dyn Trait MIR lowering. A `dyn Trait` value
//! is a fat pointer: (data_ptr, vtable_ptr). This module defines the
//! MIR-level struct that captures both components, before they are lowered
//! to LLVM IR by the codegen module.
//!
//! Per §16: uses only `String` — no `mir::ty` / `codegen::EmitType` /
//! `traits::TraitResolver` reference, no circular dependency.
//!
//! Per API-naming-standard §3: `DynTraitFatPtr` follows `<Noun><Noun><Noun>`
//! pattern. Field names follow `<noun>_<noun>` pattern.

/// Stage 5.61: MIR-level representation of a `dyn Trait` fat pointer value.
///
/// A `dyn Trait` value is a fat pointer consisting of two components:
/// - **data pointer**: points to the concrete value (erased type)
/// - **vtable pointer**: points to the vtable for the trait implementation
///
/// This struct captures both components at the MIR level, along with the
/// trait name and type name that identify which (trait, type) pair this
/// fat pointer belongs to. The actual LLVM IR emission is handled by the
/// codegen module (Stage 5.43-5.60).
///
/// # Example
///
/// For `let x: dyn Display = MyType;`, the fat pointer would be:
/// ```text
/// DynTraitFatPtr {
///     trait_name: "Display",
///     type_name: "MyType",
///     data_symbol: ".data.MyType",
///     vtable_symbol: ".vtable.Display.MyType",
///     dynptr_symbol: ".dynptr.Display.MyType",
/// }
/// ```
///
/// Per API-naming-standard §3: `DynTraitFatPtr` follows `<Noun><Noun><Noun>`
/// pattern. Field names follow `<noun>_<noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynTraitFatPtr {
    /// The trait name (e.g. "Display", "Clone").
    pub trait_name: String,
    /// The concrete type name implementing the trait (e.g. "MyType").
    pub type_name: String,
    /// LLVM symbol for the data global (`.data.<type>`).
    pub data_symbol: String,
    /// LLVM symbol for the vtable global (`.vtable.<trait>.<type>`).
    pub vtable_symbol: String,
    /// LLVM symbol for the dynptr global (`.dynptr.<trait>.<type>`).
    pub dynptr_symbol: String,
}

impl DynTraitFatPtr {
    /// Stage 5.61: Construct a `DynTraitFatPtr` from trait name + type name.
    ///
    /// Automatically computes the three LLVM symbols using the same naming
    /// convention as the codegen module:
    /// - `data_symbol = ".data.{type_name}"`
    /// - `vtable_symbol = ".vtable.{trait_name}.{type_name}"`
    /// - `dynptr_symbol = ".dynptr.{trait_name}.{type_name}"`
    ///
    /// Per API-naming-standard §3: `new` is the standard constructor name.
    pub fn new(trait_name: &str, type_name: &str) -> Self {
        Self {
            trait_name: trait_name.to_string(),
            type_name: type_name.to_string(),
            data_symbol: format!(".data.{type_name}"),
            vtable_symbol: format!(".vtable.{trait_name}.{type_name}"),
            dynptr_symbol: format!(".dynptr.{trait_name}.{type_name}"),
        }
    }

    /// Stage 5.61: Check if this fat pointer is for a marker trait.
    ///
    /// Marker traits (Copy, Send, Sync, Sized, Unpin, Eq) have no methods,
    /// so their vtables are empty. This check uses the trait name — if it
    /// matches one of the known marker trait names, returns `true`.
    ///
    /// Per API-naming-standard §3: `is_marker` follows `is_<adj>` pattern.
    pub fn is_marker(&self) -> bool {
        matches!(
            self.trait_name.as_str(),
            "Copy" | "Send" | "Sync" | "Sized" | "Unpin" | "Eq"
        )
    }
}

// ============================================================================
// Stage 5.62: Bridge function — build DynTraitFatPtr list from TraitResolver
//
// Bridges Stage 5.61's DynTraitFatPtr (MIR representation) with
// TraitResolver (trait implementation data source). For each (trait, type)
// pair in TraitResolver.vtables, constructs a DynTraitFatPtr with the
// resolved trait/type names and auto-computed LLVM symbols.
//
// Per API-naming-standard §3: `build_dyn_trait_fat_ptrs_from_resolver`
// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
//
// Per §16: takes `&TraitResolver` + `&Rodeo`, returns `Vec<DynTraitFatPtr>`.
// No `mir::ty` / `codegen` reference, no circular dependency.
// ============================================================================

/// Stage 5.62: Build a list of `DynTraitFatPtr` from `TraitResolver.vtables`.
///
/// For each `(trait_name, self_ty_name)` key in `trait_resolver.vtables`,
/// constructs a `DynTraitFatPtr` with:
/// - `trait_name` resolved via interner (or "Trait" default)
/// - `type_name` resolved via interner (or "Type" default)
/// - LLVM symbols auto-computed by `DynTraitFatPtr::new()`
///
/// This is the **bridge function** between the MIR-level `DynTraitFatPtr`
/// (Stage 5.61) and `TraitResolver` (the source of truth for trait impls).
/// Stage 5.63+ MIR lowering will call this to get the fat pointer list.
///
/// Per API-naming-standard §3: `build_dyn_trait_fat_ptrs_from_resolver`
/// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn build_dyn_trait_fat_ptrs_from_resolver(
    trait_resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> Vec<DynTraitFatPtr> {
    let mut fat_ptrs: Vec<DynTraitFatPtr> = Vec::new();
    for (trait_name, self_ty_name) in trait_resolver.vtables.keys() {
        let trait_str = interner.try_resolve(trait_name).unwrap_or("Trait");
        let type_str = interner.try_resolve(self_ty_name).unwrap_or("Type");
        fat_ptrs.push(DynTraitFatPtr::new(trait_str, type_str));
    }
    fat_ptrs
}

// ============================================================================
// Stage 5.63: Convert DynTraitFatPtr to LLVM IR text
//
// Bridges Stage 5.61's DynTraitFatPtr (MIR representation) with Stage 5.48's
// emit_dynptr_global_text() (codegen text output). This is the conversion
// function that Stage 5.64+ MIR lowering will call to generate LLVM IR text
// from a DynTraitFatPtr.
//
// Per API-naming-standard §3: `emit_dyn_trait_fat_ptr_text` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
//
// Per §16: takes `&DynTraitFatPtr`, returns `String`. Calls
// `crate::codegen::emit_dynptr_global_text()` (one-way: mir → codegen,
// no circular dependency).
// ============================================================================

/// Stage 5.63: Convert a `DynTraitFatPtr` to LLVM IR text.
///
/// Produces a line like:
/// ```text
/// @.dynptr.<trait>.<type> = private unnamed_addr constant
///     { ptr, ptr } { ptr @.data.<type>, ptr @.vtable.<trait>.<type> }
/// ```
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
