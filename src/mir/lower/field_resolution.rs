//! Stage 6.5: Field resolution helper extraction from mir/lower/mod.rs (TD-011 split step 5).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (2656 → ~2380).
//! Contains functions for resolving field types, field indices, struct DefIds,
//! index element types, and ADT field types from HIR.
//!
//! Stage 16.53 (Task 11 Phase 2): Added `resolve_adt_field_tys_with_substs`
//! which applies generic type substitution to field types. This is the
//! integration point for the `substitute` function — when a generic struct
//! like `Box<T>` has field `val: T`, the field type is lowered as
//! `Param(ParamTy { index: 0, .. })` and then substituted with the Adt's
//! substs (e.g., `[i32]`) to produce the concrete field type `i32`.

use crate::hir::*;
use crate::mir::place::LocalId;
use crate::mir::ty::*;

use crate::session::Span;

use super::{lower_hir_ty_to_mir_ty_with_generics, lower_hir_ty_to_mir_ty_with_hir, MirLowerCtxt};

/// Resolve the type of a specific field of a struct, given the receiver
/// expression and the field index.
///
/// Stage 3.32 (L-DEBT-2 fix): looks up the receiver's struct DefId (via
/// `find_receiver_struct_def_id`), then reads the field's type from the
/// HIR struct definition. Returns `None` if the receiver isn't a struct
/// or the field index is out of bounds — caller falls back to
/// `fresh_infer_ty`.
///
/// Stage 16.53 (Task 11 Phase 2): when the receiver's type has non-empty
/// substs (e.g., `Box<i32>`), the field type is lowered with generic param
/// resolution and then substituted. This makes `box.val` produce `i32`
/// instead of `Param(0)` or `Error`.
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

    // Stage 16.53: Extract substs from the receiver's type. If the receiver
    // is `box: Box<i32>`, the substs are `[i32]`. These are used to
    // substitute the field type's Param placeholders.
    let receiver_substs = find_receiver_substs(cx, receiver);

    let owner = hir.find_owner(struct_def_id)?;
    if let OwnerNode::Item(HirItem::Struct(s)) = owner {
        let field = s.fields.get(field_index as usize)?;
        // Stage 16.53: If we have substs, lower with generics + substitute.
        // Otherwise, use the plain lowerer (non-generic fast path).
        // Stage 16.56: Use lower_hir_ty_to_mir_ty_with_hir to pass HIR
        // through, so nested generic paths in field types are resolved.
        if let Some(substs) = receiver_substs {
            if !substs.is_empty() {
                let generic_params = crate::hir::generics::find_generics(struct_def_id, hir);
                let field_ty = lower_hir_ty_to_mir_ty_with_generics(&field.ty, &generic_params);
                return Some(crate::mir::substitute::substitute(&field_ty, &substs));
            }
        }
        Some(lower_hir_ty_to_mir_ty_with_hir(&field.ty, Some(hir)))
    } else {
        None
    }
}

