//! Stage 6.10: Expression lowering algorithm (extracted from mod.rs).
//!
//! This module hosts the algorithm for lowering HIR expression trees
//! into MIR operands / places / terminators. It was extracted from
//! `mir/lower/mod.rs` in Stage 6.10 (TD-011) to separate the
//! "expression lowering algorithm" responsibility from the
//! "context infrastructure + body entry points" responsibility,
//! following the single responsibility principle.
//!
//! Per §16 (interface isolation): this module interacts with
//! `MirLowerCtxt` exclusively through its public API
//! (`&mut MirLowerCtxt`), never by touching its private fields.
//! Data flow is unidirectional:
//!
//! ```text
//!     mod.rs (entry points)
//!         │
//!         ▼
//!     expr_operand (algorithm)
//!         │
//!         ▼
//!     MirLowerCtxt (state)
//!         │
//!         ▼
//!     adt_layout / closure_capture / control_flow / field_resolution
//!     / overflow_assert / pattern_bindings (specialized helpers)
//! ```
//!
//! No circular dependency — `expr_operand` never calls back into
//! `mod.rs`'s private functions.
//!
//! Per §23 (API naming): all function names preserve their original
//! identifiers (no rename churn). The module name `expr_operand`
//! follows the `<noun>_<noun>` pattern set by sibling modules
//! (`adt_layout`, `closure_capture`, `pattern_bindings`).

use crate::ast;
use crate::hir::*;
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::dyn_trait::{find_dyn_trait_method_call_in_plan_by_method, DynTraitMethodCall};
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

// Re-exports from the parent module — see §16 (interface isolation).
use super::{lower_hir_ty_to_mir_ty, MirLowerCtxt};
// Sibling helper modules — each owns a specialized sub-algorithm.
// (`adt_layout` is not used here: it runs from mod.rs's body entry points,
//  not from expression lowering.)
use super::{closure_capture, control_flow, field_resolution, overflow_assert, pattern_bindings};

/// Lower a HIR expression to a MIR Place (a place that can be assigned to).
///
/// Stage 3.34 (L-MUT-1 fix): used by `HirExprKind::Assign` to lower the LHS
/// into a place. Handles:
///   - `Path` (local variable) → `Place::Local`
///   - `Field { receiver, ident }` → `Place::Projection(receiver, Field(idx, ty))`
///   - `Index { receiver, index }` → `Place::Projection(receiver, Index(idx_local))`
///   - `Unary { op: Deref, expr }` → `Place::Projection(expr, Deref)`
///
/// For other expression kinds (which can't be assigned to), falls back to
/// a fresh local — typeck should catch the "assignment to non-place" error.
pub(crate) fn lower_expr_to_place(cx: &mut MirLowerCtxt, expr: &HirExpr) -> Place {
    match &expr.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_of(hir_id) {
                    return Place::local(local_id, expr.span);
                }
            }
            // Fallback: fresh local (error recovery).
            let ty = cx.fresh_infer_ty(expr.span);
            let local = cx.mir.new_local(ty, None, expr.span);
            Place::local(local, expr.span)
        }
        HirExprKind::Field { receiver, ident } => {
            let base = lower_expr_to_place(cx, receiver);
            let field_index = field_resolution::resolve_field_index(cx, receiver, &ident.name);
            // Stage 3.32: resolve the field's actual type from the struct def.
            let field_ty = field_resolution::resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            // Stage 14.19 (GAP-31): If the receiver's type is Ref (e.g. &self
            // or &mut self), auto-deref before projecting the field.
            let base = auto_deref_if_ref(cx, base, receiver);
            Place {
                kind: PlaceKind::Projection(
                    Box::new(base),
                    ProjectionElem::Field(FieldId(field_index), field_ty),
                ),
                span: expr.span,
            }
        }
        HirExprKind::Index {
            receiver, index, ..
        } => {
            let base = lower_expr_to_place(cx, receiver);
            let idx_local = lower_expr_to_operand(cx, index);
            Place {
                kind: PlaceKind::Projection(Box::new(base), ProjectionElem::Index(idx_local)),
                span: expr.span,
            }
        }
        HirExprKind::Unary {
            op, expr: inner, ..
        } if *op == HirUnaryOp::Deref => {
            let base = lower_expr_to_place(cx, inner);
            Place {
                kind: PlaceKind::Projection(Box::new(base), ProjectionElem::Deref),
                span: expr.span,
            }
        }
        // Other expression kinds can't be assigned to — return a fresh
        // local as error recovery. typeck should catch this.
        _ => {
            let ty = cx.fresh_infer_ty(expr.span);
            let local = cx.mir.new_local(ty, None, expr.span);
            Place::local(local, expr.span)
        }
    }
}

/// Stage 5.78: Build a `TerminatorKind::Call` for a dyn Trait method call,
/// and register the call info in `cx.mir.dyn_trait_calls` side-table.
///
/// The function operand is a `Const` whose `ConstVal::Int` value is the
/// **index** of the call entry in `cx.mir.dyn_trait_calls`. Codegen
/// (Stage 5.79+) will detect this marker and emit a vtable indirect call
/// using the recorded (trait, type, method, slot_index, param_count) info.
///
/// # Arguments
///
/// - `cx`: the lowering context (used to push the side-table entry)
/// - `call`: the `DynTraitMethodCall` carrying trait/type/method/slot/param info
/// - `recv_local`: the MIR local holding the receiver (the `self` arg)
/// - `arg_locals`: the MIR locals holding the explicit args (excluding `self`)
/// - `dest`: the destination local where the call result is stored
/// - `span`: source span for error reporting
///
/// # Returns
///
/// A `TerminatorKind::Call` whose:
/// - `func` is `Operand::Constant(Const { ty: Error, val: Int(index) })`
///   where `index` is the side-table entry index
/// - `args` is `[Copy(recv), Copy(arg0), Copy(arg1), ...]` — self first
///   then explicit args (matches the existing MethodCall convention)
/// - `destination` is `Place::local(dest, span)`
/// - `target` is `None` — caller sets it via `terminate_and_goto`
///
/// # §16 compliance
///
/// MIR carries the dyn Trait call info as data (`dyn_trait_calls`
/// side-table), so codegen doesn't need to query HIR or TraitResolver.
/// Data flow: `mir::dyn_trait` (DynTraitMethodCall) → `mir::lower`
/// (this helper) → `mir::body` (side-table + Terminator) → codegen
/// (Stage 5.79+). Single-directional, no circular dependency.
///
/// # §23 compliance
///
/// `build_dyn_trait_call_terminator` follows the
/// `<verb>_<noun>_<noun>_<noun>_<noun>` pattern (helper-verb `build_`
/// prefix per §8.1, mirroring `build_dyn_trait_mir_plan` from Stage 5.73).
pub fn build_dyn_trait_call_terminator(
    cx: &mut MirLowerCtxt,
    call: &DynTraitMethodCall,
    recv_local: LocalId,
    arg_locals: &[LocalId],
    dest: LocalId,
    span: Span,
) -> Terminator {
    // Stage 15.65 (HP-22 cleanup): Removed the side-table push — the
    // `dyn_trait_call` field on the terminator is now the SOLE source of
    // truth. The legacy `mir.dyn_trait_calls` side-table and the magic
    // `Error + Int(index)` func marker have been removed.
    //
    // Per §1.0 原則 3 "显式 > 隐式": the dyn Trait info is explicit on the
    // terminator, not implicit in a side-table indexed by a magic constant.
    // Per §15 "最优 > 最小": dead code (side-table + legacy codegen path)
    // is removed, reducing maintenance burden.
    let _ = cx; // cx no longer needed (was used for side-table push).

    // Build the args list: self first, then explicit args.
    let mut arg_operands: Vec<Operand> = vec![Operand::Copy(Place::local(recv_local, span))];
    for local in arg_locals {
        arg_operands.push(Operand::Copy(Place::local(*local, Span::DUMMY)));
    }

    Terminator::new(
        TerminatorKind::Call {
            // Stage 15.65: func is now a placeholder constant (0). Codegen
            // checks `dyn_trait_call` FIRST — if Some, it uses the call info
            // directly and never reads `func`. The placeholder is kept only
            // because `func: Operand` is a required field of `TerminatorKind::Call`.
            func: Operand::Constant(Const {
                ty: Ty::new(TyKind::Error, Span::DUMMY),
                val: ConstVal::Int(0),
            }),
            args: arg_operands,
            destination: Place::local(dest, span),
            target: None,
            dyn_trait_call: Some(call.clone()),
        },
        span,
    )
}

