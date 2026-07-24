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

    // Also walk AggregateKind::Adt field_tys in every Assign statement.
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            if let StatementKind::Assign(boxed) = &stmt.kind {
                let (_, rvalue) = &**boxed;
                if let Rvalue::Aggregate(AggregateKind::Adt(_, _, _, field_tys), _) = rvalue {
                    for ft in field_tys {
                        collect_adt_def_ids(ft, &mut def_ids_to_register);
                    }
                }
            }
        }
    }

    // Register each unique DefId using the Entry API.
    for def_id in def_ids_to_register {
        if let std::collections::hash_map::Entry::Vacant(e) = mir.adt_layouts.entry(def_id) {
            if let Some(layout) = build_adt_layout(def_id, hir) {
                let nested: Vec<DefId> = layout.field_def_ids();
                e.insert(layout);
                for nested_id in nested {
                    if let std::collections::hash_map::Entry::Vacant(ne) =
                        mir.adt_layouts.entry(nested_id)
                    {
                        if let Some(nested_layout) = build_adt_layout(nested_id, hir) {
                            ne.insert(nested_layout);
                        }
                    }
                }
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
