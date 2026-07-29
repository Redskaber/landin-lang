//! Stage 6.1: ADT layout extraction from mir/lower/mod.rs (TD-011 split).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (3346 → ~3200).
//! Contains functions for sinking ADT storage layouts from HIR into MIR's
//! `adt_layouts` side-table, so codegen can resolve `TyKind::Adt(def_id, _)`
//! without reading HIR (per §16 — L-PIPE-1 closure from Stage 3.47).

use crate::hir::{DefId, HirCrate, HirItem, OwnerNode};
use crate::mir::body::{AdtLayout, MirBody, StatementKind};
use crate::mir::place::{AggregateKind, Rvalue};
use crate::mir::ty::{Ty, TyKind};
use crate::session::Span;

// Re-export lower_hir_ty_to_mir_ty from the parent module.
use super::lower_hir_ty_to_mir_ty;

/// Stage 3.47 (L-PIPE-1 closure): sink ADT layouts from HIR into MIR's
/// `adt_layouts` side-table.
///
/// Walks every local's type and every `AggregateKind::Adt` field type,
/// collecting all `TyKind::Adt(def_id, _)` DefIds. For each unique DefId,
/// builds an `AdtLayout` from HIR and inserts it into `mir.adt_layouts`.
/// Also registers one level of nested Adts.
pub(crate) fn populate_adt_layouts(mir: &mut MirBody, hir: &HirCrate) {
    // Collect all DefIds referenced by any local's type (top-level scan).
    let mut def_ids_to_register: Vec<DefId> = Vec::new();
    for ld in &mir.local_decls {
        collect_adt_def_ids(&ld.ty, &mut def_ids_to_register);
    }

    // Also walk AggregateKind::Adt field_tys AND AggregateKind::Closure
    // substs in every Assign statement.
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            if let StatementKind::Assign(boxed) = &stmt.kind {
                let (_, rvalue) = &**boxed;
                match rvalue {
                    Rvalue::Aggregate(AggregateKind::Adt(_, _, _, field_tys), _) => {
                        for ft in field_tys {
                            collect_adt_def_ids(ft, &mut def_ids_to_register);
                        }
                    }
                    // Stage 14.82 (GAP-7 partial fix): walk closure capture
                    // substs so captured Adts get their layouts registered.
                    Rvalue::Aggregate(AggregateKind::Closure(_, substs), _) => {
                        for st in substs {
                            collect_adt_def_ids(st, &mut def_ids_to_register);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Register each unique DefId using the Entry API.
    for def_id in def_ids_to_register {
        register_adt_layout_recursive(&mut mir.adt_layouts, def_id, hir);
    }
}

/// Stage 14.43: Recursively register an ADT layout and all of its nested ADTs.
///
/// Previously, `populate_adt_layouts` only registered one level of nesting
/// (e.g., for L1→L2→L3, it registered L1 and L2 but not L3). This caused
/// `mir_type_to_emit_type_with_layouts` to return wrong types for deeply
/// nested structs — L1 would render as `{{i32}}` (2 levels) instead of
/// `{{{i32}}}` (3 levels), causing LLVM type mismatches.
///
/// Per §13.4 (design alignment): the layout registry should be complete —
/// all reachable ADTs should have their layouts registered. This function
/// walks the nesting chain recursively until no new ADTs are found.
fn register_adt_layout_recursive(
    layouts: &mut std::collections::HashMap<DefId, AdtLayout>,
    def_id: DefId,
    hir: &HirCrate,
) {
    use std::collections::hash_map::Entry;
    if let Entry::Vacant(e) = layouts.entry(def_id) {
        if let Some(layout) = build_adt_layout(def_id, hir) {
            let nested: Vec<DefId> = layout.field_def_ids();
            e.insert(layout);
            // Recursively register all nested ADTs (any depth).
            for nested_id in nested {
                register_adt_layout_recursive(layouts, nested_id, hir);
            }
        }
    }
}

/// Walk a `Ty` and collect every `TyKind::Adt(def_id, _)` DefId into `out`.
/// Recurses into Tuple, Array, Ref, RawPtr, Slice.
fn collect_adt_def_ids(ty: &Ty, out: &mut Vec<DefId>) {
    match &ty.kind {
        TyKind::Adt(def_id, _) => out.push(*def_id),
        TyKind::Tuple(tys) => {
            for t in tys {
                collect_adt_def_ids(t, out);
            }
        }
        TyKind::Array(elem, _) => collect_adt_def_ids(elem, out),
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => collect_adt_def_ids(inner, out),
        TyKind::Slice(elem) => collect_adt_def_ids(elem, out),
        // Stage 14.82 (GAP-7 partial fix): recurse into Closure substs so
        // captured Adts get their layouts registered. Without this, a
        // closure capturing a struct would have the struct's layout missing
        // from `mir.adt_layouts`, causing `mir_type_to_emit_type_with_layouts`
        // to fall back to `EmitType::I32` for the captured struct type —
        // producing wrong LLVM types and "Invalid InsertValueInst operands!"
        // errors.
        TyKind::Closure(_, substs) => {
            for t in substs {
                collect_adt_def_ids(t, out);
            }
        }
        _ => {}
    }
}

/// Build an `AdtLayout` for the given DefId by reading HIR.
/// Returns `None` if the DefId doesn't resolve to a struct or enum.
fn build_adt_layout(def_id: DefId, hir: &HirCrate) -> Option<AdtLayout> {
    let owner = hir.owner(def_id)?;
    match owner {
        OwnerNode::Item(HirItem::Struct(s)) => {
            let field_tys = s
                .fields
                .iter()
                .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
                .collect();
            Some(AdtLayout::Struct { field_tys })
        }
        OwnerNode::Item(HirItem::Enum(e)) => {
            let discriminant_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
            let variant_payloads: Vec<Vec<Ty>> = e
                .variants
                .iter()
                .map(|variant| match &variant.data {
                    crate::hir::HirVariantData::Unit(_) => Vec::new(),
                    crate::hir::HirVariantData::Tuple(fields, _) => fields
                        .iter()
                        .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
                        .collect(),
                    crate::hir::HirVariantData::Struct(fields, _) => fields
                        .iter()
                        .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
                        .collect(),
                })
                .collect();
            Some(AdtLayout::Enum {
                discriminant_ty,
                variant_payloads,
            })
        }
        _ => None,
    }
}

/// Extension method on AdtLayout to extract nested Adt DefIds (for recursion).
trait AdtLayoutExt {
    fn field_def_ids(&self) -> Vec<DefId>;
}

impl AdtLayoutExt for AdtLayout {
    fn field_def_ids(&self) -> Vec<DefId> {
        let mut out = Vec::new();
        match self {
            AdtLayout::Struct { field_tys } => {
                for t in field_tys {
                    collect_adt_def_ids(t, &mut out);
                }
            }
            AdtLayout::Enum {
                variant_payloads, ..
            } => {
                for payload in variant_payloads {
                    for t in payload {
                        collect_adt_def_ids(t, &mut out);
                    }
                }
            }
        }
        out
    }
}
