//! Vtable data structures for trait dispatch.
//!
//! Stage 5.5: VtableEntry + Vtable structs.
//! Stage 5.6: VtableEntry.fn_name carries resolved LLVM symbol name.
//!
//! Per §16: vtable data is collected by TraitResolver during collect()
//! and passed as data to codegen.

use crate::hir::DefId;
use lasso::Spur;

/// Stage 5.5: A single entry in a vtable — maps a trait method name
/// to the concrete function that implements it.
///
/// Stage 5.6: `fn_def_id` has been replaced by `fn_name: String` —
/// the resolved LLVM symbol name (e.g. `landin_S_bar`).
#[derive(Debug, Clone)]
pub struct VtableEntry {
    pub method_name: Spur,
    pub fn_name: String,
}

/// Stage 5.5: A vtable for a specific (trait, type) pair.
#[derive(Debug, Clone)]
pub struct Vtable {
    pub trait_name: Spur,
    pub self_ty_name: Spur,
    pub impl_def_id: DefId,
    pub entries: Vec<VtableEntry>,
}
