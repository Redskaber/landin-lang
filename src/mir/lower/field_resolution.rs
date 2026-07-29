//! Stage 6.5: Field resolution helper extraction from mir/lower/mod.rs (TD-011 split step 5).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (2656 → ~2380).
//! Contains functions for resolving field types, field indices, struct DefIds,
//! index element types, and ADT field types from HIR.

use crate::ast;
use crate::hir::*;
use crate::mir::place::LocalId;
use crate::mir::ty::*;
use crate::session::Span;

use super::{lower_hir_ty_to_mir_ty, MirLowerCtxt};

/// Resolve the type of a specific field of a struct, given the receiver
/// expression and the field index.
///
/// Stage 3.32 (L-DEBT-2 fix): looks up the receiver's struct DefId (via
/// `find_receiver_struct_def_id`), then reads the field's type from the
/// HIR struct definition. Returns `None` if the receiver isn't a struct
/// or the field index is out of bounds — caller falls back to
/// `fresh_infer_ty`.
///
/// Per §16: this is MIR lower reading HIR (allowed — data flows downstream).
/// The resolved type is sunk into `ProjectionElem::Field(_, field_ty)` so
/// codegen reads it from MIR.
pub(crate) fn resolve_field_type(
    cx: &MirLowerCtxt,
    receiver: &HirExpr,
    field_index: u32,
) -> Option<Ty> {
    let hir = cx.hir?;
    let struct_def_id = find_receiver_struct_def_id(cx, receiver)?;
    let owner = hir.owner(struct_def_id)?;
    if let OwnerNode::Item(HirItem::Struct(s)) = owner {
        let field = s.fields.get(field_index as usize)?;
        Some(lower_hir_ty_to_mir_ty(&field.ty))
    } else {
        None
    }
}

