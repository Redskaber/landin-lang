//! Call lowering: dyn trait call + closure call + method call expression.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.132):
//! Extracted from `expr_operand.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains call-related helpers + the MethodCall match arm.
//!
//! ## Sub-responsibility
//! Call lowering: convert HIR call expressions (function calls, method calls,
//! closure calls, dyn trait calls) into MIR terminators + operands.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (call lowering)
//! - J3: no circular deps (called by expr_operand; no callback)
//! - J4: call lowering sub-responsibility is complete in this file
//! - J5: stays within mir::lower stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use crate::hir::*;
use crate::mir::body::*;
use crate::mir::dyn_trait::DynTraitMethodCall;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::method_resolution::auto_deref_if_ref;
use super::MirLowerCtxt;
// Stage 18.132: lower_expr_to_operand is recursive (called by lower_expr_to_place + lower_closure_call_to_synthesized)
use super::field_resolution;
use super::lower_expr_to_operand;

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
pub(super) fn lower_expr_to_place(cx: &mut MirLowerCtxt, expr: &HirExpr) -> Place {
    match &expr.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id) = path.res {
                if let Some(local_id) = cx.find_local(hir_id) {
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
        // Stage 18.237 (Store-through-Deref on GEP result fix): When the
        // expression is a Binary (e.g., `p + 0` pointer arithmetic), lower
        // it via `lower_expr_to_operand` which correctly generates a
        // GetElementPtr, then return the result local as the Place.
        //
        // Previously, Binary expressions fell through to the `_` arm which
        // created a fresh Infer local — losing the GEP result and causing
        // codegen to load from the wrong local.
        //
        // Per §1.0 原則 6 (通解 > 特解): one path for all Binary expressions.
        // Per §10 (DRY): reuses lower_expr_to_operand (which handles GEP).
        HirExprKind::Binary { .. } => {
            let local = super::lower_expr_to_operand(cx, expr);
            Place::local(local, expr.span)
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
/// Stage 16.16 (Task 10 Steps 3+4): Lower a closure call to a
/// `TerminatorKind::Call` to the synthesized `call` function.
///
/// This replaces the Stage 13.3a inline approach. Instead of inlining
/// the closure body at the call site, we emit a `TerminatorKind::Call`
/// to the synthesized `call` function (built in Stage 16.14).
///
/// The call passes:
/// - `func`: `Operand::Copy(closure_local)` — the closure struct as `self`
/// - `args`: the call arguments
/// - `destination`: a fresh result local
///
/// The synthesized function's DefId is stored in the closure struct's
/// `TyKind::Closure(def_id, ...)` type. Codegen resolves the function
/// name via `fn_name_by_def_id`.
///
/// Stage 16.29 (通解): This is now the SOLE closure call lowering path
/// — ALL closures (no-capture, i32-capture, struct-capture, nested-capture)
/// go through this function. The `lower_closure_call_inline` path is
/// retained as `#[deprecated]` for backward compatibility with tests
/// that might still reference it, but is no longer invoked by the
/// production closure call dispatch.
///
/// Per §1.0 原則 6 "通用 > 特例": one call path for all closures.
/// Per §16: no HIR access at call site — DefId is in the type.
pub(super) fn lower_closure_call_to_synthesized(
    cx: &mut MirLowerCtxt,
    closure_local: LocalId,
    arg_locals: &[LocalId],
    expr: &HirExpr,
) -> LocalId {
    // Get the closure struct's type to extract the DefId.
    let closure_ty = cx.mir.local(closure_local).ty.clone();
    let closure_def_id = match &closure_ty.kind {
        crate::mir::ty::TyKind::Closure(def_id, _) => *def_id,
        _ => {
            // Defensive: if the closure local doesn't have a Closure type,
            // fall back to a fresh infer ty (shouldn't happen).
            let infer_ty = cx.fresh_infer_ty(expr.span);
            return cx.eval_rvalue_to_temp(
                Rvalue::Use(Operand::Copy(Place::local(closure_local, expr.span))),
                infer_ty,
                expr.span,
            );
        }
    };

    // Build the func operand: a FnDef-typed constant pointing to the
    // synthesized closure call function. The DefId is the closure's DefId.
    let func_ty = Ty::new(
        crate::mir::ty::TyKind::FnDef(closure_def_id, Vec::new().into()),
        expr.span,
    );
    let func_local = cx.mir.new_local(func_ty.clone(), None, expr.span);
    cx.mir
        .block_mut(cx.current_block)
        .statements
        .push(Statement {
            kind: StatementKind::Assign(Box::new((
                Place::local(func_local, expr.span),
                Rvalue::Use(Operand::Constant(crate::mir::ty::Const {
                    ty: func_ty.clone(),
                    val: crate::mir::ty::ConstVal::Uint(closure_def_id.as_u32() as u128),
                })),
            ))),
            span: expr.span,
        });

    // Build the call arguments: [self (closure struct), args...]
    let mut call_args: Vec<Operand> = Vec::new();
    // First arg: the closure struct (self).
    // Stage 16.22: For closures without captures (empty struct), use
    // Operand::Copy — the empty struct is effectively Copy, and this
    // allows chained calls like f(f(f(0))).
    // For closures WITH captures, use Operand::Move (borrowck requires
    // it for non-Copy types). But capture closures use the inline path,
    // so this code is only reached for no-capture closures.
    // Stage 16.23: For closures WITH captures, Closure is not Copy.
    // Use Operand::Move (borrowck requires it). But codegen passes the
    // closure struct by pointer (alloca), so the Move doesn't actually
    // consume the value — the pointer is just passed to the callee.
    // For no-capture closures, Copy is used (Closure is Copy when empty).
    let closure_ty = cx.mir.local(closure_local).ty.clone();
    let is_copy_closure = matches!(
        &closure_ty.kind,
        crate::mir::ty::TyKind::Closure(_, substs) if substs.is_empty()
    );
    let self_operand = if is_copy_closure {
        Operand::Copy(Place::local(closure_local, expr.span))
    } else {
        Operand::Move(Place::local(closure_local, expr.span))
    };
    call_args.push(self_operand);
    // Remaining args: the call arguments.
    for &arg_local in arg_locals {
        call_args.push(Operand::Copy(Place::local(arg_local, expr.span)));
    }

    // Create a destination local for the call result.
    let dest_ty = cx.fresh_infer_ty(expr.span);
    let dest = cx.mir.new_local(dest_ty, None, expr.span);

    // Emit the TerminatorKind::Call.
    let cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Copy(Place::local(func_local, expr.span)),
            args: call_args,
            destination: Place::local(dest, expr.span),
            target: Some(cont),
            dyn_trait_call: None,
        },
        cont,
    );

    dest
}