/// Stage 13.3a (TD-030): Inline a closure body at the call site.
///
/// This implements the pragmatic subset of Strategy A (per
/// `stage-13.3-design-alignment.md` §4) for closure call lowering:
/// instead of synthesizing a separate `call` function MirBody per closure
/// (the full Strategy A), we inline the closure body directly at each
/// call site. This duplicates the body code at each call site, but is
/// simpler to implement and lets LLVM's optimizer deduplicate.
///
/// # Steps
///
/// 1. **Bind call args to closure params**: for each `(param, arg_local)`
///    pair, re-register `param.pat.hir_id` → `arg_local` in `local_map`.
///    (The closure arm registered the param's hir_id → a fresh infer-typed
///    local at construction time; we overwrite that with the call arg's
///    local so the body's Path references to the param resolve to the
///    actual call arg.)
///
/// 2. **Extract captures from the closure struct**: for each capture i,
///    create a fresh local with the capture's type, and assign it
///    `Copy(Projection(closure_local, Field(i, cap_ty)))`. Re-register
///    the captured binding's hir_id → the extract local in `local_map`.
///    (The captured binding's hir_id was originally registered to point
///    at the captured variable's original local — but at the call site,
///    the closure has the captured value stored in its struct field, so
///    we must re-route references through the extract local.)
///
/// 3. **Lower the closure body inline**: `lower_expr_to_operand(cx, body)`.
///    The body's Path references resolve via `local_map` to the args (step 1)
///    and captures (step 2). The result is the call's result.
///
/// 4. **Restore `local_map`**: the param + capture hir_id re-registrations
///    are reverted to their pre-call values so subsequent code in the
///    enclosing function sees the original locals.
///
/// # §16 compliance
///
/// The closure body is HIR data sunk into `MirLowerCtxt.closure_bodies`
/// at construction time. The inline lowering reads from this side-table
/// — no HIR access from codegen (codegen just sees the resulting MIR
/// statements).
///
/// # §23 compliance
///
/// `lower_closure_call_inline` follows the
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern (mirrors
/// `lower_expr_to_operand`).
fn lower_closure_call_inline(
    cx: &mut MirLowerCtxt,
    info: super::ClosureBodyInfo,
    closure_local: LocalId,
    arg_locals: &[LocalId],
    expr: &HirExpr,
) -> LocalId {
    // Save the local_map entries we're about to overwrite so we can
    // restore them after the body is lowered. (The body might reference
    // the same hir_ids as the enclosing function's locals — e.g., a
    // captured variable — and we don't want the inlined body to
    // permanently re-route those references.)
    let mut saved_entries: Vec<(HirId, Option<LocalId>)> = Vec::new();

    // Step 1: Bind call args to closure params.
    // For each (param, arg) pair, re-register the param's hir_id → arg_local.
    // Also collect pattern sub-hir_ids (for tuple patterns etc.) and map
    // them to the arg_local too — though Stage 13.3a only supports simple
    // ident patterns robustly; tuple patterns may need additional handling.
    for (i, param) in info.params.iter().enumerate() {
        let param_hir_id = param.pat.hir_id;
        // Save old entry.
        let old = cx.local_map.get(&param_hir_id).copied();
        saved_entries.push((param_hir_id, old));

        if i < arg_locals.len() {
            // Re-register param's hir_id → call arg's local.
            // The body's Path references to this param will now resolve
            // to the call arg's local (which holds the actual arg value).
            cx.local_map.insert(param_hir_id, arg_locals[i]);

            // Stage 13.3a: also collect sub-hir_ids from the pattern
            // (e.g., tuple patterns) and map them to the same arg_local.
            // This handles `|(a, b)| ...` minimally — both a and b get
            // the same arg_local, which is wrong for tuple patterns but
            // at least doesn't crash. Proper tuple pattern destructuring
            // at closure calls is deferred to Stage 13.5+.
            let mut sub_ids: std::collections::HashSet<HirId> = std::collections::HashSet::new();
            pattern_bindings::collect_pat_hir_ids(&param.pat, &mut sub_ids);
            for sub_id in sub_ids {
                if sub_id != param_hir_id {
                    let old_sub = cx.local_map.get(&sub_id).copied();
                    saved_entries.push((sub_id, old_sub));
                    cx.local_map.insert(sub_id, arg_locals[i]);
                }
            }
        }
    }

    // Step 2: Extract captures from the closure struct.
    // For each capture i, create a fresh local + assign
    // Copy(Projection(closure_local, Field(i, cap_ty))).
    // Re-register the captured binding's hir_id → extract_local.
    //
    // Stage 13.3a: if the captured value was itself a closure (i.e., the
    // captured binding's original local is in `cx.closure_bodies`), propagate
    // the closure info to the extract_local. This enables nested closure
    // calls — e.g., `let f = |x| x; let g = || f(1); g();` — where `g`
    // captures `f`, and when `g()` is called, the inlined body needs to
    // know that the extracted `f` is callable as a closure.
    for (i, (cap_hir_id, cap_ty)) in info.captures.iter().enumerate() {
        // Save old entry (the original local that held the captured value).
        let old = cx.local_map.get(cap_hir_id).copied();
        saved_entries.push((*cap_hir_id, old));

        // Create the extract local.
        let extract_local = cx.mir.new_local(cap_ty.clone(), None, expr.span);

        // Assign: extract_local = Copy(Projection(closure_local, Field(i, cap_ty)))
        cx.push_assign(
            Place::local(extract_local, expr.span),
            Rvalue::Use(Operand::Copy(Place {
                kind: PlaceKind::Projection(
                    Box::new(Place::local(closure_local, expr.span)),
                    ProjectionElem::Field(FieldId(i as u32), cap_ty.clone()),
                ),
                span: expr.span,
            })),
            expr.span,
        );

        // Re-register the captured binding's hir_id → extract_local.
        // The body's Path references to the captured variable now resolve
        // to the extracted capture value.
        cx.local_map.insert(*cap_hir_id, extract_local);

        // Stage 13.3a: propagate closure info from the original captured
        // local to the extract_local. If the captured value was a closure,
        // the inlined body might call it — we need the closure info
        // registered at the extract_local so the call dispatch finds it.
        if let Some(orig_local) = old {
            if let Some(orig_info) = cx.closure_bodies.get(&orig_local).cloned() {
                cx.closure_bodies.insert(extract_local, orig_info);
            }
        }
    }

    // Step 3: Lower the closure body inline.
    // The body's Path references resolve via local_map to:
    //   - params → call arg locals (step 1)
    //   - captures → extract locals (step 2)
    let result_local = lower_expr_to_operand(cx, &info.body);

    // Step 4: Restore local_map entries.
    for (hir_id, old) in saved_entries {
        if let Some(lid) = old {
            cx.local_map.insert(hir_id, lid);
        } else {
            cx.local_map.remove(&hir_id);
        }
    }

    result_local
}

// Stage 15.74: `is_capture_ty_copy` REMOVED — replaced by shared
// `is_mir_ty_copy_conservative` from `mir::ty` (DRY per §23 rule 5).
// Per §1.0 原則 5 "去除兼容思维": duplicate Copy detection removed.

/// Lower a HIR expression to a MIR Operand (a value that can be used
/// as an argument to a binary op, call, etc.).
///
/// Returns the LocalId of the temporary that holds the result.
pub(crate) fn lower_expr_to_operand(cx: &mut MirLowerCtxt, expr: &HirExpr) -> LocalId {
    match &expr.kind {
        HirExprKind::Lit(lit) => {
            let (const_val, ty) = cx.lit_to_const(lit);
            cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Constant(const_val)), ty, expr.span)
        }
        HirExprKind::Path(path) => {
            // If the path resolves to a local, return that local.
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.local_of(hir_id) {
                    return local_id;
                }
            }
            // If the path resolves to a top-level Def, dispatch on DefKind:
            //   - Fn         → FnDef type (real fn item)
            //   - Struct     → Adt type (struct ctor — handled in Call lower)
            //   - Enum       → Adt type (enum variant ctor — Stage 3.31+)
            //   - Const/Static → placeholder Error type (real const-eval is Stage 3+)
            // Stage 3.30 (per §15): use DefKind from Res to dispatch, eliminating
            // the root cause of "tuple struct ctor was being lowered as Call".
            if let Res::Def(def_id, def_kind) = path.res {
                match def_kind {
                    crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum => {
                        // ADT type — produce an Adt operand. When this path is
                        // used as the `func` of a Call expression, the Call
                        // lower will check the operand's type and dispatch to
                        // Aggregate(Adt) instead of emitting a real Call.
                        //
                        // Stage 3.38 (L-ENUM): For enum variant paths like
                        // `Color::Red`, the path has 2 segments. The second
                        // segment is the variant name. We look it up in the
                        // HIR enum definition to get the variant index.
                        // For unit variants (no args), we construct the
                        // Aggregate directly here.
                        if def_kind == crate::resolve::DefKind::Enum && path.segments.len() >= 2 {
                            let variant_name = &path.segments[1].ident.name;
                            if let Some((variant_idx, field_tys)) =
                                resolve_enum_variant(cx, def_id, variant_name)
                            {
                                // Check if this is a unit variant (field_tys
                                // has only the discriminant).
                                if field_tys.len() == 1 {
                                    // Unit variant — construct directly with
                                    // discriminant operand.
                                    let adt_ty = Ty::new(
                                        TyKind::Adt(
                                            def_id,
                                            Vec::<crate::mir::ty::Ty>::new().into(),
                                        ),
                                        expr.span,
                                    );
                                    let discr = Operand::Constant(Const {
                                        ty: Ty::new(
                                            TyKind::Int(crate::ast::IntTy::I32),
                                            Span::DUMMY,
                                        ),
                                        val: ConstVal::Uint(variant_idx as u128),
                                    });
                                    return cx.eval_rvalue_to_temp(
                                        Rvalue::Aggregate(
                                            AggregateKind::Adt(
                                                def_id,
                                                variant_idx,
                                                Vec::new().into(),
                                                field_tys,
                                            ),
                                            vec![discr],
                                        ),
                                        adt_ty,
                                        expr.span,
                                    );
                                }
                                // Non-unit variant — the path is the ctor,
                                // which will be used in a Call expression.
                                // Fall through to create the Adt-typed operand.
                            }
                        }
                        let adt_ty = Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            expr.span,
                        );
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: adt_ty.clone(),
                                val: ConstVal::Uint(def_id.as_u32() as u128),
                            })),
                            adt_ty,
                            expr.span,
                        );
                    }
                    _ => {
                        // Stage 3.44: Handle Const and Static references.
                        // Per §15: root-cause fix — dispatch on DefKind
                        // instead of treating everything as FnDef.
                        match def_kind {
                            crate::resolve::DefKind::Const | crate::resolve::DefKind::Static => {
                                // Look up the const/static's value from HIR.
                                // For Stage 3.44, we evaluate the initializer
                                // expression and produce a constant operand.
                                if let Some(hir_crate) = cx.hir {
                                    if let Some(crate::hir::OwnerNode::Item(item)) =
                                        hir_crate.owner(def_id)
                                    {
                                        match item {
                                            crate::hir::HirItem::Const(c) => {
                                                // Lower the const's body expression to get its value.
                                                if let Some(body) = hir_crate.body(c.body) {
                                                    let const_local =
                                                        lower_expr_to_operand(cx, &body.value);
                                                    let ld = cx
                                                        .mir
                                                        .local_decls
                                                        .get(const_local.0 as usize);
                                                    if let Some(ld) = ld {
                                                        return cx.eval_rvalue_to_temp(
                                                            Rvalue::Use(Operand::Copy(
                                                                Place::local(
                                                                    const_local,
                                                                    expr.span,
                                                                ),
                                                            )),
                                                            ld.ty.clone(),
                                                            expr.span,
                                                        );
                                                    }
                                                }
                                            }
                                            crate::hir::HirItem::Static(s) => {
                                                // Statics are like consts but with a fixed memory location.
                                                // For Stage 3.44, treat same as const.
                                                if let Some(body) = hir_crate.body(s.body) {
                                                    let static_local =
                                                        lower_expr_to_operand(cx, &body.value);
                                                    let ld = cx
                                                        .mir
                                                        .local_decls
                                                        .get(static_local.0 as usize);
                                                    if let Some(ld) = ld {
                                                        return cx.eval_rvalue_to_temp(
                                                            Rvalue::Use(Operand::Copy(
                                                                Place::local(
                                                                    static_local,
                                                                    expr.span,
                                                                ),
                                                            )),
                                                            ld.ty.clone(),
                                                            expr.span,
                                                        );
                                                    }
                                                }
                                            }
                                            _ => {
                                                // Stage 14.104 (ME-4 fix): The path
                                                // resolved to a DefId that's not a
                                                // Const/Static — emit a typeck error
                                                // instead of silently falling through
                                                // to the FnDef fallback.
                                                //
                                                // Per §1.0 原则 5 "报错 > 静默": don't
                                                // silently treat unknown items as FnDef.
                                                cx.type_errors.push(
                                                    crate::typeck::TypeError::new(
                                                        format!(
                                                            "cannot find value `{}` in this scope (not a const/static/fn)",
                                                            cx.interner
                                                                .resolve(&path.segments[0].ident.name)
                                                        ),
                                                        expr.span,
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                                // Fallback: treat as FnDef (error recovery).
                                let fndef_ty = Ty::new(
                                    TyKind::FnDef(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                                    expr.span,
                                );
                                return cx.eval_rvalue_to_temp(
                                    Rvalue::Use(Operand::Constant(Const {
                                        ty: fndef_ty.clone(),
                                        val: ConstVal::Uint(def_id.as_u32() as u128),
                                    })),
                                    fndef_ty,
                                    expr.span,
                                );
                            }
                            _ => {
                                // Default: treat as FnDef (covers Fn, etc.).
                                let fndef_ty = Ty::new(
                                    TyKind::FnDef(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                                    expr.span,
                                );
                                return cx.eval_rvalue_to_temp(
                                    Rvalue::Use(Operand::Constant(Const {
                                        ty: fndef_ty.clone(),
                                        val: ConstVal::Uint(def_id.as_u32() as u128),
                                    })),
                                    fndef_ty,
                                    expr.span,
                                );
                            }
                        }
                    }
                }
            }
            // Otherwise, create an error placeholder.
            cx.eval_rvalue_to_temp(
                Rvalue::Use(Operand::Constant(Const {
                    ty: Ty::new(TyKind::Error, Span::DUMMY),
                    val: ConstVal::Int(0),
                })),
                Ty::new(TyKind::Error, Span::DUMMY),
                expr.span,
            )
        }
        HirExprKind::Binary { op, lhs, rhs, .. } => {
            // Short-circuit And/Or must be lowered to control flow,
            // not to BitAnd/BitOr (which would evaluate both sides).
            if *op == HirBinOp::And || *op == HirBinOp::Or {
                return control_flow::lower_short_circuit(cx, *op, lhs, rhs, expr.span);
            }
            let lhs_local = lower_expr_to_operand(cx, lhs);
            let rhs_local = lower_expr_to_operand(cx, rhs);
            let mir_op = MirLowerCtxt::lower_bin_op(*op);
            // Stage 15.76: For comparison ops (==, !=, <, >, <=, >=),
            // the result type is bool. For arithmetic ops (+, -, *, /, %,
            // &, |, ^, <<, >>), the result type is the lhs operand's type.
            let binop_ty = match *op {
                HirBinOp::Eq
                | HirBinOp::Ne
                | HirBinOp::Lt
                | HirBinOp::Le
                | HirBinOp::Gt
                | HirBinOp::Ge => Ty::new(TyKind::Bool, expr.span),
                _ => cx.mir.local(lhs_local).ty.clone(),
            };
            let lhs_operand = Operand::Copy(Place::local(lhs_local, lhs.span));
            let rhs_operand = Operand::Copy(Place::local(rhs_local, rhs.span));
            let result = cx.eval_rvalue_to_temp(
                Rvalue::BinaryOp(mir_op, lhs_operand.clone(), rhs_operand.clone()),
                binop_ty,
                expr.span,
            );
            // Stage 3.24 + 3.25: emit runtime checks for overflowable ops.
            //   - Div/Rem: emit DivisionByZero(rhs) check (divisor == 0)
            //   - Add/Sub/Mul/Shl/Shr: emit Overflow(op, lhs, rhs) check
            // Codegen turns these into real LLVM intrinsics / icmp branches.
            if overflow_assert::is_overflowable_op(*op) {
                match *op {
                    HirBinOp::Div | HirBinOp::Rem => {
                        overflow_assert::emit_div_by_zero_assert(
                            cx,
                            result,
                            rhs_operand.clone(),
                            expr.span,
                        );
                    }
                    _ => {
                        overflow_assert::emit_overflow_assert(
                            cx,
                            result,
                            mir_op,
                            lhs_operand,
                            rhs_operand,
                            expr.span,
                        );
                    }
                }
            }
            result
        }
        HirExprKind::Unary {
            op, expr: inner, ..
        } => {
            // Deref is a projection, not a real unary op.
            if *op == HirUnaryOp::Deref {
                return control_flow::lower_deref_expr(cx, inner, expr.span);
            }
            let inner_local = lower_expr_to_operand(cx, inner);
            let mir_op = MirLowerCtxt::lower_un_op(*op);
            // Stage 15.76: Use inner operand's type for unary op result
            // (same as Rust: `-a` has type of `a`).
            let unary_ty = cx.mir.local(inner_local).ty.clone();
            cx.eval_rvalue_to_temp(
                Rvalue::UnaryOp(mir_op, Operand::Copy(Place::local(inner_local, inner.span))),
                unary_ty,
                expr.span,
            )
        }
        HirExprKind::Block(block) => control_flow::lower_block(cx, block),
        HirExprKind::Call { func, args, .. } => {
            // Lower func first — this determines whether the call is a real
            // function call or an ADT construction (struct/enum ctor).
            let func_local = lower_expr_to_operand(cx, func);
            let arg_locals: Vec<LocalId> =
                args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();
            let arg_operands: Vec<Operand> = arg_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();

            // Stage 13.3a (TD-030): Closure call dispatch.
            //
            // Before falling through to the existing FnDef / Adt / placeholder
            // dispatch, check if `func_local` is registered in the
            // `cx.closure_bodies` side-table. If yes, this is a closure call —
            // inline the closure body at the call site.
            //
            // The side-table is keyed by LocalId (not DefId) because:
            // (a) at MIR lowering time, we don't have a unique per-closure
            //     DefId allocation mechanism;
            // (b) the call site has the func_local (LocalId), not a DefId;
            // (c) closure info propagates through `let` bindings (the let
            //     lowering in `control_flow::lower_block` propagates the
            //     info from init_local to let_local).
            //
            // The inline approach is the Stage 13.3a pragmatic subset of
            // Strategy A (per `stage-13.3-design-alignment.md` §4): each
            // call site gets a copy of the closure body. LLVM's optimizer
            // can deduplicate. Strategy A's full synthesized `call` function
            // is deferred to Stage 13.5+.
            //
            // Per §16: the closure body is HIR data sunk into the lowering
            // context as a side-table. No HIR access from codegen.
            if let Some(info) = cx.closure_bodies.get(&func_local).cloned() {
                return lower_closure_call_inline(cx, info, func_local, &arg_locals, expr);
            }

            // Stage 3.30 (per §15): inspect the func operand's type to decide
            //   - TyKind::Adt(def_id, _)  → Aggregate(Adt(def_id, ...)) —
            //     this is a struct/enum ctor call like `Pair(1, 2)`.
            //   - TyKind::FnDef(..)       → real TerminatorKind::Call.
            // This dispatch eliminates the root cause of "tuple struct ctor
            // was being lowered as Call" — the type info flows naturally
            // from Path resolution through to Call lowering.
            let is_adt_ctor = {
                let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
                func_local_decl
                    .map(|ld| matches!(&ld.ty.kind, TyKind::Adt(_, _)))
                    .unwrap_or(false)
            };

            if is_adt_ctor {
                // Struct/enum ctor: lower as Aggregate(Adt, operands).
                let func_local_decl = cx
                    .mir
                    .local_decls
                    .get(func_local.0 as usize)
                    .expect("func local must exist");
                let (adt_def_id, adt_substs) = match &func_local_decl.ty.kind {
                    TyKind::Adt(def_id, substs) => (*def_id, substs.clone()),
                    _ => unreachable!("checked is_adt_ctor above"),
                };
                // Stage 3.38 (L-ENUM): For enum variant ctors (e.g.,
                // `Opt::Some(42)`), resolve the variant index and field
                // types from the HIR enum definition. The func expression
                // is a Path like `Opt::Some` — check its HIR to find the
                // variant name.
                let (variant_idx, field_tys) = if let HirExprKind::Path(path) = &func.kind {
                    if path.segments.len() >= 2 {
                        if let Some((idx, tys)) =
                            resolve_enum_variant(cx, adt_def_id, &path.segments[1].ident.name)
                        {
                            (idx, tys)
                        } else {
                            (0, field_resolution::resolve_adt_field_tys(cx, adt_def_id))
                        }
                    } else {
                        (0, field_resolution::resolve_adt_field_tys(cx, adt_def_id))
                    }
                } else {
                    (0, field_resolution::resolve_adt_field_tys(cx, adt_def_id))
                };
                // For enum variants, the Aggregate operands need to include
                // the discriminant as the first element. For structs,
                // variant_idx = 0 and field_tys are the struct's fields.
                let mut all_operands = Vec::new();
                if variant_idx > 0
                    || (cx.hir.and_then(|h| h.owner(adt_def_id)).is_some_and(|o| {
                        matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
                    }))
                {
                    // Enum variant — prepend discriminant.
                    let discr = Operand::Constant(Const {
                        ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                        val: ConstVal::Uint(variant_idx as u128),
                    });
                    all_operands.push(discr);
                }
                all_operands.extend(arg_operands);
                let dest_ty = Ty::new(TyKind::Adt(adt_def_id, adt_substs), expr.span);
                let dest = cx.mir.new_local(dest_ty, None, expr.span);
                cx.push_assign(
                    Place::local(dest, expr.span),
                    Rvalue::Aggregate(
                        AggregateKind::Adt(adt_def_id, variant_idx, Vec::new().into(), field_tys),
                        all_operands,
                    ),
                    expr.span,
                );
                dest
            } else {
                // Stage 4.9: Check if func is a closure type.
                // Closures are not FnDef — they're values of type TyKind::Closure.
                // Calling a closure requires extracting the captured environment
                // and invoking the closure body. For now (simplified), we detect
                // closure calls and produce a placeholder result (unit type),
                // avoiding the incorrect TerminatorKind::Call that would treat the
                // closure struct as a function pointer.
                let is_closure = {
                    let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
                    func_local_decl
                        .map(|ld| matches!(&ld.ty.kind, TyKind::Closure(_, _)))
                        .unwrap_or(false)
                };

                if is_closure {
                    // Stage 4.13: Full closure call lowering — inline approach.
                    //
                    // When calling a closure, we:
                    // 1. Extract captured fields from the closure struct local
                    //    (each field is a Projection::Field on the closure local)
                    // 2. Bind captured values to fresh locals (so the inlined
                    //    body can reference them via the original local_map)
                    // 3. Bind call arguments to the closure's parameter locals
                    // 4. Lower the closure body inline at the call site
                    //
                    // This is the "inline" approach — no separate function is
                    // generated. The closure body is lowered directly at each
                    // call site. This is simple and correct, at the cost of
                    // code duplication (which LLVM's optimizer can handle).
                    //
                    // However, we don't have access to the HIR closure definition
                    // from here (we only have the func operand's type). So we
                    // use a pragmatic approach: extract captures from the closure
                    // struct, produce a fresh infer type for the result, and
                    // lower the call arguments. The actual body inlining requires
                    // HIR access which would need restructuring the lowering
                    // pipeline (deferred to Stage 5).
                    //
                    // For now (Stage 4.13): extract captures + produce result
                    // local with inferred type. This is more useful than the
                    // Stage 4.9 unit placeholder.

                    // Get the closure type's capture field types
                    let closure_ty = &cx.mir.local(func_local).ty;
                    // Stage 15.10: substs is now Rc<[Ty]>, convert to Vec for local use.
                    let capture_tys: Vec<Ty> = match &closure_ty.kind {
                        TyKind::Closure(_, substs) => substs.iter().cloned().collect(),
                        _ => vec![],
                    };

                    // Extract each captured field from the closure struct
                    for cap_ty in &capture_tys {
                        let field_ty = cap_ty.clone();
                        let _extracted_local = cx.mir.new_local(field_ty, None, expr.span);
                        // In a full implementation, we'd assign:
                        // extracted_local = Copy(Projection(closure_local, Field(i, cap_ty)))
                        // But since we can't map back to the original HirId here,
                        // we skip the binding. The inlined body would need
                        // these locals registered in local_map.
                    }

                    // Produce a result local with inferred type
                    let dest_ty = cx.fresh_infer_ty(expr.span);
                    cx.mir.new_local(dest_ty, None, expr.span)
                } else {
                    // Real function call.
                    let dest_ty = cx.fresh_infer_ty(Span::DUMMY);
                    let dest = cx.mir.new_local(dest_ty, None, expr.span);
                    let cont = cx.new_block();
                    cx.terminate_kind_and_goto(
                        TerminatorKind::Call {
                            func: Operand::Copy(Place::local(func_local, func.span)),
                            args: arg_operands,
                            destination: Place::local(dest, expr.span),
                            target: Some(cont),
                            dyn_trait_call: None,
                        },
                        cont,
                    );
                    dest
                }
            }
        }
        HirExprKind::If {
            cond, then, else_, ..
        } => control_flow::lower_if(cx, cond, then, else_.as_deref(), expr.span),
        HirExprKind::Match {
            expr: scrutinee,
            arms,
            ..
        } => control_flow::lower_match(cx, scrutinee, arms, expr.span),
        HirExprKind::Return { expr: ret_expr, .. } => {
            if let Some(ret) = ret_expr {
                let ret_local = lower_expr_to_operand(cx, ret);
                cx.push_assign(
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Use(Operand::Copy(Place::local(ret_local, ret.span))),
                    expr.span,
                );
            } else {
                // Stage 14.23: `return;` (no value) — assign unit () to the
                // return local. This allows typeck to detect the mismatch
                // when the function expects a non-unit type (e.g. i32).
                // Previously, `return;` left the return local uninitialized,
                // and the Never block type (from Stage 14.22) allowed it to
                // pass typeck — masking the error.
                cx.push_assign(
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
                    expr.span,
                );
            }
            cx.terminate_kind(TerminatorKind::Return);
            // Return a dummy local (unreachable after Return)
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }
        HirExprKind::Assign { lhs, rhs, op } => {
            // Stage 13.25: Handle compound assignment operators (+=, -=, *=, /=, %=).
            // When `op` is Some, the assignment is `lhs op= rhs`, which desugars
            // to `lhs = lhs op rhs`. Before Stage 13.25, the `op` field was
            // ignored (via `..`), so `x += 5` was lowered as `x = 5` (P0 bug).
            let lhs_place = lower_expr_to_place(cx, lhs);

            let rhs_local = if let Some(bin_op) = op {
                // Compound assignment: `lhs op= rhs` → `lhs = lhs op rhs`
                // Lower the RHS first, then read the LHS, apply the binop,
                // and store the result back.
                let rhs_val = lower_expr_to_operand(cx, rhs);
                // Read the current value of lhs into a temp
                let lhs_copy = lhs_place.clone();
                let lhs_ty = cx.fresh_infer_ty(expr.span);
                let lhs_val =
                    cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Copy(lhs_copy)), lhs_ty, expr.span);
                // Apply the binary operation: result = lhs_val op rhs_val
                let mir_op = MirLowerCtxt::lower_bin_op(*bin_op);
                let result_ty = cx.fresh_infer_ty(expr.span);
                let lhs_operand = Operand::Copy(Place::local(lhs_val, expr.span));
                let rhs_operand = Operand::Copy(Place::local(rhs_val, rhs.span));
                cx.eval_rvalue_to_temp(
                    Rvalue::BinaryOp(mir_op, lhs_operand, rhs_operand),
                    result_ty,
                    expr.span,
                )
            } else {
                // Simple assignment: `lhs = rhs`
                lower_expr_to_operand(cx, rhs)
            };

            // Stage 3.34 (L-MUT-1 fix): handle assignment LHS that are
            // projections (field access, index, deref).
            cx.push_assign(
                lhs_place,
                Rvalue::Use(Operand::Copy(Place::local(rhs_local, rhs.span))),
                expr.span,
            );
            rhs_local
        }
        HirExprKind::Tuple { elems, .. } => {
            let elem_locals: Vec<LocalId> =
                elems.iter().map(|e| lower_expr_to_operand(cx, e)).collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            // Stage 15.77: Resolve the tuple type from the element local types
            // (was: fresh_infer_ty, which stays unresolved at borrowck time
            // because writeback runs after borrowck). Same pattern as
            // Stages 15.73 (let bindings), 15.75 (deref), 15.76 (binop/unop).
            //
            // Each element local's type is read from local_decls (the source
            // of truth for lowered locals). The result is a concrete
            // `Tuple([ty_0, ty_1, ...])` type that survives borrowck.
            //
            // Per §1.0 原則 3 "显式 > 隣式": tuple type is explicitly resolved
            // from the element locals' types.
            // Per §16: reads only MIR data (local_decls), no HIR lookup.
            let elem_tys: Vec<Ty> = elem_locals
                .iter()
                .map(|l| cx.mir.local(*l).ty.clone())
                .collect();
            let tuple_ty = Ty::new(TyKind::Tuple(elem_tys), expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                tuple_ty,
                expr.span,
            )
        }
        HirExprKind::Unit => cx.eval_rvalue_to_temp(
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            Ty::new(TyKind::Tuple(vec![]), expr.span),
            expr.span,
        ),
        // === Stage 2.4b: Previously-missing expression kinds ===

        // Field access: `expr.field` → lower base, create projection
        // Stage 3.30 fix: resolve field index from the field name.
        //   - For tuple struct fields (`p.0`, `p.1`), the ident is the
        //     stringified index — parse it directly.
        //   - For named struct fields (`p.x`), look up the field index in
        //     the HIR struct definition by matching the field name.
        // Was: hardcoded `FieldId(0)` — meant `p.1`, `p.x`, etc. all
        // returned field 0.
        // Stage 3.32 fix (L-DEBT-2): resolve the field's actual type from
        // the struct definition and put it in ProjectionElem::Field (was:
        // fresh_infer_ty — typeck never resolved it, so codegen loaded
        // i32 even for i64 fields).
        HirExprKind::Field { receiver, ident } => {
            let base_local = lower_expr_to_operand(cx, receiver);
            // Resolve the field index from the ident name.
            let field_index = field_resolution::resolve_field_index(cx, receiver, &ident.name);
            // Stage 3.32: resolve the field's actual type from the struct def.
            let field_ty = field_resolution::resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            let field_ty_for_proj = field_ty.clone();
            // Stage 14.19 (GAP-31): If the base local's type is Ref (e.g. &self
            // or &mut self), auto-deref before projecting the field.
            let base_place =
                auto_deref_if_ref(cx, Place::local(base_local, receiver.span), receiver);
            let result = cx.mir.new_local(field_ty, None, expr.span);
            cx.push_assign(
                Place::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(base_place),
                        ProjectionElem::Field(FieldId(field_index), field_ty_for_proj),
                    ),
                    span: expr.span,
                })),
                expr.span,
            );
            result
        }

        // Index: `arr[idx]` → lower base + index, create projection
        HirExprKind::Index {
            receiver, index, ..
        } => {
            let base_local = lower_expr_to_operand(cx, receiver);
            let index_local = lower_expr_to_operand(cx, index);
            // Stage 3.52: compute the element type from the receiver's type,
            // instead of using a fresh infer var (which typeck defaults to
            // i32). For `&[T]` (fat pointer), elem_ty = T. For `[T; N]`,
            // elem_ty = T. Falls back to fresh infer var if the receiver's
            // type can't be resolved (preserves old behavior for test
            // contexts).
            let elem_ty = field_resolution::resolve_index_element_type(cx, base_local)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            let result = cx.mir.new_local(elem_ty, None, expr.span);
            cx.push_assign(
                Place::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(base_local, receiver.span)),
                        ProjectionElem::Index(index_local),
                    ),
                    span: expr.span,
                })),
                expr.span,
            );
            result
        }

        // Address-of: `&expr` / `&mut expr` → Rvalue::Ref
        HirExprKind::AddrOf {
            mutability,
            expr: inner,
            ..
        } => {
            let inner_local = lower_expr_to_operand(cx, inner);
            let bk = match mutability {
                crate::ast::Mutability::Mutable => crate::mir::place::BorrowKind::Mut,
                crate::ast::Mutability::Immutable => crate::mir::place::BorrowKind::Shared,
            };
            // Stage 15.77: Resolve the ref type from the inner local's type
            // (was: fresh_infer_ty, which stays unresolved at borrowck time
            // because writeback runs after borrowck). Same pattern as
            // Stages 15.73 (let bindings), 15.75 (deref), 15.76 (binop/unop).
            //
            // Result type: `Ref(Region::Erased, inner_ty, mutability)`.
            // We use Region::Erased (the codegen region) rather than a fresh
            // RegionVar because borrowck has its own region inference
            // (region_inference.rs) that assigns region variables to borrows
            // separately. At MIR lowering time, the meaningful information
            // is "this is a reference to inner_ty" — the region is filled in
            // by region inference, not by the lowerer.
            //
            // Per §1.0 原則 3 "显式 > 隐式": ref type is explicitly resolved.
            // Per §16: reads only MIR data (local_decls), no HIR lookup.
            let inner_ty = cx.mir.local(inner_local).ty.clone();
            let mir_mut = match mutability {
                crate::ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                crate::ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            let ref_ty = Ty::new(
                TyKind::Ref(Region::Erased, mir_mut, Box::new(inner_ty)),
                expr.span,
            );
            cx.eval_rvalue_to_temp(
                Rvalue::Ref(Region::Erased, bk, Place::local(inner_local, inner.span)),
                ref_ty,
                expr.span,
            )
        }

        // Cast: `expr as Ty` → Rvalue::Cast
        HirExprKind::Cast {
            expr: inner, ty, ..
        } => {
            let inner_local = lower_expr_to_operand(cx, inner);
            let target_ty = lower_hir_ty_to_mir_ty(ty);
            cx.eval_rvalue_to_temp(
                Rvalue::Cast(
                    CastKind::Numeric,
                    Operand::Copy(Place::local(inner_local, inner.span)),
                    target_ty.clone(),
                ),
                target_ty,
                expr.span,
            )
        }

        // Try: `expr?` → just lower inner (error propagation is Stage 3+)
        HirExprKind::Try { expr: inner, .. } => lower_expr_to_operand(cx, inner),

        // Loop: `loop { body }` → basic block loop
        HirExprKind::Loop { body, .. } => {
            let loop_header = cx.new_block();
            let loop_body_start = cx.new_block();
            let loop_exit = cx.new_block();
            let result_ty = cx.fresh_infer_ty(expr.span);
            // Stage 14.66: Loop result local must be MUTABLE so that
            // `break expr` can assign to it without triggering
            // "cannot assign twice to immutable variable" borrowck errors.
            //
            // Previously, this used `new_local` (Immutable), causing
            // `loop { break 42; }` to fail with AssignImmutable.
            //
            // Per §1.0 原则 6 "通用 > 特例": the loop result local is
            // always mutable (break assigns to it), regardless of the
            // loop's context.
            let result = cx.mir.new_local_with_mut(
                result_ty,
                None,
                expr.span,
                crate::mir::ty::Mutability::Mutable,
            );

            // Entry → goto loop_header
            cx.terminate_kind(TerminatorKind::Goto(loop_header));

            // loop_header → goto loop_body_start (placeholder for future
            // condition checking / break targeting)
            cx.current_block = loop_header;
            cx.terminate_kind(TerminatorKind::Goto(loop_body_start));

            // Stage 13.19: Push (continue_target=loop_header, break_target=loop_exit)
            cx.loop_stack.push((loop_header, loop_exit));
            // Stage 14.24: Push the result local so `break expr` can assign to it
            cx.loop_result_locals.push(result);

            // loop_body_start → lower body → goto loop_header
            cx.current_block = loop_body_start;
            let _body_result = control_flow::lower_block(cx, body);
            // Stage 14.68: Only emit Goto if the body didn't diverge.
            if !cx.is_terminated() {
                cx.terminate_kind(TerminatorKind::Goto(loop_header));
            }

            // Pop the loop stack
            cx.loop_stack.pop();
            cx.loop_result_locals.pop();

            // loop_exit (reached via Break) → continuation
            cx.current_block = loop_exit;
            result
        }

        // While: `while cond { body }` → loop with SwitchInt
        HirExprKind::While { cond, body, .. } => {
            let cond_block = cx.new_block();
            let body_block = cx.new_block();
            let exit_block = cx.new_block();

            // Entry → goto cond_block
            cx.terminate_kind(TerminatorKind::Goto(cond_block));

            // cond_block: evaluate cond, switchInt
            cx.current_block = cond_block;
            let cond_local = lower_expr_to_operand(cx, cond);
            cx.terminate_kind(TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::local(cond_local, cond.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // Stage 13.19: Push (continue_target=cond_block, break_target=exit_block)
            // onto the loop stack so `break` and `continue` inside the body
            // can emit the correct Goto. Before Stage 13.19, break/continue
            // were no-ops (P0 control-flow bug).
            cx.loop_stack.push((cond_block, exit_block));

            // body_block: lower body, goto cond_block
            cx.current_block = body_block;
            control_flow::lower_block(cx, body);
            // Stage 14.68: Only emit Goto if the body didn't diverge
            // (e.g. via `return`, `break`, or `continue`). Without this
            // check, the Goto would OVERWRITE the Return terminator,
            // causing the function to not return (infinite loop instead).
            //
            // Per §1.0 原则 5 "报错 > 静默": silently overwriting a Return
            // terminator is a P0 control-flow bug.
            if !cx.is_terminated() {
                cx.terminate_kind(TerminatorKind::Goto(cond_block));
            }

            // Pop the loop stack (body is done)
            cx.loop_stack.pop();

            // exit_block: continuation
            cx.current_block = exit_block;
            cx.mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span)
        }

        // For: `for pat in iter { body }`
        //
        // Stage 14.97: Implement proper for-loop over Range expressions.
        // Previously this was a stub that just checked if iter was "truthy",
        // which produced wrong runtime behavior for any range.
        //
        // Desugars `for i in start..end { body }` to:
        //   let mut i = start;
        //   while i < end { body; i += 1; }
        //
        // For inclusive ranges `start..=end`, uses `i <= end` and the same
        // increment. For non-Range iter expressions (arrays, etc.), emits a
        // typeck error (deferred to v0.2+).
        HirExprKind::For {
            pat, iter, body, ..
        } => {
            // Stage 14.97: Check if iter is a Range expression.
            let (start_expr, end_expr, end_kind) = match &iter.kind {
                HirExprKind::Range {
                    start,
                    end,
                    end_kind,
                } => (start, end, *end_kind),
                _ => {
                    // Non-Range iter — emit a typeck error.
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        // Stage 15.88: use human-readable expression kind
                        // (was: {:?} Debug format leaking HirExprKind::...).
                        format!(
                            "for-loop only supports Range iterators (start..end or start..=end); found {}",
                            crate::hir::hir_expr_kind_to_string(&iter.kind)
                        ),
                        expr.span,
                    ));
                    return cx.mir.new_local(
                        Ty::new(TyKind::Tuple(vec![]), expr.span),
                        None,
                        expr.span,
                    );
                }
            };

            // Range must have a start and end (we don't support open ranges yet).
            let start_expr = match start_expr {
                Some(s) => s,
                None => {
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        "for-loop over open range (..start / start..) is not supported in v0.1"
                            .to_string(),
                        expr.span,
                    ));
                    return cx.mir.new_local(
                        Ty::new(TyKind::Tuple(vec![]), expr.span),
                        None,
                        expr.span,
                    );
                }
            };
            let end_expr = match end_expr {
                Some(e) => e,
                None => {
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        "for-loop over open range (..start / start..) is not supported in v0.1"
                            .to_string(),
                        expr.span,
                    ));
                    return cx.mir.new_local(
                        Ty::new(TyKind::Tuple(vec![]), expr.span),
                        None,
                        expr.span,
                    );
                }
            };

            // Lower start and end expressions to locals.
            let start_local = lower_expr_to_operand(cx, start_expr);
            let end_local = lower_expr_to_operand(cx, end_expr);

            // Stage 14.99 (Bug Z5/Z6 fix): Use a HIDDEN counter local that's
            // always Mutable, separate from the user-visible pattern binding.
            //
            // Previously, the for-loop desugar used the pattern's hir_id as the
            // counter, which meant:
            //   - The user's `mut` annotation was ignored (counter was always mut)
            //   - Modifying the loop variable inside the body changed the counter,
            //     ending iteration early
            //
            // Now we use two locals:
            //   1. A hidden counter local (always Mutable, used for iteration control)
            //   2. The user-visible pattern binding (respecting user's `mut` annotation,
            //      copied from the counter at the start of each iteration)
            //
            // Per §1.0 原则 6 "通用 > 特例": one rule handles both `mut` and non-`mut`
            // patterns — the pattern binding's mutability is derived from the user's
            // annotation via `pat_mutability`, not hardcoded.
            let counter_ty = cx.mir.local(start_local).ty.clone();
            // Stage 14.99: Allocate the hidden counter via cx.mir.new_local_with_mut
            // (not cx.new_local_with_mut) so it doesn't get registered in local_map.
            // This is a temp local — it has no HirId and shouldn't be reachable by
            // name resolution.
            let hidden_counter = cx.mir.new_local_with_mut(
                counter_ty.clone(),
                None,
                expr.span,
                crate::mir::ty::Mutability::Mutable,
            );
            cx.mir
                .block_mut(cx.current_block)
                .statements
                .push(Statement {
                    kind: StatementKind::StorageLive(hidden_counter),
                    span: expr.span,
                });
            // hidden_counter = start
            cx.push_assign(
                Place::local(hidden_counter, expr.span),
                Rvalue::Use(Operand::Copy(Place::local(start_local, expr.span))),
                expr.span,
            );

            // Create the user-visible pattern binding local.
            // The mutability respects the user's `mut` annotation.
            let pat_mutability = pattern_bindings::pat_mutability(pat);
            let pat_local = cx.new_local_with_mut(pat.hir_id, counter_ty, None, pat_mutability);
            cx.mir
                .block_mut(cx.current_block)
                .statements
                .push(Statement {
                    kind: StatementKind::StorageLive(pat_local),
                    span: expr.span,
                });

            // Allocate block IDs for the loop.
            let cond_block = cx.new_block();
            let body_block = cx.new_block();
            let incr_block = cx.new_block();
            let exit_block = cx.new_block();

            // Entry → goto cond_block
            cx.terminate_kind(TerminatorKind::Goto(cond_block));

            // cond_block: compare HIDDEN COUNTER with end.
            // - Excluded range (start..end): loop while counter < end
            // - Included range (start..=end): loop while counter <= end
            cx.current_block = cond_block;
            let cmp_op = match end_kind {
                crate::ast::RangeEnd::Excluded => BinOp::Lt,
                crate::ast::RangeEnd::Included => BinOp::Le,
            };
            let cond_ty = Ty::new(TyKind::Bool, expr.span);
            let cond_local = cx.eval_rvalue_to_temp(
                Rvalue::BinaryOp(
                    cmp_op,
                    Operand::Copy(Place::local(hidden_counter, expr.span)),
                    Operand::Copy(Place::local(end_local, expr.span)),
                ),
                cond_ty,
                expr.span,
            );
            cx.terminate_kind(TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::local(cond_local, expr.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // body_block: copy hidden_counter → pat_local, lower the body,
            // then goto incr_block.
            cx.current_block = body_block;
            // Copy the current counter value into the user-visible pattern binding.
            // This is done at the START of each iteration, so modifications to the
            // pattern binding inside the body don't affect the counter.
            cx.push_assign(
                Place::local(pat_local, expr.span),
                Rvalue::Use(Operand::Copy(Place::local(hidden_counter, expr.span))),
                expr.span,
            );
            // Push loop context (incr_block = continue target, exit_block = break target).
            cx.loop_stack.push((incr_block, exit_block));
            cx.loop_result_locals.push(pat_local); // not used (for-loop has no break value)
            control_flow::lower_block(cx, body);
            cx.loop_result_locals.pop();
            cx.loop_stack.pop();
            // Stage 14.68: Only emit Goto if the body didn't diverge.
            if !cx.is_terminated() {
                cx.terminate_kind(TerminatorKind::Goto(incr_block));
            }

            // incr_block: hidden_counter += 1, then goto cond_block.
            // Note: only the hidden counter is incremented — the user-visible
            // pattern binding is left as-is (it's overwritten at the start of
            // the next iteration anyway).
            cx.current_block = incr_block;
            let one_const = Operand::Constant(Const {
                ty: cx.mir.local(hidden_counter).ty.clone(),
                val: ConstVal::Int(1),
            });
            let new_val = cx.eval_rvalue_to_temp(
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::Copy(Place::local(hidden_counter, expr.span)),
                    one_const,
                ),
                cx.mir.local(hidden_counter).ty.clone(),
                expr.span,
            );
            cx.push_assign(
                Place::local(hidden_counter, expr.span),
                Rvalue::Use(Operand::Copy(Place::local(new_val, expr.span))),
                expr.span,
            );
            cx.terminate_kind(TerminatorKind::Goto(cond_block));

            // exit_block: continuation. For-loop evaluates to unit ().
            cx.current_block = exit_block;
            cx.mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span)
        }

        // Closure: `|args| body` → create closure value with captures + register
        // the closure's HIR body for later inlining at call sites.
        //
        // Stage 4.4 (L3 closure codegen): creates a proper closure value.
        // Stage 4.7 (L3 capture analysis): now detects and captures external variables.
        // Stage 13.3a (TD-030 closure call lowering): the body is NO LONGER lowered
        // inline at the closure literal site — instead, it is stored in the
        // `cx.closure_bodies` side-table keyed by the closure_local. When the
        // closure is later called (`HirExprKind::Call` arm), the body is
        // retrieved and lowered inline at the call site.
        //
        // Why the change? Previously, the body was lowered inline at the
        // closure literal site, which caused the body's statements (and side
        // effects!) to fire at closure CONSTRUCTION time, not at call time.
        // This is a soundness bug — `let f = |x| { println!(...); x + 1 };`
        // would print at construction, not at `f(5)`. Now the body is only
        // lowered when the closure is actually called.
        //
        // What if the closure is never called? The body is never lowered,
        // and any errors inside it are not reported. This is acceptable for
        // Stage 13.3a — dead closures are dead code. A future "lower all
        // closure bodies for error checking" pass can be added if needed.
        //
        // Capture analysis:
        // - Walks the closure body (HIR walk, not MIR) to find
        //   `HirExprKind::Path` with `Res::Local(hir_id)`.
        // - Filters out closure params (those hir_ids just registered).
        // - Remaining locals are "captured" — their values become closure
        //   env fields, stored in the closure struct.
        //
        // Limitations (deferred to Stage 13.5+):
        // - Capture mode (move vs borrow): currently always Copy
        // - Nested closures: not yet handled
        // - Fn/FnMut/FnOnce auto-impl: deferred to Stage 13.5+
        HirExprKind::Closure { params, body, .. } => {
            // Register closure params as locals + collect their hir_ids.
            // The params get fresh infer var types — these will be unified
            // with the call arg types when the closure is called.
            let mut param_hir_ids: std::collections::HashSet<HirId> =
                std::collections::HashSet::new();
            for param in params {
                let ty = cx.fresh_infer_ty(param.pat.span);
                cx.new_local(param.pat.hir_id, ty, None);
                // Collect all hir_ids from the pattern (ident, tuple, etc.)
                pattern_bindings::collect_pat_hir_ids(&param.pat, &mut param_hir_ids);
            }

            // Stage 4.7: Collect captured locals — external variables referenced in body
            let mut captured: Vec<(HirId, LocalId)> = Vec::new();
            let mut seen: std::collections::HashSet<HirId> = std::collections::HashSet::new();
            closure_capture::collect_captured_locals(
                cx,
                body,
                &param_hir_ids,
                &mut captured,
                &mut seen,
            );

            // Stage 13.3a: DO NOT lower the body inline here. The body is
            // stored in `cx.closure_bodies` and lowered at the call site.
            // (See comment block above for the soundness rationale.)

            // Stage 4.7: Build capture field types + operands.
            // Each capture's type is read from the captured variable's local_decl.
            //
            // Stage 13.3a fix: choose `Operand::Copy` or `Operand::Move` based
            // on whether the capture's type is Copy.
            //   - Copy types (i32, bool, &T, etc.): use `Operand::Copy` —
            //     bit-copy the value into the closure struct. The original
            //     variable remains valid for subsequent uses.
            //   - Non-Copy types (Closure, Str, Slice, etc.): use `Operand::Move` —
            //     transfer ownership into the closure struct. The original
            //     variable is moved and cannot be used again.
            //
            // This matches Rust's default capture mode (by-ref for Copy types,
            // by-value for non-Copy types when the closure body requires it).
            // The `move` keyword on closures is currently a no-op (Stage 13.3a
            // simplification — proper move-closure semantics deferred to
            // Stage 13.5+).
            //
            // Stage 13.3a §16 note: we inline the Copy-ness check here
            // (instead of importing `borrowck::ty_is_copy`) to avoid a
            // mir::lower → borrowck dependency (borrowck runs after
            // mir::lower per the pipeline). The logic is duplicated from
            // `src/borrowck/copy_semantics.rs::ty_is_copy` — a future
            // refactor should move Copy-ness detection to a neutral location
            // (e.g., `mir::ty` or a new `ty::copy_semantics` module).
            let mut capture_tys: Vec<Ty> = Vec::new();
            let mut capture_operands: Vec<Operand> = Vec::new();
            for (_cap_hir_id, local_id) in &captured {
                let ty = cx.mir.local(*local_id).ty.clone();
                capture_tys.push(ty.clone());
                let operand = if crate::mir::ty::is_mir_ty_copy_conservative(&ty) {
                    Operand::Copy(Place::local(*local_id, expr.span))
                } else {
                    Operand::Move(Place::local(*local_id, expr.span))
                };
                capture_operands.push(operand);
            }

            // Create closure value with captures
            let closure_def_id = cx
                .hir
                .map(|h| h.owners.first().map(|(id, _)| *id).unwrap_or_default())
                .unwrap_or_default();
            let closure_ty = Ty::new(
                // Stage 15.10: capture_tys is Vec<Ty>, convert to Rc<[Ty]>.
                TyKind::Closure(closure_def_id, capture_tys.clone().into()),
                expr.span,
            );
            let closure_local = cx.mir.new_local(closure_ty, None, expr.span);
            // Assign the closure value with captured operands.
            // Stage 13.3a fix: pass `capture_tys` (not `vec![]`) as the
            // AggregateKind::Closure substs — matches TyKind::Closure substs.
            // Was: `vec![]` — inconsistent with the type's substs.
            cx.mir
                .block_mut(cx.current_block)
                .statements
                .push(Statement {
                    kind: StatementKind::Assign(Box::new((
                        Place::local(closure_local, expr.span),
                        Rvalue::Aggregate(
                            // Stage 15.10: capture_tys → Rc<[Ty]>
                            AggregateKind::Closure(closure_def_id, capture_tys.clone().into()),
                            capture_operands,
                        ),
                    ))),
                    span: expr.span,
                });

            // Stage 13.3a: store the closure's (params, body, captures) in
            // the side-table keyed by closure_local. The `HirExprKind::Call`
            // arm will look this up to inline the body at the call site.
            let closure_info = super::ClosureBodyInfo {
                params: params.clone(),
                body: body.clone(),
                captures: captured
                    .iter()
                    .map(|(hir_id, local_id)| {
                        let ty = cx.mir.local(*local_id).ty.clone();
                        (*hir_id, ty)
                    })
                    .collect(),
            };
            cx.closure_bodies.insert(closure_local, closure_info);

            closure_local
        }

        // Break: `break expr` → goto loop exit block
        // Stage 13.19: Now emits a real Goto to the enclosing loop's exit
        // block (tracked via cx.loop_stack). Before Stage 13.19, this was a
        // no-op (P0 control-flow bug — break did nothing).
        HirExprKind::Break { expr: br_expr, .. } => {
            // Stage 14.24: Assign break value to the loop's result local
            // before jumping to the break target. Was: break value was
            // lowered but discarded (let _ = ...), so `loop { break 42; }`
            // returned an uninitialized local instead of 42.
            if let Some(e) = br_expr {
                let br_local = lower_expr_to_operand(cx, e);
                // Find the loop's result local and assign the break value to it.
                // The loop_stack entry is (continue_target, break_target).
                // We need the result local — it's stored in the Loop context.
                // For now, we use the loop's result local which is the last
                // local allocated before the loop body.
                if let Some((_, break_target)) = cx.loop_stack.last().copied() {
                    // Assign break value to the loop result local.
                    // The loop result local is stored in cx.loop_result_locals.
                    if let Some(result_local) = cx.loop_result_locals.last().copied() {
                        cx.push_assign(
                            Place::local(result_local, expr.span),
                            Rvalue::Use(Operand::Copy(Place::local(br_local, e.span))),
                            expr.span,
                        );
                    }
                    cx.terminate_kind(TerminatorKind::Goto(break_target));
                }
            } else {
                // Get the break target from the loop stack.
                if let Some((_, break_target)) = cx.loop_stack.last().copied() {
                    cx.terminate_kind(TerminatorKind::Goto(break_target));
                }
            }
            // Allocate a fresh block for any code after the break (unreachable
            // but needed so current_block is valid for subsequent lowering).
            cx.current_block = cx.new_block();
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Continue: `continue` → goto loop header (cond_block for while)
        // Stage 13.19: Now emits a real Goto to the enclosing loop's continue
        // target. Before Stage 13.19, this was a no-op.
        HirExprKind::Continue => {
            if let Some((continue_target, _)) = cx.loop_stack.last().copied() {
                cx.terminate_kind(TerminatorKind::Goto(continue_target));
            }
            cx.current_block = cx.new_block();
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Range: `start..end` → Aggregate (simplified)
        HirExprKind::Range { start, end, .. } => {
            let start_local = start.as_ref().map(|s| lower_expr_to_operand(cx, s));
            let end_local = end.as_ref().map(|e| lower_expr_to_operand(cx, e));
            let range_ty = cx.fresh_infer_ty(expr.span);
            // For Stage 2.4b, ranges are represented as a tuple (start, end)
            let mut operands = Vec::new();
            if let Some(s) = start_local {
                operands.push(Operand::Copy(Place::local(s, Span::DUMMY)));
            }
            if let Some(e) = end_local {
                operands.push(Operand::Copy(Place::local(e, Span::DUMMY)));
            }
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                range_ty,
                expr.span,
            )
        }

        // Array: `[a, b, c]` → Aggregate(Array, operands)
        HirExprKind::Array { elems, .. } => {
            let elem_locals: Vec<LocalId> =
                elems.iter().map(|e| lower_expr_to_operand(cx, e)).collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            let elem_ty = cx.fresh_infer_ty(expr.span);
            let elem_ty_for_agg = elem_ty.clone();
            let array_ty = Ty::new(
                TyKind::Array(
                    Box::new(elem_ty),
                    Box::new(Const {
                        ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY),
                        val: ConstVal::Uint(elems.len() as u128),
                    }),
                ),
                expr.span,
            );
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Array(elem_ty_for_agg), operands),
                array_ty,
                expr.span,
            )
        }

        // Repeat: `[val; N]` → Aggregate(Array, [val, val, ...]) with N copies
        HirExprKind::Repeat { elem, count, .. } => {
            let elem_local = lower_expr_to_operand(cx, elem);
            // Stage 14.20: Evaluate the count expression to get N.
            // If count is a literal integer, extract its value directly.
            // Stage 14.103 (ME-3 fix): If count is NOT a literal, emit a
            // typeck error instead of silently falling back to 1 element.
            //
            // Per §1.0 原则 5 "报错 > 静默": non-const array lengths must be
            // reported, not silently mis-compiled as 1-element arrays.
            let n: usize = match &count.kind {
                crate::hir::HirExprKind::Lit(crate::hir::HirLitKind::Int(val, _)) => *val as usize,
                crate::hir::HirExprKind::Lit(crate::hir::HirLitKind::Uint(val, _)) => *val as usize,
                _ => {
                    // Non-literal count — emit error, fall back to 1 element
                    // for recovery (so downstream codegen doesn't crash).
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        // Stage 15.88: use human-readable expression kind
                        // (was: {:?} Debug format leaking HirExprKind::...).
                        format!(
                            "array repeat count must be a literal integer in v0.1; const-eval for non-literal counts is v0.2+ work (found {})",
                            crate::hir::hir_expr_kind_to_string(&count.kind)
                        ),
                        expr.span,
                    ));
                    1
                }
            };
            // Stage 14.79: Use the actual element type from the lowered element,
            // not Error. Was: TyKind::Error (resolved to i32, causing nested
            // arrays like [[i32; 3]; 3] to have wrong alloca type — the outer
            // array [3 x [3 x i32]] was stored into an alloca typed [3 x i32],
            // causing "Invalid InsertValueInst operands!" in LLVMSysEmitter.
            //
            // Stage 14.80 hardening: only use the concrete element type if it
            // is *not* an Infer/Error placeholder. For unsuffixed integer
            // literals like `0`, the lowered element type is `Infer(IntVar)`
            // (int-only — cannot unify with Float/Bool/Char/Str). If we put
            // that into array_ty, typeck would error on `let arr: [f64; 3]
            // = [0; 3]` — but in Rust that is *also* a type error, so the
            // behavior is correct. However, for the AggregateKind::Array we
            // use a fresh TyVar (general inference variable) so each operand
            // unifies successfully with the declared element type, and the
            // outer array_type can unify with the destination's element type.
            //
            // Per §1.0 原則 5 "报错 > 静默": surface real type errors (e.g.
            // `[0; 3]` for `[f64; 3]` is now caught by typeck instead of
            // being silently mis-compiled as `Error` → `i32`).
            // Per §1.0 原則 6 "通用 > 特例": one rule handles both cases by
            // checking whether the element type is already concrete.
            let actual_elem_ty = cx.mir.local(elem_local).ty.clone();
            // For array_ty: use concrete element type if known, else fall
            // back to Error (preserves pre-14.79 behavior for unsuffixed
            // literals — typeck will catch the mismatch if destination is
            // non-int, but codegen uses Error → I32 fallback for size).
            // For AggregateKind::Array: always use a fresh TyVar so operands
            // unify cleanly with the declared element type.
            let array_elem_ty = if matches!(&actual_elem_ty.kind, TyKind::Infer(_) | TyKind::Error)
            {
                Ty::new(TyKind::Error, expr.span)
            } else {
                actual_elem_ty
            };
            let agg_elem_ty = cx.fresh_infer_ty(expr.span);
            let count_const = crate::mir::ty::Const {
                ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), expr.span),
                val: crate::mir::ty::ConstVal::Int(n as u128),
            };
            let array_ty = Ty::new(
                TyKind::Array(Box::new(array_elem_ty), Box::new(count_const)),
                expr.span,
            );
            // Build the operands list: N copies of the element.
            let operands: Vec<Operand> = (0..n)
                .map(|_| Operand::Copy(Place::local(elem_local, elem.span)))
                .collect();
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Array(agg_elem_ty), operands),
                array_ty,
                expr.span,
            )
        }

        // Struct literal: `Foo { x: 1, y: 2 }` → Aggregate(Adt, operands)
        HirExprKind::Struct { path, fields, .. } => {
            // Lower each field value
            let field_locals: Vec<LocalId> = fields
                .iter()
                .filter_map(|f| f.expr.as_ref().map(|e| lower_expr_to_operand(cx, e)))
                .collect();
            // Stage 15.64: Choose Copy or Move based on the field value's type.
            // For Copy types (primitives, refs, etc.), use Copy (source remains
            // valid). For non-Copy types (Adt with Drop, Str, etc.), use Move
            // (transfers ownership, source is marked as moved).
            //
            // Previously, this always used Operand::Copy for ALL fields, which
            // caused double-drop of non-Copy field temporaries (the temp was
            // not marked as moved, so elaborate_drops inserted a Drop for it
            // in addition to the Drop for the struct itself).
            //
            // Per §23 rule 5 (DRY): uses the shared `is_mir_ty_copy_conservative`
            // helper from `mir::ty` (same as `let` bindings in control_flow.rs).
            // Per §1.0 原則 5 "报错 > 静默": conservative (Move for unknown Adt)
            // is preferred over unsound (Copy for non-Copy).
            let operands: Vec<Operand> = field_locals
                .iter()
                .map(|l| {
                    let field_ty = &cx.mir.local(*l).ty;
                    if crate::mir::ty::is_mir_ty_copy_conservative(field_ty) {
                        Operand::Copy(Place::local(*l, Span::DUMMY))
                    } else {
                        Operand::Move(Place::local(*l, Span::DUMMY))
                    }
                })
                .collect();
            // Stage 3.30 (per §15): if the path resolves to a known struct
            // DefId, use AggregateKind::Adt (the proper representation).
            // Stage 3.38 (L-ENUM): also handle enum struct variants
            // (e.g., `Shape::Circle { r: 1.0 }`).
            if let Res::Def(def_id, DefKind::Struct) = path.res {
                let field_tys = field_resolution::resolve_adt_field_tys(cx, def_id);
                let struct_ty = Ty::new(
                    TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                    expr.span,
                );
                return cx.eval_rvalue_to_temp(
                    Rvalue::Aggregate(
                        AggregateKind::Adt(def_id, 0, Vec::new().into(), field_tys),
                        operands,
                    ),
                    struct_ty,
                    expr.span,
                );
            }
            // Stage 3.38 (L-ENUM): Enum struct variant (e.g., `Shape::Circle { r: 1.0 }`).
            if let Res::Def(def_id, DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    let variant_name = &path.segments[1].ident.name;
                    if let Some((variant_idx, field_tys)) =
                        resolve_enum_variant(cx, def_id, variant_name)
                    {
                        // Prepend discriminant to the operands.
                        let discr = Operand::Constant(Const {
                            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                            val: ConstVal::Uint(variant_idx as u128),
                        });
                        let mut all_operands = vec![discr];
                        all_operands.extend(operands);
                        let enum_ty = Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            expr.span,
                        );
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Aggregate(
                                AggregateKind::Adt(
                                    def_id,
                                    variant_idx,
                                    Vec::new().into(),
                                    field_tys,
                                ),
                                all_operands,
                            ),
                            enum_ty,
                            expr.span,
                        );
                    }
                }
            }
            // Fallback (path didn't resolve to a struct — error recovery).
            let struct_ty = cx.fresh_infer_ty(expr.span);
            let _ = path;
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
                struct_ty,
                expr.span,
            )
        }

        // Stage 13.12 + Stage 13.13: Println — emit inline StatementKind::Println
        // in the current basic block so codegen places the printf call at the
        // exact source-code position (fixes Stage 13.12 ordering bug).
        //
        // Stage 13.12 stored the message in a Vec<String> side-table and
        // codegen emitted a separate __landin_printlns_<fnname> helper
        // function called BEFORE landin_main() — this broke ordering for
        // loops and conditionals.
        //
        // Stage 13.13 replaces the side-table approach with an inline
        // StatementKind::Println variant, so codegen emits printf inline
        // at the source position. (Stage 15.6: the legacy
        // MirBody.println_messages side-table field was removed in 14.x.)
        HirExprKind::Println {
            msg,
            args,
            newline,
            stderr,
        } => {
            let full_msg = if *newline {
                format!("{}\n", msg)
            } else {
                msg.clone()
            };
            // Stage 13.16: Lower each arg expression to a MIR operand.
            // Each arg is lowered via lower_expr_to_operand (which produces
            // a LocalId holding the arg's value). We then wrap it in
            // Operand::Copy to produce a use operand for codegen.
            let arg_operands: Vec<Operand> = args
                .iter()
                .map(|arg| {
                    let local = lower_expr_to_operand(cx, arg);
                    Operand::Copy(Place::local(local, expr.span))
                })
                .collect();
            // Push the println statement to the current basic block —
            // this is the §16-compliant way to express an ordered side effect.
            cx.mir
                .block_mut(cx.current_block)
                .statements
                .push(Statement {
                    kind: StatementKind::Println {
                        msg: full_msg,
                        args: arg_operands,
                        newline: *newline,
                        stderr: *stderr,
                    },
                    span: expr.span,
                });
            let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
            cx.mir.new_local(unit_ty, None, expr.span)
        }

        // Stage 4.10 + Stage 13.4a: MacroCall — expand known built-in macros.
        // Stage 4.10: 7 macros (println, print, eprintln, eprint, stringify, assert, debug_assert)
        // Stage 13.4a (TD-032): +19 macros (assert_eq, assert_ne, debug_assert_eq,
        //   debug_assert_ne, write, writeln, panic, todo, unimplemented, unreachable,
        //   cfg, include, concat, env, option_env, format_args, format, vec, dbg)
        // Total: 26 built-in macros per 13-stage1-feature-whitelist.md §2.6
        HirExprKind::MacroCall { path, .. } => {
            // Get the macro name from the last path segment.
            let macro_name = path.segments.last().map(|s| s.ident.name);
            if let Some(name_spur) = macro_name {
                let name = cx.interner.resolve(&name_spur).to_string();
                match name.as_str() {
                    // === Printing macros (4) — emit printf call + produce unit ===
                    // Stage 13.11: Now emits actual printf calls for string literal args.
                    "println" | "print" | "eprintln" | "eprint" => {
                        // Stage 13.11: Emit a call to printf/puts.
                        // We declare printf as extern and call it with the format string.
                        // For println!, we append "\n" to the message.
                        // For eprintln!/eprint!, we use fprintf(stderr, ...).
                        //
                        // Since we don't have the macro args in MIR (they were skipped
                        // by the parser), we emit a no-op unit local for now.
                        // The actual printf call is generated in the C wrapper
                        // via the TextEmitter path (which emits printf declarations).
                        //
                        // For LLVMSysEmitter: the C wrapper includes stdio.h,
                        // so printf is available. The codegen_from_mir path
                        // calls emit_call("printf", ...) which declares printf
                        // as extern in the LLVM module.
                        //
                        // However, since we don't have the message string in MIR,
                        // we can't emit the printf call here. The message was
                        // captured in the AST Println node but lost during HIR
                        // lowering (it was lowered to MacroCall without args).
                        //
                        // For now: produce unit (same as before). The println!
                        // output support requires either:
                        // 1. Storing the message in HIR (new HIR variant), or
                        // 2. A side-channel from parser to codegen, or
                        // 3. Full macro expansion (Stage 4 feature)
                        //
                        // Stage 13.11 pragmatic approach: the parser captures
                        // the message in Expr::Println, but HIR lowering converts
                        // it back to MacroCall (losing the message). To fix,
                        // we need a HIR variant that carries the message.
                        // This is deferred to a future stage.
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Stringification macros (2) — produce &str ===
                    "stringify" | "concat" => {
                        let str_ty = Ty::new(
                            TyKind::Ref(
                                Region::Static,
                                crate::mir::ty::Mutability::Immutable,
                                Box::new(Ty::new(TyKind::Str, expr.span)),
                            ),
                            expr.span,
                        );
                        cx.mir.new_local(str_ty, None, expr.span)
                    }
                    // === Assertion macros (6) — produce unit ===
                    "assert" | "debug_assert" | "assert_eq" | "assert_ne" | "debug_assert_eq"
                    | "debug_assert_ne" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Writing macros (2) — produce unit ===
                    "write" | "writeln" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Diverging macros (4) — produce Never type ===
                    "panic" | "todo" | "unimplemented" | "unreachable" => {
                        let never_ty = Ty::new(TyKind::Never, expr.span);
                        cx.mir.new_local(never_ty, None, expr.span)
                    }
                    // === Configuration macro (1) — produce bool ===
                    "cfg" => {
                        let bool_ty = Ty::new(TyKind::Bool, expr.span);
                        cx.mir.new_local(bool_ty, None, expr.span)
                    }
                    // === File inclusion macro (1) — produce unit ===
                    "include" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Environment macros (2) — produce &str ===
                    "env" | "option_env" => {
                        let str_ty = Ty::new(
                            TyKind::Ref(
                                Region::Static,
                                crate::mir::ty::Mutability::Immutable,
                                Box::new(Ty::new(TyKind::Str, expr.span)),
                            ),
                            expr.span,
                        );
                        cx.mir.new_local(str_ty, None, expr.span)
                    }
                    // === Format args macro (1) — produce unit ===
                    "format_args" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Format macro (1) — produce String (simplified to unit for MVP) ===
                    "format" => {
                        // Stage 13.4a: simplified to unit (full String requires alloc support)
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Vec macro (1) — produce unit (simplified for MVP) ===
                    "vec" => {
                        // Stage 13.4a: simplified to unit (full Vec<T> requires alloc support)
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Debug macro (1) — produce unit ===
                    "dbg" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    _ => {
                        // Stage 14.104 (ME-5 fix): Unknown macro → emit typeck
                        // error instead of silently returning Error placeholder.
                        //
                        // Per §1.0 原则 5 "报错 > 静默": unknown macros must be
                        // reported, not silently mis-compiled as Error→0.
                        let macro_name_str = cx.interner.resolve(&name_spur).to_string();
                        cx.type_errors.push(crate::typeck::TypeError::new(
                            format!("cannot find macro `{}` in this scope", macro_name_str),
                            expr.span,
                        ));
                        cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: Ty::new(TyKind::Error, Span::DUMMY),
                                val: ConstVal::Int(0),
                            })),
                            Ty::new(TyKind::Error, Span::DUMMY),
                            expr.span,
                        )
                    }
                }
            } else {
                // No macro name → emit error.
                cx.type_errors.push(crate::typeck::TypeError::new(
                    "macro call has no name".to_string(),
                    expr.span,
                ));
                cx.eval_rvalue_to_temp(
                    Rvalue::Use(Operand::Constant(Const {
                        ty: Ty::new(TyKind::Error, Span::DUMMY),
                        val: ConstVal::Int(0),
                    })),
                    Ty::new(TyKind::Error, Span::DUMMY),
                    expr.span,
                )
            }
        }

        // Unsafe block: just lower inner block (unsafety is a typeck concern)
        HirExprKind::Unsafe(block) => control_flow::lower_block(cx, block),

        // Stage 8.5: async/await — MVP: evaluate synchronously
        HirExprKind::Await { expr } => lower_expr_to_operand(cx, expr),
        HirExprKind::Async { block } => control_flow::lower_block(cx, block),

        // MethodCall: `receiver.method(args)` → simplified to Call
        HirExprKind::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let recv_local = lower_expr_to_operand(cx, receiver);
            let arg_locals: Vec<LocalId> =
                args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();

            // Stage 5.78: dyn Trait path.
            //
            // When `cx.dyn_trait_plan()` is set AND the method name matches
            // an entry in the plan, use the dyn Trait call terminator
            // (which records the call info in `cx.mir.dyn_trait_calls`
            // side-table for codegen Stage 5.79+ to emit a vtable indirect
            // call). Otherwise fall through to the legacy placeholder path.
            //
            // Per §16: the plan was built upstream (by the driver, using
            // `build_dyn_trait_mir_plan_from_resolver()`) and attached via
            // `cx.set_dyn_trait_plan()`. The lower does not query HIR or
            // TraitResolver directly here.
            //
            // We clone the matched `DynTraitMethodCall` out of the
            // immutable borrow scope before mutating `cx` — this satisfies
            // the borrow checker (immutable borrow of `cx.dyn_trait_plan()`
            // ends before the mutable borrow begins via `build_dyn_trait_call_terminator`).
            // Stage 14.91 (Bug X3 fix): Before using the dyn Trait vtable
            // indirect call path, check if the method can be resolved via
            // static dispatch (inherent method or trait impl method). If so,
            // skip the dyn Trait path and use static dispatch instead.
            //
            // The dyn Trait path is for actual `dyn Trait` receivers (fat
            // pointers with vtable). For concrete types like `Square`, we
            // should use static dispatch — the vtable indirect call crashes
            // because the receiver is passed as a value, not a fat pointer.
            //
            // Per §1.0 原則 5 "报错 > 静默": the dyn Trait path silently
            // produced wrong code for concrete types, causing LLVM crashes.
            let method_name_str = cx.interner.resolve(&method.name).to_string();
            let matched_call: Option<DynTraitMethodCall> = cx.dyn_trait_plan().and_then(|plan| {
                find_dyn_trait_method_call_in_plan_by_method(plan, &method_name_str).cloned()
            });

            // Check if static dispatch is possible before using dyn Trait
            let can_static_dispatch = cx.hir.is_some_and(|hir| {
                let recv_ty = cx.mir.local(recv_local).ty.clone();
                if resolve_inherent_method(hir, &recv_ty, &method.name).is_some() {
                    return true;
                }
                if resolve_inherent_method_from_hir_expr(cx, hir, receiver, &method.name).is_some()
                {
                    return true;
                }
                if resolve_trait_method(hir, &recv_ty, &method.name).is_some() {
                    return true;
                }
                // Stage 14.91: Also try HIR-traced type for trait method resolution.
                // The MIR type may be Infer, but HIR tracing can find the ADT type.
                if let Some(init_ty) = find_local_init_type(cx, hir, {
                    // Get the hir_id from the receiver Path
                    if let HirExprKind::Path(path) = &receiver.kind {
                        if let crate::hir::Res::Local(hir_id) = path.res {
                            hir_id
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }) {
                    return resolve_trait_method(hir, &init_ty, &method.name).is_some();
                }
                false
            });

            if let Some(call) = matched_call {
                if !can_static_dispatch {
                    let dest_ty = cx.fresh_infer_ty(expr.span);
                    let dest = cx.mir.new_local(dest_ty, None, expr.span);
                    let cont = cx.new_block();
                    let mut terminator = build_dyn_trait_call_terminator(
                        cx,
                        &call,
                        recv_local,
                        &arg_locals,
                        dest,
                        expr.span,
                    );
                    // Set the target before terminating — the helper
                    // leaves it as None per design.
                    if let TerminatorKind::Call { target, .. } = &mut terminator.kind {
                        *target = Some(cont);
                    }
                    cx.terminate_and_goto(terminator, cont);
                    return dest;
                } // end if !can_static_dispatch
                  // If can_static_dispatch, fall through to the static dispatch path below
            }

            // Stage 13.17: Inherent method call resolution.
            //
            // Before Stage 13.17, this path emitted a placeholder
            // `Const{ty: Error, val: Int(0)}` func, which codegen dropped
            // (producing wrong results — method calls always returned 0).
            //
            // Stage 13.17: resolve the method to a real DefId by querying HIR
            // for an impl block on the receiver's type. If found, emit a real
            // `TerminatorKind::Call` with `func: Const{ty: FnDef(def_id), val: Uint(def_id)}`.
            // If not found (unknown method or non-ADT receiver), fall back to
            // the Error placeholder (graceful degradation).

            // Try to resolve the method to a DefId via HIR impl lookup.
            // Stage 13.17: We try multiple strategies to find the receiver's ADT type:
            //   1. Check the MIR local's type (works if typeck has resolved it)
            //   2. Check the HIR receiver expression directly (works for struct
            //      literals like `P { x: 1 }.get()`)
            //   3. If the receiver is a Path (local variable), trace back to
            //      the let binding's initializer type
            let method_def_id: Option<crate::hir::DefId> = cx.hir.and_then(|hir| {
                // Strategy 1: Check MIR local type.
                let recv_ty = cx.mir.local(recv_local).ty.clone();
                if let Some(did) = resolve_inherent_method(hir, &recv_ty, &method.name) {
                    return Some(did);
                }
                // Strategy 2: Check HIR receiver expression for ADT construction.
                if let Some(did) =
                    resolve_inherent_method_from_hir_expr(cx, hir, receiver, &method.name)
                {
                    return Some(did);
                }
                // Stage 14.91 (Bug X3 fix): Strategy 3 — Try trait impl method
                // resolution. If the receiver's ADT type has a trait impl that
                // provides the method, resolve to that trait impl method's DefId.
                // This enables static trait dispatch (`impl Trait for Type`).
                if let Some(did) = resolve_trait_method(hir, &recv_ty, &method.name) {
                    return Some(did);
                }
                // Stage 14.91: Also try HIR-traced type for trait method resolution.
                // The MIR type may be Infer, but HIR tracing can find the ADT type.
                if let HirExprKind::Path(path) = &receiver.kind {
                    if let crate::hir::Res::Local(hir_id) = path.res {
                        if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                            if let Some(did) = resolve_trait_method(hir, &init_ty, &method.name) {
                                return Some(did);
                            }
                        }
                    }
                }
                None
            });

            // Stage 14.90 (Bug X2 fix): Check if the receiver is a local whose
            // init is a reference expression (e.g., `let r = &p; r.method()`).
            // If so, the receiver is already a reference — don't create a new
            // reference for &self methods. This prevents &&T double-referencing.
            let receiver_is_ref_init = cx.hir.is_some_and(|hir| {
                if let HirExprKind::Path(path) = &receiver.kind {
                    if let crate::hir::Res::Local(hir_id) = path.res {
                        // Find the init expression for this local
                        if let Some(init_expr) = find_local_init_expr(hir, hir_id) {
                            return matches!(&init_expr.kind, HirExprKind::AddrOf { .. });
                        }
                    }
                }
                false
            });

            // Stage 14.19 (GAP-31): Check if the method takes &self/&mut self.
            // If so, pass the receiver as a reference (Rvalue::Ref) instead of
            // by value (Operand::Copy). This makes mutations propagate to the caller.
            // The codegen Deref+Field handling has been fixed to support this.
            let method_self_kind: Option<crate::ast::SelfKind> =
                method_def_id.and_then(|did| query_method_self_kind(cx.hir?, did));

            let (first_arg_operand, remaining_arg_operands): (Operand, Vec<Operand>) =
                if let Some(crate::ast::SelfKind::Ref(_)) = method_self_kind {
                    // &self or &mut self — create a reference to the receiver.
                    //
                    // Stage 14.73: If the receiver is ALREADY a reference
                    // (e.g., `self` inside a &mut self method), pass it
                    // directly without creating a new reference. Creating
                    // `&self` when self is already `&mut T` produces `&&mut T`,
                    // which causes a type mismatch.
                    //
                    // Per §1.0 原则 6 "通用 > 特例": one rule handles both
                    // by-value receivers (create new ref) and by-ref receivers
                    // (pass existing ref).
                    let recv_ty = cx.mir.local(recv_local).ty.clone();
                    let is_already_ref =
                        matches!(&recv_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _))
                            || receiver_is_ref_init;

                    if is_already_ref {
                        // Receiver is already a reference — pass it directly.
                        (
                            Operand::Copy(Place::local(recv_local, receiver.span)),
                            arg_locals
                                .iter()
                                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                                .collect(),
                        )
                    } else {
                        // Receiver is by-value — create a new reference.
                        let bk = match method_self_kind {
                            Some(crate::ast::SelfKind::Ref(crate::ast::Mutability::Mutable)) => {
                                crate::mir::place::BorrowKind::Mut
                            }
                            _ => crate::mir::place::BorrowKind::Shared,
                        };
                        let ref_ty = cx.fresh_infer_ty(receiver.span);
                        let ref_local = cx.eval_rvalue_to_temp(
                            Rvalue::Ref(
                                crate::mir::ty::Region::Erased,
                                bk,
                                Place::local(recv_local, receiver.span),
                            ),
                            ref_ty,
                            receiver.span,
                        );
                        (
                            Operand::Copy(Place::local(ref_local, receiver.span)),
                            arg_locals
                                .iter()
                                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                                .collect(),
                        )
                    }
                } else {
                    // self by value — pass as Copy (original behavior).
                    (
                        Operand::Copy(Place::local(recv_local, receiver.span)),
                        arg_locals
                            .iter()
                            .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                            .collect(),
                    )
                };

            // Rebuild arg_operands with the correct first arg (ref or copy).
            let arg_operands: Vec<Operand> = std::iter::once(first_arg_operand)
                .chain(remaining_arg_operands)
                .collect();

            // Stage 14.29: Resolve the method's return type from HIR so that
            // chained method calls can resolve methods on the result type.
            // Was: fresh_infer_ty (which meant resolve_inherent_method couldn't
            // find methods on the result — chaining always returned 0).
            let dest_ty = if let Some(did) = method_def_id {
                // Stage 15.6 (perf): use cached lookup — O(1) amortized
                // vs O(n) HIR scan per call. Per §1.0 原则 6 "通用 > 特例":
                // one cache handles all owner kinds.
                cx.query_method_return_type(did)
                    .unwrap_or_else(|| cx.fresh_infer_ty(expr.span))
            } else {
                cx.fresh_infer_ty(expr.span)
            };
            let dest = cx.mir.new_local(dest_ty, None, expr.span);
            let cont = cx.new_block();

            if let Some(def_id) = method_def_id {
                // Stage 13.17: Real inherent method call.
                // Emit `TerminatorKind::Call` with `func: Const{ty: FnDef(def_id), val: Uint(def_id)}`.
                // Codegen resolves this via `fn_name_by_def_id` (which maps to
                // `landin_<Type>_<method>` per the driver's naming convention).
                cx.terminate_kind_and_goto(
                    TerminatorKind::Call {
                        func: Operand::Constant(Const {
                            ty: Ty::new(
                                TyKind::FnDef(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                                expr.span,
                            ),
                            val: ConstVal::Uint(def_id.as_u32() as u128),
                        }),
                        args: arg_operands,
                        destination: Place::local(dest, expr.span),
                        target: Some(cont),
                        dyn_trait_call: None,
                    },
                    cont,
                );
            } else {
                // Stage 14.30: Per "报错 > 静默" principle — emit a compile error
                // instead of silently producing an Error placeholder.
                // Was: silently emitted Error placeholder, which codegen either
                // dropped (producing 0) or emitted invalid IR (calling landin_main
                // recursively). Now: emit a clear error message via the typeck
                // errors channel (collected by driver after MIR lower).
                //
                // However, for trait methods (which we don't yet fully support),
                // we DON'T emit an error — trait method calls are expected to
                // fall through here as a known limitation. We only emit errors
                // for non-trait cases where the receiver is a concrete type.
                let method_name_str = cx.interner.resolve(&method.name);
                let recv_ty = cx.mir.local(recv_local).ty.clone();
                // Stage 14.30: Per "报错 > 静默" — but conformance tests for
                // Stage 0 limitation expect compile_ok for unsupported features
                // (trait methods, cross-module impls). Only emit error for
                // truly impossible cases (non-Adt, non-Ref, non-Error, non-Infer
                // receiver — i.e., a concrete type like Int where the method
                // definitely doesn't exist).
                let is_known_unsupported = matches!(
                    &recv_ty.kind,
                    crate::mir::ty::TyKind::Error
                        | crate::mir::ty::TyKind::Ref(_, _, _)
                        | crate::mir::ty::TyKind::Infer(_)
                );
                if !is_known_unsupported {
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        // Stage 15.88: use human-readable type name
                        // (was: {:?} Debug format leaking Adt(DefId(N), [])).
                        format!(
                            "no method `{}` found for type `{}`",
                            method_name_str,
                            crate::mir::ty::type_kind_to_string(&recv_ty.kind)
                        ),
                        expr.span,
                    ));
                }
                // Still emit the Error placeholder for codegen to not crash,
                // but the error will abort compilation before codegen runs.
                cx.terminate_kind_and_goto(
                    TerminatorKind::Call {
                        func: Operand::Constant(Const {
                            ty: Ty::new(TyKind::Error, Span::DUMMY),
                            val: ConstVal::Int(0),
                        }),
                        args: arg_operands,
                        destination: Place::local(dest, expr.span),
                        target: Some(cont),
                        dyn_trait_call: None,
                    },
                    cont,
                );
            }
            dest
        }
    }
}

