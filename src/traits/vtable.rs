//! Vtable data structures for trait dispatch.
//!
//! Stage 5.5: VtableEntry + Vtable structs.
//! Stage 5.6: VtableEntry.fn_name carries resolved LLVM symbol name.
//! Stage 15.9 (v0.2): VtableEntry.fn_name changed from `String` to `Spur`
//!   — interns the resolved symbol name to avoid per-entry heap allocation.
//!   Closes Phase 2 audit HP-B16 ("Intern VtableEntry.fn_name to Spur").
//!
//! Per §16: vtable data is collected by TraitResolver during collect()
//! and passed as data to codegen.
//! Per §1.0 原则 6 "通用 > 特例": one interned symbol per unique name.
//! Per §23 (API Naming): no API change to existing call sites — `fn_name`
//!   becomes `Spur`, callers use `interner.try_resolve(&e.fn_name)`.

use crate::hir::DefId;
use lasso::Spur;

/// Stage 5.5: A single entry in a vtable — maps a trait method name
/// to the concrete function that implements it.
///
/// Stage 5.6: `fn_def_id` has been replaced by `fn_name: String` —
/// the resolved LLVM symbol name (e.g. `landin_S_bar`).
///
/// Stage 15.9 (v0.2): `fn_name` changed from `String` to `Spur` —
/// the resolved LLVM symbol name is now interned in the `Rodeo` interner.
/// This eliminates per-entry `String` heap allocation. For a crate with
/// 50 trait methods, that's 50 fewer heap allocations per compilation.
///
/// Callers that need the `&str` should use `interner.try_resolve(&e.fn_name)`.
/// The interner is always available wherever VtableEntry is consumed
/// (TraitResolver, codegen, driver — all hold `&Rodeo`).
#[derive(Debug, Clone)]
pub struct VtableEntry {
    pub method_name: Spur,
    /// Stage 15.9: Interned symbol name. Use `interner.try_resolve(&fn_name)`
    /// to recover the `&str` (e.g. "landin_S_bar").
    pub fn_name: Spur,
}

/// Stage 5.5: A vtable for a specific (trait, type) pair.
#[derive(Debug, Clone)]
pub struct Vtable {
    pub trait_name: Spur,
    pub self_ty_name: Spur,
    pub impl_def_id: DefId,
    pub entries: Vec<VtableEntry>,
}
