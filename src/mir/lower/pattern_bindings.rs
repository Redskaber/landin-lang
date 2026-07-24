//! Stage 6.3: Pattern binding extraction from mir/lower/mod.rs (TD-011 split step 3).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (3035 → ~2580).
//! Contains functions for collecting pattern bindings into MIR locals,
//! lowering enum variant pattern bindings with payload extraction,
//! and pattern-related utility functions.

use crate::ast;
use crate::hir::*;
use crate::mir::place::*;
use crate::mir::ty::*;

use super::{resolve_enum_variant, MirLowerCtxt};

/// Check if a pattern binds a mutable local.
///
/// For `Ident` patterns with `mut`, returns `Mutable`. For non-Ident
/// patterns (Wild, Tuple, Struct, etc.), returns `Immutable` (the default —
/// these patterns don't directly bind a single local).
pub(crate) fn pat_mutability(pat: &HirPat) -> Mutability {
    use crate::ast::BindingMode;
    match &pat.kind {
        HirPatKind::Ident(
            BindingMode::ByValue(ast::Mutability::Mutable)
            | BindingMode::ByRef(ast::Mutability::Mutable),
            _,
            _,
        ) => Mutability::Mutable,
        _ => Mutability::Immutable,
    }
}

/// Collect pattern bindings into the MIR local map.
pub(crate) fn collect_pat_bindings_for_mir(cx: &mut MirLowerCtxt, pat: &HirPat) {
    match &pat.kind {
        HirPatKind::Ident(_mode, _ident, sub) => {
            let ty = cx.fresh_infer_ty(pat.span);
            cx.new_local(pat.hir_id, ty, None);
            if let Some(s) = sub {
                collect_pat_bindings_for_mir(cx, s);
            }
        }
        HirPatKind::TupleStruct(_, pats) => {
            for p in pats {
                collect_pat_bindings_for_mir(cx, p);
            }
        }
        HirPatKind::Tuple(pats) => {
            for p in pats {
                collect_pat_bindings_for_mir(cx, p);
            }
        }
        HirPatKind::Struct(_, fields, _) => {
            for f in fields {
                collect_pat_bindings_for_mir(cx, &f.pat);
            }
        }
        _ => {}
    }
}