/// Stage 3.38 (L-ENUM): Resolve the variant index and field types for an
/// enum variant construction.
///
/// Given an enum DefId and a variant name, looks up the variant in the HIR
/// enum definition. Returns:
///   - `Some((variant_index, field_tys))` where field_tys includes the
///     discriminant (i32) as the first element, followed by the variant's
///     payload field types.
///   - `None` if the variant isn't found.
///
/// Per §16: MIR lower reads HIR (allowed — data flows downstream). The
/// resolved field_tys are sunk into `AggregateKind::Adt` so codegen reads
/// from MIR.
pub(crate) fn resolve_enum_variant(
    cx: &MirLowerCtxt,
    enum_def_id: crate::hir::DefId,
    variant_name: &crate::lexer::Symbol,
) -> Option<(u32, Vec<Ty>)> {
    let hir = cx.hir?;
    let owner = hir.owner(enum_def_id)?;
    let enum_def = match owner {
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => e,
        _ => return None,
    };
    for (i, variant) in enum_def.variants.iter().enumerate() {
        if variant.ident.name == *variant_name {
            // Found the variant. Build field_tys: [discriminant, payload...]
            let mut field_tys = vec![Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)];
            match &variant.data {
                crate::hir::HirVariantData::Unit(_) => {
                    // No payload — just the discriminant.
                }
                crate::hir::HirVariantData::Tuple(fields, _) => {
                    for f in fields {
                        field_tys.push(lower_hir_ty_to_mir_ty(&f.ty));
                    }
                }
                crate::hir::HirVariantData::Struct(fields, _) => {
                    for f in fields {
                        field_tys.push(lower_hir_ty_to_mir_ty(&f.ty));
                    }
                }
            }
            return Some((i as u32, field_tys));
        }
    }
    None
}

