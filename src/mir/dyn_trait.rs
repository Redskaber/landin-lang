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