/// Stage 3.48 (L-ENUM-BINDING closure): Generate payload-extraction
/// projections for enum tuple/struct variant patterns.
///
/// Before this fix, `Opt::Some(x) => x` would allocate a local for `x`
/// but never assign it — reading uninitialized memory (P0 soundness bug).
///
/// This function walks the pattern, and for each enum variant pattern
/// (`TupleStruct` or `Struct` on an enum variant), generates MIR
/// statements that extract the variant's payload fields from the
/// scrutinee and assign them to the binding locals.
///
/// Per §16: reads HIR (allowed — data flows downstream per §16.2.1) to
/// resolve variant_idx and field types. Codegen reads the MIR projection
/// — no HIR lookup at codegen time.
///
/// The flat field_idx computation (per Stage 3.48 layout):
///   `field_idx(variant_V, field_F) = 1 + sum(field_counts of variants 0..V-1) + F`
/// where the `1` accounts for the discriminant at storage.0.
pub(crate) fn lower_enum_variant_pattern_bindings(
    cx: &mut MirLowerCtxt,
    scrut_local: LocalId,
    pat: &HirPat,
) {
    let span = pat.span;
    match &pat.kind {
        HirPatKind::TupleStruct(path, sub_pats) => {
            if let Res::Def(enum_def_id, crate::resolve::DefKind::Enum) = path.res {
                if let Some((variant_idx, field_tys)) =
                    resolve_enum_variant(cx, enum_def_id, &path.segments[1].ident.name)
                {
                    let payload_tys = &field_tys[1..];
                    let starting_idx =
                        compute_enum_payload_starting_idx(cx, enum_def_id, variant_idx);
                    for (i, sub_pat) in sub_pats.iter().enumerate() {
                        if let HirPatKind::Ident(_mode, _ident, _) = &sub_pat.kind {
                            let field_idx = starting_idx + i as u32;
                            let field_ty = payload_tys.get(i).cloned().unwrap_or_else(|| {
                                Ty::new(TyKind::Int(crate::ast::IntTy::I32), span)
                            });
                            let binding_local =
                                cx.mir.new_local(field_ty.clone(), None, sub_pat.span);
                            cx.local_map.insert(sub_pat.hir_id, binding_local);
                            cx.push_assign(
                                Place::local(binding_local, sub_pat.span),
                                Rvalue::Use(Operand::Copy(Place {
                                    kind: PlaceKind::Projection(
                                        Box::new(Place::local(scrut_local, span)),
                                        ProjectionElem::Field(FieldId(field_idx), field_ty),
                                    ),
                                    span: sub_pat.span,
                                })),
                                sub_pat.span,
                            );
                        }
                    }
                }
            }
            for p in sub_pats {
                lower_enum_variant_pattern_bindings(cx, scrut_local, p);
            }
        }
        HirPatKind::Struct(path, fields, _) => {
            if let Res::Def(enum_def_id, crate::resolve::DefKind::Enum) = path.res {
                if let Some((variant_idx, field_tys)) =
                    resolve_enum_variant(cx, enum_def_id, &path.segments[1].ident.name)
                {
                    let payload_tys = &field_tys[1..];
                    let starting_idx =
                        compute_enum_payload_starting_idx(cx, enum_def_id, variant_idx);
                    let hir = match cx.hir {
                        Some(h) => h,
                        None => return,
                    };
                    if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(enum_def))) =
                        hir.owner(enum_def_id)
                    {
                        if let Some(variant) = enum_def.variants.get(variant_idx as usize) {
                            if let crate::hir::HirVariantData::Struct(var_fields, _) = &variant.data
                            {
                                for field_pat in fields {
                                    if let Some(field_pos) = var_fields.iter().position(|f| {
                                        f.ident.map(|i| i.name) == Some(field_pat.ident.name)
                                    }) {
                                        if let HirPatKind::Ident(_mode, _ident, _) =
                                            &field_pat.pat.kind
                                        {
                                            let field_idx = starting_idx + field_pos as u32;
                                            let field_ty = payload_tys
                                                .get(field_pos)
                                                .cloned()
                                                .unwrap_or_else(|| {
                                                    Ty::new(
                                                        TyKind::Int(crate::ast::IntTy::I32),
                                                        span,
                                                    )
                                                });
                                            let binding_local = cx.mir.new_local(
                                                field_ty.clone(),
                                                None,
                                                field_pat.pat.span,
                                            );
                                            cx.local_map
                                                .insert(field_pat.pat.hir_id, binding_local);
                                            cx.push_assign(
                                                Place::local(binding_local, field_pat.pat.span),
                                                Rvalue::Use(Operand::Copy(Place {
                                                    kind: PlaceKind::Projection(
                                                        Box::new(Place::local(scrut_local, span)),
                                                        ProjectionElem::Field(
                                                            FieldId(field_idx),
                                                            field_ty,
                                                        ),
                                                    ),
                                                    span: field_pat.pat.span,
                                                })),
                                                field_pat.pat.span,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for f in fields {
                lower_enum_variant_pattern_bindings(cx, scrut_local, &f.pat);
            }
        }
        HirPatKind::Tuple(pats) => {
            for p in pats {
                lower_enum_variant_pattern_bindings(cx, scrut_local, p);
            }
        }
        _ => {}
    }
}

/// Stage 3.48 (L-ENUM-BINDING): Compute the starting field_idx for variant V's
/// payload in the flat enum storage layout.
///
/// Layout (per `mir_type_to_emit_type_with_layouts` Stage 3.48):
///   `{ discr, variant_0_fields..., variant_1_fields..., ... }`
/// (unit variants contribute no fields.)
///
/// `field_idx(variant_V, field_F) = 1 + sum(field_counts of variants 0..V-1) + F`
pub(crate) fn compute_enum_payload_starting_idx(
    cx: &MirLowerCtxt,
    enum_def_id: crate::hir::DefId,
    variant_idx: u32,
) -> u32 {
    let hir = match cx.hir {
        Some(h) => h,
        None => return 1,
    };
    let owner = match hir.owner(enum_def_id) {
        Some(o) => o,
        None => return 1,
    };
    let enum_def = match owner {
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => e,
        _ => return 1,
    };
    let mut idx = 1u32; // skip discriminant
    for (i, variant) in enum_def.variants.iter().enumerate() {
        if i as u32 >= variant_idx {
            break;
        }
        let field_count = match &variant.data {
            crate::hir::HirVariantData::Unit(_) => 0,
            crate::hir::HirVariantData::Tuple(fields, _) => fields.len(),
            crate::hir::HirVariantData::Struct(fields, _) => fields.len(),
        };
        idx += field_count as u32;
    }
    idx
}

/// Collect all HirIds from a pattern (for identifying closure params).
pub(crate) fn collect_pat_hir_ids(pat: &HirPat, out: &mut std::collections::HashSet<HirId>) {
    match &pat.kind {
        HirPatKind::Ident(_, _, sub) => {
            out.insert(pat.hir_id);
            if let Some(s) = sub {
                collect_pat_hir_ids(s, out);
            }
        }
        HirPatKind::Struct(_, fields, _) => {
            for f in fields {
                collect_pat_hir_ids(&f.pat, out);
            }
        }
        HirPatKind::TupleStruct(_, pats) => {
            for p in pats {
                collect_pat_hir_ids(p, out);
            }
        }
        HirPatKind::Tuple(pats) => {
            for p in pats {
                collect_pat_hir_ids(p, out);
            }
        }
        HirPatKind::Slice(pats, rest) => {
            for p in pats {
                collect_pat_hir_ids(p, out);
            }
            if let Some(r) = rest {
                collect_pat_hir_ids(r, out);
            }
        }
        HirPatKind::Or(pats) => {
            for p in pats {
                collect_pat_hir_ids(p, out);
            }
        }
        HirPatKind::Ref(p, _) => {
            collect_pat_hir_ids(p, out);
        }
        HirPatKind::Path(_)
        | HirPatKind::Lit(_)
        | HirPatKind::Wild
        | HirPatKind::Rest
        | HirPatKind::Range(_, _, _) => {}
    }
}