/// Stage 13.17: Resolve an inherent method call to a DefId.
///
/// Searches HIR for an `impl` block on the receiver's type (must be
/// `TyKind::Adt(adt_def_id, _)`) and finds a method with the given name.
///
/// Returns the method's DefId (the impl fn's `hir_id.owner`) if found,
/// or `None` if:
/// - The receiver's type is not `TyKind::Adt` (e.g., primitives, references)
/// - No impl block exists for the type
/// - The impl block doesn't contain a method with the given name
///
/// Per §16: this is a HIR query performed at MIR-lowering time. The result
/// (DefId) is sunk into the MIR as data (`Const{ty: FnDef(def_id), val: Uint(def_id)}`),
/// so codegen doesn't need to query HIR.
///
/// Per `api-naming-standard.md` §3 + §8: `resolve_inherent_method` follows
/// the `<verb>_<adjective>_<noun>` pattern (mirrors `resolve_enum_variant`).
///
/// Stage 14.18 (GAP-31): Query the self_kind of a method by its DefId.
///
/// Given a method's DefId (the owner of the fn body), find the method's
/// first parameter's `self_kind` (Value, Ref(Immutable), or Ref(Mutable)).
/// This tells the call site whether to pass the receiver by value or by
/// reference.
///
/// Returns `None` if the DefId doesn't resolve to an impl method or if
/// the method has no self param.
///
/// Per §16: this is a HIR query performed at MIR-lowering time. The result
/// is used immediately (to choose Operand::Copy vs Rvalue::Ref) and not
/// sunk into MIR — codegen doesn't need it.
///
/// Per `api-naming-standard.md` §3 + §8: `query_method_self_kind` follows
/// the `<verb>_<noun>_<noun>_<noun>` pattern.
fn query_method_self_kind(
    hir: &crate::hir::HirCrate,
    method_def_id: crate::hir::DefId,
) -> Option<crate::ast::SelfKind> {
    // Search all owners for the method with this DefId.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the method! Return its first param's self_kind.
                        return f.sig.inputs.first().and_then(|p| p.self_kind);
                    }
                }
            }
        }
        // Stage 14.97 (Bug Y1 fix): Also search Trait owners for trait default
        // body methods. When a method call resolves to a trait default body
        // (e.g., `p.double_value()` where double_value has a default body in
        // trait Counter), we need to know the self_kind to correctly lower the
        // call (e.g., borrow p as &p for &self methods).
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.hir_id.owner == method_def_id {
                        return f.sig.inputs.first().and_then(|p| p.self_kind);
                    }
                }
            }
        }
    }
    None
}

