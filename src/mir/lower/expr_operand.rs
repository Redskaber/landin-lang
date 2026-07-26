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
            let field_ty = field_resolution::resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
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

/// Stage 5.78: Build a `Terminator::Call` for a dyn Trait method call,
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
/// A `Terminator::Call` whose:
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
    // Push the call info into the side-table; the index becomes the marker.
    let index = cx.mir.dyn_trait_calls.len() as u128;
    cx.mir.dyn_trait_calls.push(call.clone());

    // Build the args list: self first, then explicit args.
    let mut arg_operands: Vec<Operand> = vec![Operand::Copy(Place::local(recv_local, span))];
    for local in arg_locals {
        arg_operands.push(Operand::Copy(Place::local(*local, Span::DUMMY)));
    }

    Terminator::Call {
        // The Const's Int value is the side-table index. Codegen detects
        // this marker and emits a vtable indirect call instead of a direct
        // function call.
        func: Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
            val: ConstVal::Int(index),
        }),
        args: arg_operands,
        destination: Place::local(dest, span),
        target: None,
    }
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

/// Stage 13.3a (TD-030): Determine whether a capture's type is Copy.
///
/// This is a duplicate of `borrowck::copy_semantics::ty_is_copy` — inlined
/// here to avoid a `mir::lower → borrowck` dependency (§16 violation:
/// borrowck runs after mir::lower in the pipeline).
///
/// Returns `true` for:
/// - Primitives: bool, char, int, uint, float
/// - References (`&T`), raw pointers, fn defs, fn ptrs
/// - Tuples/arrays of Copy types
/// - Infer/Error (assumed Copy to avoid spurious errors during inference)
/// - Adt (assumed Copy as a fallback — precise check needs TraitResolver,
///   which is not available at MIR lower time)
///
/// Returns `false` for:
/// - `Str`, `Slice(_)`, `Closure(_, _)`, `Param(_)` — these are non-Copy
///
/// A future refactor should move Copy-ness detection to a neutral location
/// (e.g., `mir::ty` or a new `ty::copy_semantics` module) so both
/// `mir::lower` and `borrowck` can share the same logic without violating §16.
fn is_capture_ty_copy(ty: &Ty) -> bool {
    use crate::mir::ty::TyKind::*;
    match &ty.kind {
        Bool | Char | Int(_) | Uint(_) | Float(_) => true,
        Ref(_, _, _) => true,
        RawPtr(_, _) => true,
        FnDef(_, _) | FnPtr(_) => true,
        Never => true,
        Tuple(tys) => tys.iter().all(is_capture_ty_copy),
        Array(inner, _) => is_capture_ty_copy(inner),
        Infer(_) | Error | Foreign => true,
        Adt(_, _) => true,
        Str | Slice(_) | Closure(_, _) | Param(_) => false,
    }
}

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
                                    let adt_ty =
                                        Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                                    let discr = Operand::Constant(Const {
                                        ty: Box::new(Ty::new(
                                            TyKind::Int(crate::ast::IntTy::I32),
                                            Span::DUMMY,
                                        )),
                                        val: ConstVal::Uint(variant_idx as u128),
                                    });
                                    return cx.eval_rvalue_to_temp(
                                        Rvalue::Aggregate(
                                            AggregateKind::Adt(
                                                def_id,
                                                variant_idx,
                                                Vec::new(),
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
                        let adt_ty = Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: Box::new(adt_ty.clone()),
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
                                            _ => {}
                                        }
                                    }
                                }
                                // Fallback: treat as FnDef (error recovery).
                                let fndef_ty =
                                    Ty::new(TyKind::FnDef(def_id, Vec::new()), expr.span);
                                return cx.eval_rvalue_to_temp(
                                    Rvalue::Use(Operand::Constant(Const {
                                        ty: Box::new(fndef_ty.clone()),
                                        val: ConstVal::Uint(def_id.as_u32() as u128),
                                    })),
                                    fndef_ty,
                                    expr.span,
                                );
                            }
                            _ => {
                                // Default: treat as FnDef (covers Fn, etc.).
                                let fndef_ty =
                                    Ty::new(TyKind::FnDef(def_id, Vec::new()), expr.span);
                                return cx.eval_rvalue_to_temp(
                                    Rvalue::Use(Operand::Constant(Const {
                                        ty: Box::new(fndef_ty.clone()),
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
                    ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
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
            let binop_ty = cx.fresh_infer_ty(expr.span);
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
            let unary_ty = cx.fresh_infer_ty(expr.span);
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
            //   - TyKind::FnDef(..)       → real Terminator::Call.
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
                        ty: Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
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
                        AggregateKind::Adt(adt_def_id, variant_idx, Vec::new(), field_tys),
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
                // avoiding the incorrect Terminator::Call that would treat the
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
                    let capture_tys: Vec<Ty> = match &closure_ty.kind {
                        TyKind::Closure(_, substs) => substs.clone(),
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
                    cx.terminate_and_goto(
                        Terminator::Call {
                            func: Operand::Copy(Place::local(func_local, func.span)),
                            args: arg_operands,
                            destination: Place::local(dest, expr.span),
                            target: Some(cont),
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
            }
            cx.terminate(Terminator::Return);
            // Return a dummy local (unreachable after Return)
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }
        HirExprKind::Assign { lhs, rhs, .. } => {
            let rhs_local = lower_expr_to_operand(cx, rhs);
            // Stage 3.34 (L-MUT-1 fix): handle assignment LHS that are
            // projections (field access, index, deref). Was: only handled
            // `Path` LHS — `a.v = 42` fell through to "just evaluate rhs"
            // and silently dropped the mutation.
            //
            // Per §15: root-cause fix (handle all LHS shapes in the Assign
            // lower), not a hack (e.g., special-casing field mutation in
            // codegen).
            let lhs_place = lower_expr_to_place(cx, lhs);
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
            let tuple_ty = cx.fresh_infer_ty(expr.span);
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
            let result = cx.mir.new_local(field_ty, None, expr.span);
            cx.push_assign(
                Place::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(base_local, receiver.span)),
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
            let ref_ty = cx.fresh_infer_ty(expr.span);
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
            let result = cx.mir.new_local(result_ty, None, expr.span);

            // Entry → goto loop_header
            cx.terminate(Terminator::Goto(loop_header));

            // loop_header → goto loop_body_start (placeholder for future
            // condition checking / break targeting)
            cx.current_block = loop_header;
            cx.terminate(Terminator::Goto(loop_body_start));

            // loop_body_start → lower body → goto loop_header
            cx.current_block = loop_body_start;
            let _body_result = control_flow::lower_block(cx, body);
            cx.terminate(Terminator::Goto(loop_header));

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
            cx.terminate(Terminator::Goto(cond_block));

            // cond_block: evaluate cond, switchInt
            cx.current_block = cond_block;
            let cond_local = lower_expr_to_operand(cx, cond);
            cx.terminate(Terminator::SwitchInt {
                discr: Operand::Copy(Place::local(cond_local, cond.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // body_block: lower body, goto cond_block
            cx.current_block = body_block;
            control_flow::lower_block(cx, body);
            cx.terminate(Terminator::Goto(cond_block));

            // exit_block: continuation
            cx.current_block = exit_block;
            cx.mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span)
        }

        // For: `for pat in iter { body }` → lower iter, loop with next()
        HirExprKind::For {
            pat: _, iter, body, ..
        } => {
            let iter_local = lower_expr_to_operand(cx, iter);
            let cond_block = cx.new_block();
            let body_block = cx.new_block();
            let exit_block = cx.new_block();

            // Entry → goto cond_block
            cx.terminate(Terminator::Goto(cond_block));

            // cond_block: placeholder — real impl would call iter.next()
            // For Stage 2.4b, we just check if iter is truthy
            cx.current_block = cond_block;
            cx.terminate(Terminator::SwitchInt {
                discr: Operand::Copy(Place::local(iter_local, iter.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // body_block: lower body, goto cond_block
            cx.current_block = body_block;
            control_flow::lower_block(cx, body);
            cx.terminate(Terminator::Goto(cond_block));

            // exit_block: continuation
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
                let operand = if is_capture_ty_copy(&ty) {
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
                TyKind::Closure(closure_def_id, capture_tys.clone()),
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
                            AggregateKind::Closure(closure_def_id, capture_tys.clone()),
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

        // Break: `break expr` → goto loop exit (simplified: just lower expr)
        HirExprKind::Break { expr: br_expr, .. } => {
            if let Some(e) = br_expr {
                let _ = lower_expr_to_operand(cx, e);
            }
            // For Stage 2.4b, Break is simplified — no loop exit targeting.
            // Full implementation requires tracking loop exit blocks.
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Continue: `continue` → goto loop header (simplified)
        HirExprKind::Continue => {
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
                        ty: Box::new(Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY)),
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

        // Repeat: `[val; N]` → Aggregate(Array, [val, val, ...])
        HirExprKind::Repeat { elem, count, .. } => {
            let elem_local = lower_expr_to_operand(cx, elem);
            // For Stage 2.4b, we lower repeat as a 1-element array
            // (real repeat with N requires const-eval, Stage 3+)
            let _ = count;
            let elem_ty = cx.fresh_infer_ty(expr.span);
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(
                    AggregateKind::Array(elem_ty),
                    vec![Operand::Copy(Place::local(elem_local, elem.span))],
                ),
                Ty::new(TyKind::Error, expr.span), // simplified
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
            let operands: Vec<Operand> = field_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            // Stage 3.30 (per §15): if the path resolves to a known struct
            // DefId, use AggregateKind::Adt (the proper representation).
            // Stage 3.38 (L-ENUM): also handle enum struct variants
            // (e.g., `Shape::Circle { r: 1.0 }`).
            if let Res::Def(def_id, DefKind::Struct) = path.res {
                let field_tys = field_resolution::resolve_adt_field_tys(cx, def_id);
                let struct_ty = Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                return cx.eval_rvalue_to_temp(
                    Rvalue::Aggregate(
                        AggregateKind::Adt(def_id, 0, Vec::new(), field_tys),
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
                            ty: Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
                            val: ConstVal::Uint(variant_idx as u128),
                        });
                        let mut all_operands = vec![discr];
                        all_operands.extend(operands);
                        let enum_ty = Ty::new(TyKind::Adt(def_id, Vec::new()), expr.span);
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Aggregate(
                                AggregateKind::Adt(def_id, variant_idx, Vec::new(), field_tys),
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

        // Stage 4.10: MacroCall — expand known built-in macros.
        // Previously (Stage 3.x): all macro calls produced TyKind::Error placeholder.
        // Now: known macros (println!, stringify!, assert!) produce proper MIR.
        // Unknown macros still fall back to Error placeholder.
        HirExprKind::MacroCall { path, .. } => {
            // Get the macro name from the last path segment.
            let macro_name = path.segments.last().map(|s| s.ident.name);
            if let Some(name_spur) = macro_name {
                let name = cx.interner.resolve(&name_spur).to_string();
                match name.as_str() {
                    "println" | "print" | "eprintln" | "eprint" => {
                        // println!(...) → unit expression (no actual printing).
                        // The macro call is valid but produces no value.
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    "stringify" => {
                        // stringify!(expr) → &str type local (simplified).
                        // Since cx.interner is &Rodeo (immutable), we can't
                        // intern a new string here. Produce a str-typed local
                        // without assigning a constant (typeck will resolve).
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
                    "assert" | "debug_assert" => {
                        // assert!(cond) → unit expression (assertion check).
                        // For now, just produce unit (no actual assertion codegen).
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    _ => {
                        // Unknown macro → Error placeholder (fallback).
                        cx.eval_rvalue_to_temp(
                            Rvalue::Use(Operand::Constant(Const {
                                ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                                val: ConstVal::Int(0),
                            })),
                            Ty::new(TyKind::Error, Span::DUMMY),
                            expr.span,
                        )
                    }
                }
            } else {
                // No macro name → Error placeholder.
                cx.eval_rvalue_to_temp(
                    Rvalue::Use(Operand::Constant(Const {
                        ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
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
            let method_name = cx.interner.resolve(&method.name).to_string();
            let matched_call: Option<DynTraitMethodCall> = cx.dyn_trait_plan().and_then(|plan| {
                find_dyn_trait_method_call_in_plan_by_method(plan, &method_name).cloned()
            });
            if let Some(call) = matched_call {
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
                if let Terminator::Call { target, .. } = &mut terminator {
                    *target = Some(cont);
                }
                cx.terminate_and_goto(terminator, cont);
                return dest;
            }

            // Legacy placeholder path (Stage 2.1) — unchanged.
            let arg_operands: Vec<Operand> =
                std::iter::once(Operand::Copy(Place::local(recv_local, receiver.span)))
                    .chain(
                        arg_locals
                            .iter()
                            .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY))),
                    )
                    .collect();
            let dest_ty = cx.fresh_infer_ty(expr.span);
            let dest = cx.mir.new_local(dest_ty, None, expr.span);
            let cont = cx.new_block();
            cx.terminate_and_goto(
                Terminator::Call {
                    func: Operand::Constant(Const {
                        ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),
                        val: ConstVal::Int(0),
                    }), // placeholder func
                    args: arg_operands,
                    destination: Place::local(dest, expr.span),
                    target: Some(cont),
                },
                cont,
            );
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