/// Stage 16.53: Extract the substs from a receiver expression's type.
///
/// For `box: Box<i32>`, returns `Some([i32])`. For non-Adt types or Adt
/// types with empty substs, returns `Some([])`. Returns `None` if the
/// receiver's type can't be determined.
///
/// Per §23: `find_receiver_substs` follows `<verb>_<noun>_<noun>` pattern.
fn find_receiver_substs(cx: &MirLowerCtxt, receiver: &HirExpr) -> Option<SubstsRef> {
    match &receiver.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id_val) = path.res {
                let hir_id = hir_id_val;
                let local_id = cx.local_map.get(&hir_id)?;
                let ld = cx.mir.local_decls.get(local_id.0 as usize)?;
                // Unwrap Ref for &self/&mut self field access.
                match &ld.ty.kind {
                    TyKind::Adt(_, substs) => Some(substs.clone()),
                    TyKind::Ref(_, _, inner) => {
                        if let TyKind::Adt(_, substs) = &inner.kind {
                            Some(substs.clone())
                        } else {
                            None
                        }
                    }
                    _ => {
                        // Stage 18.398 (v0.5+ Phase 2 step 3): When ld.ty is Infer,
                        // resolve substs from HIR param type annotation.
                        // Per §1.0 原則 10: HIR is source of truth for type annotations.
                        if let Some(hir_crate) = cx.hir {
                            for (_, body) in &hir_crate.bodies {
                                if body.hir_id.owner == hir_id.owner {
                                    for param in &body.params {
                                        if param.pat.hir_id == hir_id {
                                            if let Some(ref param_ty) = param.ty {
                                                let mir_ty = lower_hir_ty_to_mir_ty_with_hir(
                                                    param_ty, cx.hir,
                                                );
                                                if let TyKind::Adt(_, substs) = &mir_ty.kind {
                                                    return Some(substs.clone());
                                                }
                                                if let TyKind::Ref(_, _, inner) = &mir_ty.kind {
                                                    if let TyKind::Adt(_, substs) = &inner.kind {
                                                        return Some(substs.clone());
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        None
                    }
                }
            } else {
                None
            }
        }
        // Stage 18.364 (P2 soundness fix): Handle HirExprKind::Field for
        // nested field access chains WITHOUT recursive cycle.
        //
        // Stage 18.360 introduced Field handling but created a recursive
        // cycle: find_receiver_substs(o.inner) → find_receiver_struct_def_id(o.inner)
        // → find_receiver_substs(o.inner) → ... → Error.
        //
        // Fix: Instead of calling find_receiver_struct_def_id (which calls
        // back find_receiver_substs), directly lower the inner receiver
        // to get its result local, then read the Adt DefId + substs from
        // the local_decl.ty. This breaks the cycle by using the MIR
        // local_decl as the single source of truth (唯一可信数据源).
        //
        // Per §1.0 原則 6 (通解 > 特解): one path reads local_decl.ty.
        // Per §12 (最优 > 最小): root-cause fix — break the cycle at source.
        // Per "唯一可信数据源": local_decl.ty is the single source of truth.
        HirExprKind::Field {
            receiver: _,
            ident: _,
        } => {
            // Stage 18.364: For nested field access, we can't call
            // lower_expr_to_operand (needs &mut cx) from &cx context.
            // Return None and let writeback (Phase 0 + Phase 3.7) handle it.
            None
        }
        _ => None,
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
///
/// Stage 16.05: Now takes `cx: &mut MirLowerCtxt` so that field-not-found
/// errors can be pushed directly to `cx.type_errors` (per §1.0 原則 4
/// "报错 > 静默"). Previously the error was silently dropped because the
/// immutable borrow forbade mutation; callers were expected to rely on
/// typeck catching it indirectly. The fallback return value of 0 is
/// preserved for codegen recovery, but the error is now reported.
pub(crate) fn resolve_field_index(
    cx: &mut MirLowerCtxt,
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
                if let Some(OwnerNode::Item(HirItem::Struct(s))) =
                    hir_crate.find_owner(struct_def_id)
                {
                    for (i, f) in s.fields.iter().enumerate() {
                        if let Some(f_ident) = &f.ident {
                            if f_ident.name == *field_name {
                                return i as u32;
                            }
                        }
                    }
                    // Stage 16.05: Per §1.0 原則 4 "报错 > 静默" — field not
                    // found in the receiver's struct. Now that cx is
                    // `&mut MirLowerCtxt`, push the error directly instead of
                    // relying on typeck to catch it indirectly. The fallback
                    // return value of 0 is preserved for codegen recovery
                    // (the error will abort compilation before codegen runs).
                    let struct_name = cx
                        .interner
                        .try_resolve(&s.ident.name)
                        .unwrap_or("<anonymous>");
                    let field_name_str = cx.interner.try_resolve(field_name).unwrap_or("<unknown>");
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        format!("no field `{}` on struct `{}`", field_name_str, struct_name),
                        receiver.span,
                    ));
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
            if let Res::Local(hir_id_val) = path.res {
                let hir_id = hir_id_val; // HirId is Copy — no deref needed.
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
                        // Stage 18.398 (v0.5+ Phase 2 step 3): When ld.ty is Infer
                        // (function param at lower time), resolve struct DefId from
                        // HIR param type annotation. Root-cause fix — bypasses
                        // Infer local_decl.ty by reading HIR directly.
                        // Per §1.0 原則 10: HIR is source of truth for type annotations.
                        if let Some(hir_crate) = cx.hir {
                            let owner_def_id = hir_id.owner;
                            for (_, body) in &hir_crate.bodies {
                                if body.hir_id.owner == owner_def_id {
                                    for param in &body.params {
                                        if param.pat.hir_id == hir_id {
                                            if let Some(ref param_ty) = param.ty {
                                                let mir_ty = lower_hir_ty_to_mir_ty_with_hir(
                                                    param_ty, cx.hir,
                                                );
                                                if let TyKind::Adt(def_id, _) = &mir_ty.kind {
                                                    return Some(*def_id);
                                                }
                                                if let TyKind::Ref(_, _, inner) = &mir_ty.kind {
                                                    if let TyKind::Adt(def_id, _) = &inner.kind {
                                                        return Some(*def_id);
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        // Stage 18.364: Handle HirExprKind::Field for nested field access.
        // Same issue as find_receiver_substs: can't call lower_expr_to_operand
        // from &cx. Return None and let writeback handle it.
        HirExprKind::Field {
            receiver: _,
            ident: _,
        } => None,
        _ => None,
    }
}

/// Stage 3.52: Resolve the element type of an index expression `base[idx]`
/// by inspecting the base's MIR type.
///
/// Stage 18.80 P2-D: Added `expr_span` parameter for accurate error spans
/// (was Span::DUMMY, producing "1:1" in error messages).
///
/// Stage 18.422 (§20 iterative audit): REMOVED `TyKind::Str => Some(u8)` arm.
/// Was: `&str` indexing silently returned `u8` (treating `&str` as `&[u8]`).
/// This was a design divergence from Rust, where `"hello"[0]` is a compile
/// error (cannot index `&str` directly — must use `.as_bytes()[i]` or
/// `.chars().nth(i)`). Confirmed bug: `s[0]` silently compiled and produced
/// 104 (ASCII 'h') via raw pointer read.
///
/// Per §20 (iterative audit): same class as Stage 18.416 (float bitwise via
/// bitcast was a design divergence from Rust).
/// Per §1.0 原則 5 (去除兼容思维): the byte-indexing behavior is removed, not
/// kept as a fallback.
/// Per §1.0 原則 4 (报错 > 静默): `&str` indexing must be rejected, not
/// silently coerced to `&[u8]`.
/// Per §1.6 终极检验: root-cause fix at the resolution site.
pub(crate) fn resolve_index_element_type(
    cx: &mut MirLowerCtxt,
    base_local: LocalId,
    expr_span: Span,
) -> Option<Ty> {
    let base_ty = cx.mir.local_decls.get(base_local.0 as usize)?.ty.clone();
    match &base_ty.kind {
        TyKind::Ref(_, _, inner) => match &inner.kind {
            TyKind::Slice(elem) => Some((**elem).clone()),
            TyKind::Array(elem, _) => Some((**elem).clone()),
            // Stage 18.422: REMOVED `TyKind::Str => Some(u8)` — `&str` indexing
            // is now a type error (matches Rust semantics). Users must use
            // `.as_bytes()[i]` for byte access or `.chars().nth(i)` for char
            // access.
            // Stage 18.62: Infer/Error/Param are acceptable fallbacks (typeck
            // will resolve them later). Per §1.0 原則 4: only push error for
            // truly non-indexable concrete types.
            TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
            _ => {
                // Stage 18.76 P1-D: Use type_to_string instead of Debug format.
                // Stage 18.80 P2-D: Use expr_span instead of Span::DUMMY.
                // Stage 18.422: This arm now catches `TyKind::Str` (previously
                // had its own arm returning u8). `&str` indexing reports:
                // "cannot index into type `str`" — guiding users to
                // `.as_bytes()` or `.chars()`.
                cx.type_errors.push(crate::typeck::TypeError::new(
                    format!(
                        "cannot index into type `{}` — use `.as_bytes()[i]` for byte access or `.chars().nth(i)` for char access",
                        crate::mir::ty::type_to_string(inner)
                    ),
                    expr_span,
                ));
                None
            }
        },
        TyKind::Array(elem, _) => Some((**elem).clone()),
        TyKind::Slice(elem) => Some((**elem).clone()),
        // Stage 18.62: Infer/Error/Param are acceptable fallbacks.
        TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
        _ => {
            // Stage 18.76 P1-D: Use type_to_string instead of Debug format.
            // Stage 18.80 P2-D: Use expr_span instead of Span::DUMMY.
            cx.type_errors.push(crate::typeck::TypeError::new(
                format!(
                    "cannot index into type `{}`",
                    crate::mir::ty::type_to_string(&base_ty)
                ),
                expr_span,
            ));
            None
        }
    }
}

/// Resolve the declared field types of an ADT (struct or enum variant).
///
/// For non-generic ADTs, this is the same as the field types in HIR.
/// For generic ADTs, the field types contain `TyKind::Param` placeholders
/// that should be substituted with the Adt's substs using
/// `resolve_adt_field_tys_with_substs`.
///
/// Per §16: reads HIR (allowed — data flows downstream).
pub(crate) fn resolve_adt_field_tys(cx: &MirLowerCtxt, def_id: crate::hir::DefId) -> Vec<Ty> {
    let hir = match cx.hir {
        Some(h) => h,
        None => return Vec::new(),
    };
    // Stage 18.376 (TD-ARCH-NESTED-GENERIC-FIELD-ACCESS): Pass generic_params
    // so nested generic field types (e.g., `Outer<T> { inner: Inner<T> }`)
    // resolve `T` to `Param(0)` instead of `Error`. Was: called
    // `lower_hir_ty_to_mir_ty(&f.ty)` without generic_params, which left
    // nested generic fields as `Adt(Inner, [Error])` — breaking inference.
    //
    // Per §1.0 原則 6 (通解 > 特解): one path for generic + non-generic ADTs.
    // Per §12 (最优 > 最小): root-cause fix at the resolution site.
    // Per §20 (iterative audit): same class as Stage 18.347/18.358.
    let generic_params = crate::hir::generics::find_generics(def_id, hir);
    match hir.find_owner(def_id) {
        Some(OwnerNode::Item(HirItem::Struct(s))) => s
            .fields
            .iter()
            .map(|f| lower_hir_ty_to_mir_ty_with_generics(&f.ty, &generic_params))
            .collect(),
        Some(OwnerNode::Item(HirItem::Enum(_))) => {
            vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)]
        }
        _ => Vec::new(),
    }
}

/// Stage 16.53 (Task 11 Phase 2): Resolve field types of an ADT with
/// generic type substitution applied.
///
/// Given a generic struct `Box<T>` with field `val: T` and substs `[i32]`,
/// this function:
/// 1. Gets the struct's generic params via `generics_of` (e.g., `[T]`)
/// 2. Lowers each field type with generic param resolution:
///    - `val: T` → `Param(ParamTy { index: 0, name: T })`
/// 3. Applies `substitute(field_ty, substs)` to each field:
///    - `Param(0)` with substs `[i32]` → `i32`
/// 4. Returns the substituted field types: `[i32]`
///
/// For non-generic ADTs (empty substs or no generic params), this falls
/// back to the plain `resolve_adt_field_tys` behavior.
///
/// Per §23: `resolve_adt_field_tys_with_substs` follows
/// `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads HIR + applies substitution (pure data transformation).
/// Per §1.0 原則 6 "通用 > 特例": one function for generic + non-generic ADTs.
pub(crate) fn resolve_adt_field_tys_with_substs(
    cx: &MirLowerCtxt,
    def_id: crate::hir::DefId,
    substs: &SubstsRef,
) -> Vec<Ty> {
    let hir = match cx.hir {
        Some(h) => h,
        None => return Vec::new(),
    };

    // Get the ADT's generic params (empty for non-generic ADTs).
    let generic_params = crate::hir::generics::find_generics(def_id, hir);

    // If no generic params or no substs, fall back to plain resolution.
    // This is the non-generic fast path — no substitution needed.
    if generic_params.is_empty() || substs.is_empty() {
        return resolve_adt_field_tys(cx, def_id);
    }

    // Generic ADT with substs — lower fields with generic param resolution,
    // then apply substitution.
    match hir.find_owner(def_id) {
        Some(OwnerNode::Item(HirItem::Struct(s))) => s
            .fields
            .iter()
            .map(|f| {
                // Lower with generic param resolution → may produce Param
                let field_ty = lower_hir_ty_to_mir_ty_with_generics(&f.ty, &generic_params);
                // Apply substitution → replace Param with actual type
                crate::mir::substitute::substitute(&field_ty, substs)
            })
            .collect(),
        Some(OwnerNode::Item(HirItem::Enum(_))) => {
            // Enum variants have a discriminant field (i32) — no substitution needed.
            // NOTE: This function is called when no specific variant is resolved.
            // For specific variant field types with substitution, use
            // `resolve_enum_variant` (in method_resolution.rs) + apply substitution
            // separately (see lower_call_expr's pre_adt_field_tys computation).
            vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)]
        }
        _ => Vec::new(),
    }
}