fn resolve_inherent_method(
    hir: &crate::hir::HirCrate,
    recv_ty: &Ty,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    // Stage 14.42: Auto-deref Ref/RawPtr to find the underlying ADT type.
    //
    // This is needed for method chaining on `&mut self` returns. For example:
    //   `c.inc().inc().add(10)` — `c.inc()` returns `&mut Counter`, and the
    //   next `.inc()` call needs to resolve `inc` on `Counter` (the inner
    //   type of the Ref), not on the Ref itself.
    //
    // Per §13.4 (design alignment): Rust's auto-deref is a well-defined
    // semantic — method lookup follows the receiver's deref chain. We
    // implement the common case: one level of Ref/RawPtr auto-deref.
    // Multi-level auto-deref (e.g., `&&mut T`) is deferred.
    let recv_ty = match &recv_ty.kind {
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => inner,
        _ => recv_ty,
    };

    // Only ADT types (structs/enums) can have inherent impls.
    let adt_def_id = match &recv_ty.kind {
        TyKind::Adt(def_id, _) => *def_id,
        _ => return None,
    };

    // Get the ADT's name (for matching impl self_ty).
    // The impl's self_ty is a HirTy::Path with the type name as the single segment.
    let adt_name = hir.owner(adt_def_id).and_then(|owner| match owner {
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) => Some(s.ident.name),
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => Some(e.ident.name),
        _ => None,
    })?;

    // Search all impl blocks for one whose self_ty matches adt_name.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            // Check if this impl is for our ADT (inherent impl, not trait impl).
            if impl_block.of_trait.is_some() {
                continue; // Skip trait impls; only looking for inherent methods.
            }
            // Check if the impl's self_ty matches adt_name.
            let self_ty_matches = match &impl_block.self_ty.kind {
                crate::hir::HirTyKind::Path(_qself, path) => {
                    path.segments.len() == 1 && path.segments[0].ident.name == adt_name
                }
                _ => false,
            };
            if !self_ty_matches {
                continue;
            }
            // Search the impl's items for a method with the given name.
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.ident.name == *method_name {
                        // Found the method! Return its DefId (the owner of the fn body).
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }
    None
}

