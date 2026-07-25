//! Stage 6.15 (TD-025): Typeck data tables.
//!
//! Per 03-type-system.md §4 (Type inference data structures). Extracted
//! from `checker.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns 3 data structures:
//! - `TypeckResults` (per-body type checking results: LocalId → Ty + HirId → Ty)
//! - `FieldTyTable` (pre-computed ADT field types, built by driver from HIR)
//! - `FnSigTable` (pre-computed function signatures, built by driver from HIR)

use crate::mir::place::LocalId;
use crate::mir::ty::*;

/// Per-body type checking results.
///
/// After running `TypeChecker::check_mir_body`, this struct holds:
/// - The resolved type of each local (keyed by LocalId)
/// - The resolved type of each HirId (keyed by HirId, for HIR writeback)
///
/// Stage 2.4d (P1-3): The driver collects these results so downstream
/// consumers (codegen, error display) can consult the resolved types
/// instead of re-running type inference.
#[derive(Debug, Default, Clone)]
pub struct TypeckResults {
    /// Map from LocalId → resolved Ty.
    pub local_types: std::collections::HashMap<LocalId, Ty>,
    /// Map from HirId → resolved Ty (for HIR nodes that have a type).
    /// Populated for local variable bindings; other HIR nodes are Stage 3+.
    pub hir_types: std::collections::HashMap<crate::hir::HirId, Ty>,
}

impl TypeckResults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up the resolved type of a local.
    pub fn local_type(&self, id: LocalId) -> Option<&Ty> {
        self.local_types.get(&id)
    }

    /// Look up the resolved type of a HIR node.
    pub fn hir_type(&self, id: crate::hir::HirId) -> Option<&Ty> {
        self.hir_types.get(&id)
    }
}

/// Stage 3.60: Pre-computed ADT field type table.
/// Built by the driver from HIR (data flows downstream per section 16.2.1).
/// Replaces `check_mir_body_with_hir`'s HIR reference with a pure data
/// structure — typeck no longer reads HIR directly.
#[derive(Debug, Clone, Default)]
pub struct FieldTyTable {
    /// Maps struct DefId to ordered field types (as MIR Ty).
    pub struct_fields: std::collections::HashMap<crate::hir::DefId, Vec<Ty>>,
}

impl FieldTyTable {
    /// Look up the field types for a struct by DefId.
    pub fn get_struct_fields(&self, def_id: &crate::hir::DefId) -> Option<&[Ty]> {
        self.struct_fields.get(def_id).map(|v| v.as_slice())
    }
}

/// Stage 3.60: Pre-computed function signatures.
/// Built by the driver from HIR, replacing `populate_fn_sigs(&hir)`.
#[derive(Debug, Clone, Default)]
pub struct FnSigTable {
    /// Maps fn DefId to MIR-level signature.
    pub sigs: std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
}

impl FnSigTable {
    pub fn get(&self, def_id: &crate::hir::DefId) -> Option<&crate::mir::ty::Sig> {
        self.sigs.get(def_id)
    }
}
