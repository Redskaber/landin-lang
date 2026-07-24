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
    trait_resolver: &crate::traits::TraitResolver,
    interner: &lasso::Rodeo,
) -> Vec<String> {
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(trait_resolver, interner);
    emit_dyn_trait_fat_ptrs_text_batch(&fat_ptrs)
}

// ============================================================================
// Stage 5.66: DynTraitMethodCall MIR representation
//
// Represents a `dyn Trait` method call at the MIR level: receiver fat
// pointer (trait + type) + method name + vtable slot index + param count.
// This is the last infrastructure piece before actual method call MIR
// lowering (Stage 5.67+).
//
// Per API-naming-standard §3: `DynTraitMethodCall` follows
// `<Noun><Noun><Noun>` pattern.
// ============================================================================

/// Stage 5.66: MIR-level representation of a `dyn Trait` method call.
///
/// Captures all information needed to generate LLVM IR for a dynamic
/// dispatch method call:
/// - `trait_name` / `type_name`: identify the (trait, type) pair (and
///   thus the vtable)
/// - `method_name`: the method being called (for diagnostics)
/// - `slot_index`: the vtable slot index (from `stdlib_trait_method_index`)
/// - `param_count`: number of parameters (excluding `self`)
///
/// Per API-naming-standard §3: `DynTraitMethodCall` follows
/// `<Noun><Noun><Noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynTraitMethodCall {
    /// The trait name (e.g. "Display").
    pub trait_name: String,
    /// The concrete type name (e.g. "MyType").
    pub type_name: String,
    /// The method name being called (e.g. "fmt").
    pub method_name: String,
    /// The vtable slot index (0-based, from `stdlib_trait_method_index`).
    pub slot_index: u32,
    /// Number of parameters excluding `self`.
    pub param_count: u32,
}

impl DynTraitMethodCall {
    /// Stage 5.66: Construct a `DynTraitMethodCall` from all fields.
    ///
    /// Per API-naming-standard §3: `new` is the standard constructor name.
    pub fn new(
        trait_name: &str,
        type_name: &str,
        method_name: &str,
        slot_index: u32,
        param_count: u32,
    ) -> Self {
        Self {
            trait_name: trait_name.to_string(),
            type_name: type_name.to_string(),
            method_name: method_name.to_string(),
            slot_index,
            param_count,
        }
    }

    /// Stage 5.66: Construct a `DynTraitMethodCall` from a `DynTraitFatPtr`
    /// plus method-specific info.
    ///
    /// Borrows the trait_name and type_name from the fat pointer, adding
    /// the method name, slot index, and parameter count.
    ///
    /// Per API-naming-standard §3: `from_fat_ptr` follows
    /// `<prep>_<noun>_<noun>` pattern.
    pub fn from_fat_ptr(
        fat_ptr: &DynTraitFatPtr,
        method_name: &str,
        slot_index: u32,
        param_count: u32,
    ) -> Self {
        Self {
            trait_name: fat_ptr.trait_name.clone(),
            type_name: fat_ptr.type_name.clone(),
            method_name: method_name.to_string(),
            slot_index,
            param_count,
        }
    }

    /// Stage 5.66: Get the vtable symbol for this method call's vtable.
    ///
    /// Returns `.vtable.<trait>.<type>` — the LLVM symbol for the vtable
    /// global that contains the method being called.
    pub fn vtable_symbol(&self) -> String {
        format!(".vtable.{}.{}", self.trait_name, self.type_name)
    }

    /// Stage 5.66: Get the dynptr symbol for this method call's fat pointer.
    ///
    /// Returns `.dynptr.<trait>.<type>` — the LLVM symbol for the dynptr
    /// global that contains the (data, vtable) pair.
    pub fn dynptr_symbol(&self) -> String {
        format!(".dynptr.{}.{}", self.trait_name, self.type_name)
    }
}

// ============================================================================
// Stage 5.67: emit_dyn_trait_method_call_text
//
// Converts DynTraitMethodCall (Stage 5.66 MIR representation) to LLVM IR
// text for a vtable indirect call. This is the FIRST substantive dyn Trait
// method call lowering — from data structure to actual LLVM IR instructions.
//
// Per API-naming-standard §3: `emit_dyn_trait_method_call_text` follows
// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.67: Convert a `DynTraitMethodCall` to LLVM IR text for a vtable
/// indirect call.
///
/// Produces LLVM IR that:
/// 1. Extracts the vtable pointer from the fat pointer (second field)
/// 2. Loads the method function pointer from the vtable at the slot index
/// 3. Calls the loaded function pointer with self + args
///
/// Example output for `Display::fmt` on `Vec` (slot 0, 1 param):
/// ```text
/// ; dyn Display.Vec::fmt (slot=0, params=1)
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
// Stage 5.68: build_dyn_trait_method_calls_from_fat_ptrs
//
// Bridge function connecting stdlib trait method index (Stage 5.36-5.37)
// with DynTraitMethodCall (Stage 5.66 MIR representation). For each fat
// pointer, looks up the trait's methods via stdlib_trait_methods() and
// constructs DynTraitMethodCall for each method with its slot index.
//
// Per API-naming-standard §3: `build_dyn_trait_method_calls_from_fat_ptrs`
// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` pattern.
// ============================================================================

/// Stage 5.68: Build a list of `DynTraitMethodCall` from `&[DynTraitFatPtr]`
/// using the stdlib trait method index.
///
/// For each fat pointer:
/// 1. Looks up the trait's methods via `stdlib_trait_methods()` (Stage 5.36)
/// 2. For each method, gets the slot index via `stdlib_trait_method_index()` (Stage 5.37)
/// 3. Constructs a `DynTraitMethodCall` with the fat ptr's trait/type names,
///    method name, slot index, and parameter count
///
/// Traits not in the stdlib registry are silently skipped (their methods
/// would need to be resolved from user-defined trait definitions, which is
/// a future stage).
///
/// Per API-naming-standard §3: `build_dyn_trait_method_calls_from_fat_ptrs`
/// follows `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` pattern.
pub fn build_dyn_trait_method_calls_from_fat_ptrs(
    fat_ptrs: &[DynTraitFatPtr],
) -> Vec<DynTraitMethodCall> {
    let mut calls: Vec<DynTraitMethodCall> = Vec::new();
    for fp in fat_ptrs {
        // Look up trait methods from stdlib registry (Stage 5.36)
        if let Some(methods) = crate::stdlib::stdlib_trait_methods(&fp.trait_name) {
            for method in methods {
                // Get slot index (Stage 5.37)
                if let Some(slot_index) =
                    crate::stdlib::stdlib_trait_method_index(&fp.trait_name, method.name)
                {
                    calls.push(DynTraitMethodCall::from_fat_ptr(
                        fp,
                        method.name,
                        slot_index,
                        method.param_count,
                    ));
                }
            }
        }
        // Traits not in stdlib registry are silently skipped
    }
    calls
}

// ============================================================================
// Stage 5.69: emit_dyn_trait_method_calls_text_batch
//
// Batch version of Stage 5.67's emit_dyn_trait_method_call_text().
// Converts &[DynTraitMethodCall] to Vec<String> (all method call IR text).
//
// Per API-naming-standard §3: `emit_dyn_trait_method_calls_text_batch`
// follows `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` pattern.
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