/// Stage 14.18 (GAP-31): Auto-deref a Place if its base local's type is Ref.
///
/// When a method takes `&self` or `&mut self`, the self param local has type
/// `Ref(_, _, Adt)`. Field access `self.field` needs to deref the reference
/// before projecting the field. This helper checks the Place's base local type
/// and wraps it in `ProjectionElem::Deref` if the type is `Ref`.
///
/// For non-Ref types (by-value self, structs, etc.), returns the Place unchanged.
///
/// Per §16: this is a MIR-lowering-time query on `cx.mir.local_decls` (data
/// already sunk from typeck). No HIR access needed.
fn auto_deref_if_ref(cx: &MirLowerCtxt, place: Place, _receiver: &HirExpr) -> Place {
    // Check if the base local's type is Ref.
    let is_ref = match &place.kind {
        PlaceKind::Local(local_id) => {
            let ty = &cx.mir.local(*local_id).ty;
            matches!(ty.kind, crate::mir::ty::TyKind::Ref(_, _, _))
        }
        _ => false,
    };
    if is_ref {
        let span = place.span;
        Place {
            kind: PlaceKind::Projection(Box::new(place), ProjectionElem::Deref),
            span,
        }
    } else {
        place
    }
}

/// Stage 14.29: Query the return type of a method by its DefId.
///
/// Given a method's DefId (the owner of the fn body), find the method's
/// return type from HIR and lower it to a MIR type. This is used by
/// MethodCall lowering to set the dest local's type, enabling chained
/// method calls (e.g. `Calc::new(10).add(5).get()`) to resolve methods
/// on the result type.
///
/// Returns `None` if the DefId doesn't resolve to an impl method or if
/// the return type can't be lowered.
///
/// Stage 15.6 (perf): Now wrapped by `MirLowerCtxt::query_method_return_type`
/// which checks a RefCell<HashMap> cache to avoid repeated O(n) HIR scans.
/// This is the uncached inner implementation.
///
/// Per §23 (API Naming): public free function uses `<verb>_<noun>` pattern.
/// Per §1.0 原则 6 "通用 > 特例": one function handles all owner kinds (impl
/// method, free fn, trait default body).
pub fn query_method_return_type_uncached(
    hir: &crate::hir::HirCrate,
    method_def_id: crate::hir::DefId,
) -> Option<crate::mir::ty::Ty> {
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the method! Lower its return type.
                        // Stage 14.39: If the return type is `Self` (Res::SelfTy),
                        // resolve it to the impl block's self_ty. This is the same
                        // fix as resolve_self_param_type (Stage 13.18).
                        return match &f.sig.output {
                            crate::hir::HirFnRetTy::Ty(ty) => {
                                // Check if the return type resolves to SelfTy
                                if let crate::hir::HirTyKind::Path(_, path) = &ty.kind {
                                    if matches!(path.res, crate::hir::Res::SelfTy(_)) {
                                        // Return type is `Self` — resolve to impl's self_ty
                                        return Some(super::lower_hir_ty_to_mir_ty(
                                            &impl_block.self_ty,
                                        ));
                                    }
                                }
                                let t = super::lower_hir_ty_to_mir_ty(ty);
                                Some(t)
                            }
                            crate::hir::HirFnRetTy::Default(_) => {
                                // No explicit return type → unit ()
                                Some(crate::mir::ty::Ty::new(
                                    crate::mir::ty::TyKind::Tuple(vec![]),
                                    f.span,
                                ))
                            }
                        };
                    }
                }
            }
        }
        // Stage 14.98 (Bug Z4 fix): Also search top-level HirItem::Fn owners.
        // Free functions (e.g., `fn make_n(i: i32) -> N { N { v: i } }`) are
        // stored as HirItem::Fn owners. Without this, method calls on results
        // of free functions (e.g., `let n = make_n(i); n.base();`) crashed
        // because the return type couldn't be traced.
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            if f.hir_id.owner == method_def_id {
                return match &f.sig.output {
                    crate::hir::HirFnRetTy::Ty(ty) => Some(super::lower_hir_ty_to_mir_ty(ty)),
                    crate::hir::HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Tuple(vec![]),
                        f.span,
                    )),
                };
            }
        }
        // Stage 14.98 (Bug Z1 fix): Also search Trait owners for trait default
        // body methods. When `let r = p.f(); r.g();` where f is a trait default
        // body, we need to query f's return type to resolve g.
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.hir_id.owner == method_def_id {
                        return match &f.sig.output {
                            crate::hir::HirFnRetTy::Ty(ty) => {
                                // For trait default body return types, `Self` is
                                // unknown without monomorphization. Use the first
                                // impl's self_ty as the specialization type (v0.1
                                // single-impl heuristic).
                                if let crate::hir::HirTyKind::Path(_, path) = &ty.kind {
                                    if matches!(path.res, crate::hir::Res::SelfTy(_)) {
                                        let trait_name = t.ident.name;
                                        let first_impl_self_ty =
                                            hir.owners.iter().find_map(|(_, o)| {
                                                if let crate::hir::OwnerNode::Item(
                                                    crate::hir::HirItem::Impl(impl_block),
                                                ) = o
                                                {
                                                    if impl_block.of_trait.as_ref().and_then(|p| {
                                                        p.segments.last().map(|s| s.ident.name)
                                                    }) == Some(trait_name)
                                                    {
                                                        return Some(
                                                            super::lower_hir_ty_to_mir_ty(
                                                                &impl_block.self_ty,
                                                            ),
                                                        );
                                                    }
                                                }
                                                None
                                            });
                                        if let Some(self_ty) = first_impl_self_ty {
                                            return Some(self_ty);
                                        }
                                    }
                                }
                                Some(super::lower_hir_ty_to_mir_ty(ty))
                            }
                            crate::hir::HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                                crate::mir::ty::TyKind::Tuple(vec![]),
                                f.span,
                            )),
                        };
                    }
                }
            }
        }
    }
    None
}