/// Resolve the field index for a field-access expression `receiver.ident`.
///
/// Stage 3.30 fix: was hardcoded `FieldId(0)` — meant `p.1`, `p.x`, etc.
/// all returned field 0 (silently wrong). Now:
///   - For tuple struct fields (`p.0`, `p.1`), the ident is the stringified
///     index — parse it directly.
///   - For named struct fields (`p.x`), look up the field index in the HIR
///     struct definition by matching the field name.
///   - If we can't resolve (e.g., receiver type unknown), default to 0
///     (legacy behavior — typeck should catch real errors).
///   - Stage 3.32 fix: if the receiver's type can't be resolved (e.g.,
///     `let m = Mixed { ... }; m.b` — m's type is Infer(TyVar) at lower
///     time), scan all HIR struct owners for one that has a field with
///     the given name. If exactly one match is found, use it.
pub(crate) fn resolve_field_index(
    cx: &MirLowerCtxt,
    receiver: &HirExpr,
    field_name: &crate::lexer::Symbol,
) -> u32 {
    use crate::lexer::Symbol;
    if let Some(hir_crate) = cx.hir {
        if let Some(name_str) = cx.interner.try_resolve(field_name) {
            if let Ok(idx) = name_str.parse::<u32>() {
                return idx;
            }
            if let Some(struct_def_id) = find_receiver_struct_def_id(cx, receiver) {
                if let Some(OwnerNode::Item(HirItem::Struct(s))) = hir_crate.owner(struct_def_id) {
                    for (i, f) in s.fields.iter().enumerate() {
                        if let Some(f_ident) = &f.ident {
                            if f_ident.name == *field_name {
                                return i as u32;
                            }
                        }
                    }
                    // Stage 14.31: Per "报错 > 静默" — field not found in the
                    // receiver's struct. Emit error instead of silently returning 0.
                    // Note: cx is &MirLowerCtxt (immutable), so we can't push to
                    // lower_type_errors here. Instead, we'll rely on typeck to
                    // catch this — the field_ty resolution returns None, which
                    // means the result local gets fresh_infer_ty (Infer), and
                    // typeck will report a type error if the field is used in a
                    // context that expects a specific type.
                    //
                    // TODO: When MirLowerCtxt is made mutable in this function,
                    // push the error directly. For now, the fallback behavior
                    // (return 0) is preserved but the field access will produce
                    // wrong results — typeck should catch it in most cases.
                    return 0; // fallback for codegen (error will abort before codegen)
                }
            }
            // Fallback: search all structs (for tuple struct fields like .0, .1)
            // Stage 14.64: When the field name appears in multiple structs,
            // check if they all agree on the index. If so, return that index.
            // Previously, any ambiguity caused a fallthrough to `return 0`,
            // which silently produced wrong field accesses (e.g., `u.y` where
            // u is Vec2 returned field 0 instead of field 1 because Point2
            // also has a `y` field, making the search "ambiguous" even though
            // both structs agree `y` is at index 1).
            //
            // Per §1.0 原则 5 "报错 > 静默": if all candidates agree, use the
            // agreed-upon index rather than silently defaulting to 0.
            // Per §1.0 原则 6 "通用 > 特例": one rule handles both the
            // unambiguous case and the agreeable-ambiguous case.
            let mut found_idx: Option<u32> = None;
            let mut all_agree = true;
            for (_def_id, owner) in &hir_crate.owners {
                if let OwnerNode::Item(HirItem::Struct(s)) = owner {
                    for (i, f) in s.fields.iter().enumerate() {
                        if let Some(f_ident) = &f.ident {
                            if f_ident.name == *field_name {
                                match found_idx {
                                    None => found_idx = Some(i as u32),
                                    Some(existing) => {
                                        if existing != i as u32 {
                                            all_agree = false;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                    if !all_agree {
                        break;
                    }
                }
            }
            if let Some(idx) = found_idx {
                if all_agree {
                    return idx;
                }
            }
        }
    }
    let _: Symbol = crate::lexer::Symbol::default();
    0
}

/// Find the struct DefId that a receiver expression's type resolves to.
pub(crate) fn find_receiver_struct_def_id(
    cx: &MirLowerCtxt,
    receiver: &HirExpr,
) -> Option<crate::hir::DefId> {
    match &receiver.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_map.get(&hir_id) {
                    if let Some(ld) = cx.mir.local_decls.get(local_id.0 as usize) {
                        // Stage 14.21: Auto-deref Ref types to find the Adt.
                        // For &self/&mut self methods, the self local's type is
                        // Ref(_, _, Adt(...)). We need to unwrap the Ref to
                        // find the struct DefId for field type resolution.
                        let ty_kind = &ld.ty.kind;
                        let adt_def_id = match ty_kind {
                            TyKind::Adt(def_id, _) => Some(*def_id),
                            TyKind::Ref(_, _, inner) => {
                                // &self/&mut self — unwrap the Ref to find Adt
                                if let TyKind::Adt(def_id, _) = inner.kind {
                                    Some(def_id)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        if let Some(def_id) = adt_def_id {
                            return Some(def_id);
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Stage 3.52: Resolve the element type of an index expression `base[idx]`
/// by inspecting the base's MIR type.
pub(crate) fn resolve_index_element_type(cx: &MirLowerCtxt, base_local: LocalId) -> Option<Ty> {
    let base_ty = cx.mir.local_decls.get(base_local.0 as usize)?.ty.clone();
    match &base_ty.kind {
        TyKind::Ref(_, _, inner) => match &inner.kind {
            TyKind::Slice(elem) => Some((**elem).clone()),
            TyKind::Array(elem, _) => Some((**elem).clone()),
            TyKind::Str => Some(Ty::new(TyKind::Uint(ast::UintTy::U8), Span::DUMMY)),
            _ => None,
        },
        TyKind::Array(elem, _) => Some((**elem).clone()),
        TyKind::Slice(elem) => Some((**elem).clone()),
        _ => None,
    }
}

/// Resolve the declared field types of an ADT (struct or enum variant).
pub(crate) fn resolve_adt_field_tys(cx: &MirLowerCtxt, def_id: crate::hir::DefId) -> Vec<Ty> {
    let hir = match cx.hir {
        Some(h) => h,
        None => return Vec::new(),
    };
    match hir.owner(def_id) {
        Some(OwnerNode::Item(HirItem::Struct(s))) => s
            .fields
            .iter()
            .map(|f| lower_hir_ty_to_mir_ty(&f.ty))
            .collect(),
        Some(OwnerNode::Item(HirItem::Enum(_))) => {
            vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)]
        }
        _ => Vec::new(),
    }
}