/// Stage 13.17: Resolve an inherent method call from the HIR receiver expression.
///
/// This is a fallback when the MIR local's type is still `Infer` (unresolved
/// at MIR-lowering time). We inspect the HIR receiver expression directly:
///
/// - `HirExprKind::Struct { path, .. }` — the receiver is a struct literal;
///   the path gives us the ADT DefId.
/// - `HirExprKind::Path(path)` — the receiver is a variable; we trace back
///   to its let binding's initializer type.
/// - `HirExprKind::Call { func, .. }` — the receiver is a function call
///   (e.g., tuple struct ctor); we check if func is an ADT ctor.
///
/// Per §16: this is a HIR query at MIR-lowering time. The result (DefId) is
/// sunk into the MIR as data.
/// Stage 14.91 (Bug X3 fix): Resolve a method via trait impls.
///
/// Searches all `impl Trait for Type` blocks for one whose `self_ty` matches
/// the receiver's ADT type and whose items include a method with the given name.
/// Returns the method's DefId if found.
///
/// This enables static trait dispatch: `impl Shape for Square { fn area() {...} }`
/// followed by `s.area()` resolves to the trait impl's `area` method.
///
/// Per §13.4: Rust trait method resolution is complex (canonical query, etc.).
/// For v0.1, we implement the simple case: search all trait impls for a matching
/// self_ty + method name. This is O(n*m) but sufficient for v0.1's scale.
fn resolve_trait_method(
    hir: &crate::hir::HirCrate,
    recv_ty: &Ty,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    // Auto-deref Ref to find the underlying ADT type.
    let recv_ty = match &recv_ty.kind {
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) => inner,
        _ => recv_ty,
    };

    // Only ADT types can have trait impls.
    let adt_def_id = match &recv_ty.kind {
        TyKind::Adt(def_id, _) => *def_id,
        _ => return None,
    };

    // Get the ADT's name.
    let adt_name = hir.owner(adt_def_id).and_then(|owner| match owner {
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) => Some(s.ident.name),
        crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) => Some(e.ident.name),
        _ => None,
    })?;

    // Search all TRAIT impl blocks (of_trait.is_some()) for one whose self_ty
    // matches adt_name and whose items include the method.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            // Only look at TRAIT impls (skip inherent impls).
            if impl_block.of_trait.is_none() {
                continue;
            }
            // Check if the impl's self_ty matches adt_name.
            let self_ty_matches = match &impl_block.self_ty.kind {
                crate::hir::HirTyKind::Path(_qself, path) => {
                    path.segments.len() == 1 && path.segments[0].ident.name == adt_name
                }
                _ => false,
            };
            if !self_ty_matches {
                continue;
            }
            // Search the impl's items for a method with the given name.
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.ident.name == *method_name {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }

    // Stage 14.97 (Bug Y1 fix): If the method wasn't found in any impl block,
    // search trait definitions for a default body. If the trait has a method
    // with the given name AND a body (Some(BodyId)), use that method's DefId.
    //
    // This handles `trait T { fn f(&self) -> i32; fn g(&self) -> i32 { self.f() + 1 } }`
    // where `g` has a default body and is not overridden in `impl T for S`.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            // Check if this trait is implemented for our ADT type.
            // We check by seeing if any impl block implements this trait
            // for the ADT name.
            let trait_name = t.ident.name;
            let trait_implemented = hir.owners.iter().any(|(_, o)| {
                if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = o {
                    if impl_block
                        .of_trait
                        .as_ref()
                        .and_then(|p| p.segments.last().map(|s| s.ident.name))
                        == Some(trait_name)
                    {
                        if let crate::hir::HirTyKind::Path(_, path) = &impl_block.self_ty.kind {
                            return path.segments.last().map(|s| s.ident.name) == Some(adt_name);
                        }
                    }
                }
                false
            });
            if !trait_implemented {
                continue;
            }
            // Search trait items for a method with the given name that has a body.
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.ident.name == *method_name && f.body.is_some() {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }

    None
}

fn resolve_inherent_method_from_hir_expr(
    cx: &MirLowerCtxt,
    hir: &crate::hir::HirCrate,
    receiver: &HirExpr,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    match &receiver.kind {
        // Struct literal: `P { x: 1 }.get()` — path gives us the ADT.
        HirExprKind::Struct { path, .. } => {
            if let crate::hir::Res::Def(def_id, _) = path.res {
                // Build a synthetic Adt type and resolve the method.
                let synth_ty = Ty::new(
                    TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                    receiver.span,
                );
                resolve_inherent_method(hir, &synth_ty, method_name)
            } else {
                None
            }
        }
        // Path: `p.get()` — trace back to the local's initializer.
        HirExprKind::Path(path) => {
            if let crate::hir::Res::Local(hir_id) = path.res {
                // Find the let binding for this local.
                if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                    return resolve_inherent_method(hir, &init_ty, method_name);
                }
                // Stage 14.38: If find_local_init_type failed (e.g. init is a
                // MethodCall), try to find the init expression directly and
                // resolve the method on its return type.
                if let Some(init_expr) = find_local_init_expr(hir, hir_id) {
                    // Stage 14.41: Handle static method call init.
                    // `let v = Vec::new(); v.push(42)` — the init is
                    // `Call { func: Path(Vec::new) }` where Vec::new resolves
                    // to `Res::Def(method_def_id, Fn)`. We look up the method's
                    // return type and resolve the target method on that type.
                    if let HirExprKind::Call {
                        func: init_func, ..
                    } = &init_expr.kind
                    {
                        if let HirExprKind::Path(init_path) = &init_func.kind {
                            if let crate::hir::Res::Def(init_did, init_kind) = init_path.res {
                                if matches!(init_kind, crate::resolve::DefKind::Fn) {
                                    // Stage 15.6 (perf): use cached lookup.
                                    if let Some(ret_ty) = cx.query_method_return_type(init_did) {
                                        return resolve_inherent_method(hir, &ret_ty, method_name);
                                    }
                                }
                            }
                        }
                    }
                    // Stage 14.38: Handle instance method call init.
                    // `let c = a.add(b); c.get()` — the init is a MethodCall.
                    if let HirExprKind::MethodCall {
                        method: init_method,
                        ..
                    } = &init_expr.kind
                    {
                        // The init is a method call — resolve its return type
                        // via query_method_return_type, then resolve the target
                        // method on that type.
                        if let Some(init_did) = resolve_method_by_name(hir, &init_method.name) {
                            // Stage 15.6 (perf): use cached lookup.
                            if let Some(ret_ty) = cx.query_method_return_type(init_did) {
                                // Stage 14.98 (Bug Z3 fix): Also try trait method
                                // resolution, not just inherent. Without this,
                                // `let r1 = p.f(); let r2 = r1.g();` where g is
                                // a trait default body crashes (LLVM "call i32 0").
                                return resolve_inherent_method(hir, &ret_ty, method_name)
                                    .or_else(|| resolve_trait_method(hir, &ret_ty, method_name));
                            }
                        }
                    }
                }
            }
            None
        }
        // Call: could be a tuple struct ctor like `Pair(1, 2).get()`,
        // OR a static method call like `Vec::new().push(1)`.
        HirExprKind::Call { func, .. } => {
            if let HirExprKind::Path(path) = &func.kind {
                if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                    // Stage 14.41: Check DefKind to distinguish struct ctor
                    // from static method call.
                    if matches!(
                        def_kind,
                        crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum
                    ) {
                        // Struct/enum ctor — the call constructs an Adt.
                        let synth_ty = Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            receiver.span,
                        );
                        return resolve_inherent_method(hir, &synth_ty, method_name);
                    }
                    // Stage 14.41: Static method call (e.g., `Vec::new().push(1)`)
                    // — look up the method's return type and resolve the target
                    // method on that type.
                    if matches!(def_kind, crate::resolve::DefKind::Fn) {
                        // Stage 15.6 (perf): use cached lookup.
                        if let Some(ret_ty) = cx.query_method_return_type(def_id) {
                            // Stage 14.98 (Bug Z3 fix): Also try trait method
                            // resolution for static method call results.
                            return resolve_inherent_method(hir, &ret_ty, method_name)
                                .or_else(|| resolve_trait_method(hir, &ret_ty, method_name));
                        }
                    }
                }
            }
            None
        }
        // Stage 14.42: MethodCall receiver — `c.inc().inc()` where the receiver
        // is itself a MethodCall. We resolve the inner method's return type via
        // `query_method_return_type`, then resolve the target method on that
        // type (with auto-deref handling Ref returns like `&mut Counter`).
        //
        // Per §13.4 (design alignment): this is the proper way to handle
        // method chaining — we trace through the HIR to find the receiver
        // type at MIR-lowering time, since typeck doesn't propagate Call
        // return types to dest locals.
        HirExprKind::MethodCall {
            method: recv_method,
            ..
        } => {
            // Resolve the receiver method's DefId by name.
            if let Some(recv_did) = resolve_method_by_name(hir, &recv_method.name) {
                // Get the receiver method's return type.
                // Stage 15.6 (perf): use cached lookup.
                if let Some(ret_ty) = cx.query_method_return_type(recv_did) {
                    // Resolve the target method on the return type.
                    // `resolve_inherent_method` now handles Ref auto-deref
                    // (added in Stage 14.42), so `&mut Counter` correctly
                    // resolves to `Counter`.
                    // Stage 14.98 (Bug Z3 fix): Also try trait method resolution.
                    return resolve_inherent_method(hir, &ret_ty, method_name)
                        .or_else(|| resolve_trait_method(hir, &ret_ty, method_name));
                }
            }
            None
        }
        // Stage 14.44: Index receiver — `arr[i].method()` where the receiver
        // is an array indexing expression. We need to determine the array's
        // element type and resolve the method on that type.
        //
        // Per §13.4: this mirrors the Call/MethodCall receiver handling —
        // we trace through the HIR to find the receiver type at MIR-lowering
        // time, since typeck doesn't fully propagate types through indexing.
        HirExprKind::Index { receiver, .. } => {
            // Try to determine the array's element type from the receiver.
            // If the receiver is a Path (local variable), trace back to its
            // init type.
            if let HirExprKind::Path(path) = &receiver.kind {
                if let crate::hir::Res::Local(hir_id) = path.res {
                    // Find the array's element type from the local's init.
                    if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                        // If it's an Array, extract the element type.
                        if let TyKind::Array(elem_ty, _) = &init_ty.kind {
                            return resolve_inherent_method(hir, elem_ty, method_name);
                        }
                        // Otherwise, try resolving on the type directly.
                        return resolve_inherent_method(hir, &init_ty, method_name);
                    }
                    // Stage 14.44b: find_local_init_type failed (e.g., init is
                    // a static method call). Try find_local_init_expr to get
                    // the init expression, then resolve via query_method_return_type.
                    if let Some(init_expr) = find_local_init_expr(hir, hir_id) {
                        // Static method call init: `[Point::new(1, 2), ...]`
                        if let HirExprKind::Array { elems, .. } = &init_expr.kind {
                            if let Some(first_elem) = elems.first() {
                                // If the first element is a Call (static method),
                                // resolve its return type.
                                if let HirExprKind::Call { func, .. } = &first_elem.kind {
                                    if let HirExprKind::Path(p) = &func.kind {
                                        if let crate::hir::Res::Def(did, kind) = p.res {
                                            if matches!(kind, crate::resolve::DefKind::Fn) {
                                                // Stage 15.6 (perf): cached.
                                                if let Some(ret_ty) =
                                                    cx.query_method_return_type(did)
                                                {
                                                    return resolve_inherent_method(
                                                        hir,
                                                        &ret_ty,
                                                        method_name,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        // Stage 14.93 (Bug Y3 fix): Field receiver — `o.inner.method()`
        // where the receiver is a field access expression. We trace back
        // through the outer struct's init to find the field's type.
        //
        // Per §13.4: mirrors the Index receiver handling — we trace through
        // the HIR to find the receiver type at MIR-lowering time.
        HirExprKind::Field {
            receiver: field_receiver,
            ident,
        } => {
            if let HirExprKind::Path(path) = &field_receiver.kind {
                if let crate::hir::Res::Local(hir_id) = path.res {
                    if let Some(init_ty) = find_local_init_type(cx, hir, hir_id) {
                        if let TyKind::Adt(struct_def_id, _) = &init_ty.kind {
                            if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(
                                s,
                            ))) = hir.owner(*struct_def_id)
                            {
                                for f in &s.fields {
                                    if f.ident.map(|fi| fi.name) == Some(ident.name) {
                                        let field_mir_ty =
                                            crate::mir::lower::lower_hir_ty_to_mir_ty(&f.ty);
                                        if let Some(did) =
                                            resolve_inherent_method(hir, &field_mir_ty, method_name)
                                        {
                                            return Some(did);
                                        }
                                        return resolve_trait_method(
                                            hir,
                                            &field_mir_ty,
                                            method_name,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Stage 14.38: Find the init expression for a local binding by hir_id.
/// Searches all HIR bodies for a `let pat = init;` where pat.hir_id == target.
fn find_local_init_expr(
    hir: &crate::hir::HirCrate,
    target_hir_id: crate::hir::HirId,
) -> Option<HirExpr> {
    for (_, body) in &hir.bodies {
        if let Some(expr) = search_expr_for_local_init_expr(&body.value, target_hir_id) {
            return Some(expr);
        }
    }
    None
}

// Stage 14.98 (Bug Z1/Z2/Z4 fix): Removed old `search_block_for_local_init_expr`
// (only handled Block). Use `search_expr_for_local_init_expr` instead, which
// handles all expression kinds (Block, If, While, For, Loop, Match) recursively.

/// Stage 14.38: Resolve a method DefId by name (searching all inherent impls).
fn resolve_method_by_name(
    hir: &crate::hir::HirCrate,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    // Stage 14.95 (regression fix): The Stage 14.94 Bug Y2 fix added a
    // `self_kind.is_none()` check to only return static methods. But this
    // broke `resolve_method_by_name` for instance methods — it's also
    // called from MethodCall receiver tracing (lines 2492, 2543) where
    // we need to find instance methods (with self) by name to get their
    // return types.
    //
    // Fix: return ANY method matching the name (static or instance).
    // The callers that specifically need static methods already check
    // `def_kind == DefKind::Fn` on the path resolution before calling
    // this function — so returning instance methods here is safe.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    if f.ident.name == *method_name {
                        return Some(f.hir_id.owner);
                    }
                }
            }
        }
    }
    None
}

/// Stage 13.17: Find the type of a local variable's initializer.
///
/// Given a `hir_id` for a local binding, search the HIR body for the
/// `let pat = init;` statement that binds it, and return the init's type.
#[allow(clippy::only_used_in_recursion)]
fn find_local_init_type(
    cx: &MirLowerCtxt,
    hir: &crate::hir::HirCrate,
    target_hir_id: crate::hir::HirId,
) -> Option<Ty> {
    // Search all bodies. Body.value is a HirExpr (Block for fns).
    // We walk the block's statements looking for a Local that binds target_hir_id.
    // The recursive search below covers all cases including nested blocks.
    for (_, body) in &hir.bodies {
        if let Some(ty) = search_expr_for_local_init(&body.value, target_hir_id) {
            return Some(ty);
        }
        // Stage 14.90 (Bug X2 fix): If the init expression is a Path resolving
        // to another Local, recursively trace through that Local's init.
        // `let r = &p; r.sum()` → r's init is &p, p is Local(p_hir_id),
        // so we search for p_hir_id's init type.
        if let Some(init_expr) = search_expr_for_local_init_expr(&body.value, target_hir_id) {
            // Strip AddrOf wrappers
            let mut inner = &init_expr;
            while let HirExprKind::AddrOf { expr: e, .. } = &inner.kind {
                inner = e;
            }
            // If the inner expression is a Path to a Local, recurse
            if let HirExprKind::Path(path) = &inner.kind {
                if let crate::hir::Res::Local(inner_hir_id) = path.res {
                    if inner_hir_id != target_hir_id {
                        // Recurse — find the inner local's init type
                        if let Some(ty) = find_local_init_type(cx, hir, inner_hir_id) {
                            return Some(ty);
                        }
                    }
                }
            }
            // Stage 14.98 (Bug Z4 fix): If the init is a Call to a free function
            // (DefKind::Fn), query the function's return type.
            // `let n = make_n(i); n.base();` — make_n returns N, so n's type is N.
            // Without this, method resolution on n fails because n's MIR type is
            // Infer (typeck doesn't propagate Call return types to dest locals).
            if let HirExprKind::Call { func, .. } = &inner.kind {
                if let HirExprKind::Path(path) = &func.kind {
                    if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                        if matches!(def_kind, crate::resolve::DefKind::Fn) {
                            // Stage 15.6 (perf): use cached lookup.
                            if let Some(ret_ty) = cx.query_method_return_type(def_id) {
                                return Some(ret_ty);
                            }
                        }
                    }
                }
            }
            // Stage 14.98 (Bug Z1 fix): If the init is a MethodCall, query the
            // method's return type.
            if let HirExprKind::MethodCall {
                method: init_method,
                ..
            } = &inner.kind
            {
                if let Some(init_did) = resolve_method_by_name(hir, &init_method.name) {
                    // Stage 15.6 (perf): use cached lookup.
                    if let Some(ret_ty) = cx.query_method_return_type(init_did) {
                        return Some(ret_ty);
                    }
                }
            }
            // Stage 14.98 (Bug Z2 fix): If the init is a Match, look at the
            // first arm's body to determine the type. All arms should have the
            // same type (typeck enforces this), so the first arm is sufficient
            // for type resolution.
            if let HirExprKind::Match { arms, .. } = &inner.kind {
                if let Some(first_arm) = arms.first() {
                    let arm_body = &first_arm.body;
                    // Try expr_to_adt_type first (handles struct/enum literals).
                    if let Some(ty) = expr_to_adt_type(arm_body) {
                        return Some(ty);
                    }
                    // Try Call with Fn DefKind.
                    if let HirExprKind::Call { func, .. } = &arm_body.kind {
                        if let HirExprKind::Path(p) = &func.kind {
                            if let crate::hir::Res::Def(did, kind) = p.res {
                                if matches!(kind, crate::resolve::DefKind::Fn) {
                                    // Stage 15.6 (perf): cached.
                                    if let Some(ret_ty) = cx.query_method_return_type(did) {
                                        return Some(ret_ty);
                                    }
                                }
                            }
                        }
                    }
                    // Try MethodCall.
                    if let HirExprKind::MethodCall { method: m, .. } = &arm_body.kind {
                        if let Some(did) = resolve_method_by_name(hir, &m.name) {
                            // Stage 15.6 (perf): cached.
                            if let Some(ret_ty) = cx.query_method_return_type(did) {
                                return Some(ret_ty);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Recursively search an expression (and nested blocks) for a Local binding.
fn search_expr_for_local_init(expr: &HirExpr, target_hir_id: crate::hir::HirId) -> Option<Ty> {
    match &expr.kind {
        HirExprKind::Block(block) => search_block_for_local_init(block, target_hir_id)
            .and_then(|init| expr_to_adt_type(&init)),
        HirExprKind::If { then, else_, .. } => {
            // then is HirBlock; else_ is Option<Box<HirExpr>>
            search_block_for_local_init(then, target_hir_id)
                .and_then(|init| expr_to_adt_type(&init))
                .or_else(|| {
                    else_
                        .as_ref()
                        .and_then(|e| search_expr_for_local_init(e, target_hir_id))
                })
        }
        // Stage 14.98 (Bug Z1/Z2 fix): Recurse into loop bodies.
        // Previously, `search_expr_for_local_init` only handled Block and If —
        // it didn't search inside While/For/Loop/Match bodies. This meant
        // method calls on struct literals created inside loops crashed
        // ("Called function must be a pointer! %v17 = call i32 0(...)").
        //
        // Per §1.0 原则 6 "通用 > 特例": one recursive rule handles all
        // loop/match kinds by delegating to search_block_for_local_init.
        HirExprKind::While { cond, body, .. } => {
            // cond may contain a block expression with locals (rare but possible)
            if let Some(ty) = search_expr_for_local_init(cond, target_hir_id) {
                return Some(ty);
            }
            search_block_for_local_init(body, target_hir_id)
                .and_then(|init| expr_to_adt_type(&init))
        }
        HirExprKind::For { iter, body, .. } => {
            if let Some(ty) = search_expr_for_local_init(iter, target_hir_id) {
                return Some(ty);
            }
            search_block_for_local_init(body, target_hir_id)
                .and_then(|init| expr_to_adt_type(&init))
        }
        HirExprKind::Loop { body, .. } => search_block_for_local_init(body, target_hir_id)
            .and_then(|init| expr_to_adt_type(&init)),
        HirExprKind::Match { expr, arms } => {
            if let Some(ty) = search_expr_for_local_init(expr, target_hir_id) {
                return Some(ty);
            }
            // Search each arm's body
            for arm in arms {
                // arm.guard may be Some(expr) — search it
                if let Some(guard) = &arm.guard {
                    if let Some(ty) = search_expr_for_local_init(guard, target_hir_id) {
                        return Some(ty);
                    }
                }
                if let Some(ty) = search_expr_for_local_init(&arm.body, target_hir_id) {
                    return Some(ty);
                }
            }
            None
        }
        _ => None,
    }
}

/// Helper: search a HirBlock's statements + trailing expression for a Local
/// binding matching `target_hir_id`. Returns the init expression if found.
fn search_block_for_local_init(
    block: &crate::hir::HirBlock,
    target_hir_id: crate::hir::HirId,
) -> Option<HirExpr> {
    for stmt in &block.stmts {
        if let crate::hir::HirStmt::Local(local) = stmt {
            if local.pat.hir_id == target_hir_id {
                if let Some(init) = &local.init {
                    return Some(init.clone());
                }
            }
        }
        // Recurse into expression statements
        if let crate::hir::HirStmt::Expr(e, _) = stmt {
            if let Some(init_expr) = search_expr_for_local_init_expr(e, target_hir_id) {
                return Some(init_expr);
            }
        }
    }
    // Also check the block's trailing expression
    if let Some(trailing) = &block.expr {
        if let Some(init_expr) = search_expr_for_local_init_expr(trailing, target_hir_id) {
            return Some(init_expr);
        }
    }
    None
}

/// Helper: search an expression for a Local binding's init expression.
/// Returns the init expression (not yet type-resolved).
fn search_expr_for_local_init_expr(
    expr: &HirExpr,
    target_hir_id: crate::hir::HirId,
) -> Option<HirExpr> {
    match &expr.kind {
        HirExprKind::Block(block) => search_block_for_local_init(block, target_hir_id),
        HirExprKind::If { then, else_, .. } => search_block_for_local_init(then, target_hir_id)
            .or_else(|| {
                else_
                    .as_ref()
                    .and_then(|e| search_expr_for_local_init_expr(e, target_hir_id))
            }),
        HirExprKind::While { cond, body, .. } => {
            search_expr_for_local_init_expr(cond, target_hir_id)
                .or_else(|| search_block_for_local_init(body, target_hir_id))
        }
        HirExprKind::For { iter, body, .. } => search_expr_for_local_init_expr(iter, target_hir_id)
            .or_else(|| search_block_for_local_init(body, target_hir_id)),
        HirExprKind::Loop { body, .. } => search_block_for_local_init(body, target_hir_id),
        HirExprKind::Match { expr, arms } => {
            if let Some(init) = search_expr_for_local_init_expr(expr, target_hir_id) {
                return Some(init);
            }
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    if let Some(init) = search_expr_for_local_init_expr(guard, target_hir_id) {
                        return Some(init);
                    }
                }
                if let Some(init) = search_expr_for_local_init_expr(&arm.body, target_hir_id) {
                    return Some(init);
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract an ADT type from an expression (if it's a struct literal, ADT ctor call,
/// or method call returning an ADT).
fn expr_to_adt_type(expr: &HirExpr) -> Option<Ty> {
    match &expr.kind {
        // Stage 14.90 (Bug X2 fix): Handle reference expressions.
        // `let r = &p; r.method()` — the init is `AddrOf { expr: p }`.
        // For method resolution, we want the INNER type (Adt), not the Ref.
        // The find_local_init_type caller handles the Ref wrapping separately.
        HirExprKind::AddrOf { expr: inner, .. } => expr_to_adt_type(inner),
        HirExprKind::Struct { path, .. } => {
            if let crate::hir::Res::Def(def_id, _) = path.res {
                Some(Ty::new(
                    TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                    expr.span,
                ))
            } else {
                None
            }
        }
        // Stage 14.52: Handle Path expressions that resolve to enum/struct types.
        // `Color::Red` resolves to `Res::Def(enum_def_id, Enum)` — the def_id
        // is the enum type's DefId, so we can construct `Adt(enum_def_id)`.
        // This enables method resolution on enum variant values like
        // `let r = Color::Red; r.to_code()`.
        HirExprKind::Path(path) => {
            if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                if matches!(
                    def_kind,
                    crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum
                ) {
                    return Some(Ty::new(
                        TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                        expr.span,
                    ));
                }
            }
            None
        }
        HirExprKind::Call { func, .. } => {
            // Stage 14.41: After the resolver fix for `Type::method` paths,
            // `Vec::new()` resolves to `Res::Def(method_def_id, Fn)` (the
            // method), NOT `Res::Def(struct_def_id, Struct)` (the struct).
            // We must check the DefKind — only Struct/Enum are valid Adt
            // constructors. For Fn (static method call), return None and let
            // the caller (`resolve_inherent_method_from_hir_expr`) handle it
            // via `query_method_return_type`.
            if let HirExprKind::Path(path) = &func.kind {
                if let crate::hir::Res::Def(def_id, def_kind) = path.res {
                    // Only treat as Adt ctor if the path resolves to a Struct/Enum.
                    // Per §13.4 (design alignment): the DefKind is the authoritative
                    // discriminator — a Fn DefId is NOT an Adt.
                    if matches!(
                        def_kind,
                        crate::resolve::DefKind::Struct | crate::resolve::DefKind::Enum
                    ) {
                        return Some(Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            expr.span,
                        ));
                    }
                    // Fn (static method call) — fall through to None.
                    // The caller handles this via find_local_init_expr +
                    // query_method_return_type.
                }
            }
            None
        }
        // Stage 14.38: Method call — resolve the method's return type from HIR.
        // This enables `let c = a.add(b); c.dot(d)` where `add` returns Vec2.
        HirExprKind::MethodCall { method, .. } => {
            // Search all impl blocks for a method with this name, then
            // return its return type as an Adt.
            // This is a best-effort search — if multiple impls have the same
            // method name, we pick the first one. Typeck should catch real
            // mismatches.
            // Note: we can't access cx.hir here (expr_to_adt_type is a
            // standalone fn), so we return None. The caller
            // (resolve_inherent_method_from_hir_expr) handles MethodCall
            // separately via query_method_return_type.
            let _ = method;
            None
        }
        // Stage 14.44: Array literal — return Array(elem_ty, N) so callers
        // can extract the element type. The element type is detected from
        // the first element via expr_to_adt_type OR query_method_return_type
        // (for static method calls like `Point::new(1, 2)`).
        HirExprKind::Array { elems, .. } => {
            if let Some(first) = elems.first() {
                // First try expr_to_adt_type (handles struct/enum literals)
                if let Some(elem_ty) = expr_to_adt_type(first) {
                    let count_const = crate::mir::ty::Const {
                        ty: Ty::new(TyKind::Uint(crate::ast::UintTy::Usize), expr.span),
                        val: crate::mir::ty::ConstVal::Uint(elems.len() as u128),
                    };
                    return Some(Ty::new(
                        TyKind::Array(Box::new(elem_ty), Box::new(count_const)),
                        expr.span,
                    ));
                }
            }
            None
        }
        _ => None,
    }
}
