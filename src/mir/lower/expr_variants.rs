//! Expression variant lowering: Path, Call, For, MethodCall.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.133):
//! Extracted from `expr_operand.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains the 4 largest HirExprKind match arms, extracted as functions.
//!
//! ## Sub-responsibility
//! Each function lowers one HirExprKind variant to a MIR LocalId.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (expression variant lowering)
//! - J3: no circular deps (called by expr_operand; recursive call via super::lower_expr_to_operand)
//! - J4: each variant is a complete sub-responsibility
//! - J5: stays within mir::lower stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use crate::ast::Ident;
use crate::hir::*;
use crate::mir::body::*;
use crate::mir::dyn_trait::{find_dyn_trait_method_call_in_plan_by_method, DynTraitMethodCall};
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::call_lower::{build_dyn_trait_call_terminator, lower_closure_call_to_synthesized};
use super::lower_expr_to_operand;
use super::method_resolution::{
    find_local_init_expr, find_local_init_type, query_method_self_kind, resolve_enum_variant,
    resolve_inherent_method, resolve_inherent_method_from_hir_expr, resolve_trait_method,
};
use super::ty_lower::lower_path_generic_args;
use super::MirLowerCtxt;
use super::{control_flow, field_resolution, pattern_bindings};

/// Lower a Path expression to a MIR operand (Stage 18.133: extracted from lower_expr_to_operand).
pub(super) fn lower_path_expr(cx: &mut MirLowerCtxt, expr: &HirExpr, path: &HirPath) -> LocalId {
    // If the path resolves to a local, return that local.
    if let Res::Local(hir_id) = path.res {
        if let Some(local_id) = cx.find_local(hir_id) {
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
                            //
                            // Stage 16.52 (Task 11 Phase 1c): propagate
                            // generic args from path into Adt substs.
                            // Per §1.0 原則 6 "通用 > 特例": one path
                            // for all generic enum variants.
                            let substs =
                                lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                            let adt_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
                            let discr = Operand::Constant(Const {
                                ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                                val: ConstVal::Uint(variant_idx as u128),
                            });
                            return cx.eval_rvalue_to_temp(
                                Rvalue::Aggregate(
                                    AggregateKind::Adt(def_id, variant_idx, substs, field_tys),
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
                // Stage 16.52 (Task 11 Phase 1c): propagate generic
                // args from path into Adt substs (consistent with
                // lower_hir_ty_to_mir_ty_with_regions).
                let substs = lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                let adt_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
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
                                hir_crate.find_owner(def_id)
                            {
                                match item {
                                    crate::hir::HirItem::Const(c) => {
                                        // Lower the const's body expression to get its value.
                                        if let Some(body) = hir_crate.find_body(c.body) {
                                            let const_local =
                                                lower_expr_to_operand(cx, &body.value);
                                            let ld = cx.mir.local_decls.get(const_local.0 as usize);
                                            if let Some(ld) = ld {
                                                return cx.eval_rvalue_to_temp(
                                                    Rvalue::Use(Operand::Copy(Place::local(
                                                        const_local,
                                                        expr.span,
                                                    ))),
                                                    ld.ty.clone(),
                                                    expr.span,
                                                );
                                            }
                                        }
                                    }
                                    crate::hir::HirItem::Static(s) => {
                                        // Statics are like consts but with a fixed memory location.
                                        // For Stage 3.44, treat same as const.
                                        if let Some(body) = hir_crate.find_body(s.body) {
                                            let static_local =
                                                lower_expr_to_operand(cx, &body.value);
                                            let ld =
                                                cx.mir.local_decls.get(static_local.0 as usize);
                                            if let Some(ld) = ld {
                                                return cx.eval_rvalue_to_temp(
                                                    Rvalue::Use(Operand::Copy(Place::local(
                                                        static_local,
                                                        expr.span,
                                                    ))),
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
                        // Stage 18.101: Propagate generic args from path
                        // into FnDef substs. For paths without turbofish
                        // (e.g., `id(42)` without `::<i32>`), substs may
                        // be empty — type inference back-propagation is
                        // needed to fill them (v0.2 work, tracked as
                        // TD-MONO-INFER). The substs are populated by
                        // lower_path_generic_args which reads explicit
                        // turbofish args from the path.
                        let substs =
                            lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                        let fndef_ty = Ty::new(TyKind::FnDef(def_id, substs), expr.span);
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
                        // Stage 18.101: Propagate generic args from path
                        // into FnDef substs. For paths without turbofish,
                        // substs may be empty — see TD-MONO-INFER note above.
                        let substs =
                            lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                        let fndef_ty = Ty::new(TyKind::FnDef(def_id, substs), expr.span);
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
    // Stage 18.62: Push TypeError for unresolved path expressions.
    // Per §1.0 原則 4 "报错 > 静默": previously silently returned
    // TyKind::Error without any diagnostic.
    cx.type_errors.push(crate::typeck::TypeError::new(
        "cannot find value in this scope",
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

/// Lower a Call expression to a MIR operand (Stage 18.133: extracted from lower_expr_to_operand).
pub(super) fn lower_call_expr(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    func: &HirExpr,
    args: &[HirExpr],
) -> LocalId {
    // Lower func first — this determines whether the call is a real
    // function call or an ADT construction (struct/enum ctor).
    let func_local = lower_expr_to_operand(cx, func);
    let arg_locals: Vec<LocalId> = args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();
    // Stage 16.06: Use Operand::Move for call arguments.
    // Previously always used Operand::Copy, which failed the
    // borrow checker's Copy-ness check for non-Copy types (e.g.,
    // `E::A(Inner { x: 42 })` where Inner has `impl Drop`).
    // The borrow checker's Operand::Move path (Stage 15.73) skips
    // move recording for Copy types, so Move is safe for both.
    // Per §1.0 原則 9 "正确 > 妥协": always-Move is sound.
    let arg_operands: Vec<Operand> = arg_locals
        .iter()
        .map(|l| Operand::Move(Place::local(*l, Span::DUMMY)))
        .collect();

    // Stage 13.3a (TD-030): Closure call dispatch.
    //
    // Before falling through to the existing FnDef / Adt / placeholder
    // dispatch, check if `func_local` is registered in the
    // `cx.closure_bodies` side-table. If yes, this is a closure call —
    // lower it to a `TerminatorKind::Call` to the synthesized `call`
    // function (Strategy A).
    //
    // The side-table is keyed by LocalId (not DefId) because:
    // (a) at MIR lowering time, we don't have a unique per-closure
    //     DefId allocation mechanism;
    // (b) the call site has the func_local (LocalId), not a DefId;
    // (c) closure info propagates through `let` bindings (the let
    //     lowering in `control_flow::lower_block` propagates the
    //     info from init_local to let_local).
    //
    // Per §16: the closure body is HIR data sunk into the lowering
    // context as a side-table. No HIR access from codegen.
    //
    // Stage 16.29 (通解 — Typeck on synthesized closure MIR bodies):
    // ALL closures now use the synthesized `call` function path.
    // The previous `has_complex_captures` special-case routing is
    // removed because the typeck gap is fixed — the driver runs
    // typeck on each synthesized closure MIR body, resolving all
    // Infer types (return type, param types) for ANY capture type,
    // including Adt and Closure captures.
    //
    // Stage 16.34 (Task 10 Step 5 — cleanup): Replaced the
    // `closure_bodies.contains_key` side-table lookup with a
    // type-based check. The closure literal's local has type
    // `Closure(def_id, substs)` (concrete, not Infer) at MIR
    // lowering time. Let-bound closures (`let g = |x| ...;`)
    // inherit this type via the let lowering (control_flow.rs
    // line 598-604: uses init_local's type if not Infer).
    //
    // This eliminates the `closure_bodies` side-table (TD-CLOSURE-2)
    // and the `ClosureBodyInfo` struct — the type system is the
    // single source of truth for "is this local a closure?".
    //
    // Per §1.0 原則 5 "去除兼容思维": dead side-table removed.
    // Per §1.0 原則 6 "通用 > 特例": one type-based check for all
    // closure-typed locals (literal, let-bound, re-let-bound).
    // Per §23 rule 5 (DRY): type is the single source of truth.
    let is_closure_typed = {
        let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
        func_local_decl
            .map(|ld| matches!(&ld.ty.kind, TyKind::Closure(_, _)))
            .unwrap_or(false)
    };
    if is_closure_typed {
        return lower_closure_call_to_synthesized(cx, func_local, &arg_locals, expr);
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
        //
        // Stage 16.53 (Task 11 Phase 2): Use
        // `resolve_adt_field_tys_with_substs` when adt_substs is
        // non-empty, so generic struct fields get substituted.
        let (variant_idx, field_tys) = if let HirExprKind::Path(path) = &func.kind {
            if path.segments.len() >= 2 {
                if let Some((idx, tys)) =
                    resolve_enum_variant(cx, adt_def_id, &path.segments[1].ident.name)
                {
                    (idx, tys)
                } else if adt_substs.is_empty() {
                    (0, field_resolution::resolve_adt_field_tys(cx, adt_def_id))
                } else {
                    (
                        0,
                        field_resolution::resolve_adt_field_tys_with_substs(
                            cx,
                            adt_def_id,
                            &adt_substs,
                        ),
                    )
                }
            } else if adt_substs.is_empty() {
                (0, field_resolution::resolve_adt_field_tys(cx, adt_def_id))
            } else {
                (
                    0,
                    field_resolution::resolve_adt_field_tys_with_substs(
                        cx,
                        adt_def_id,
                        &adt_substs,
                    ),
                )
            }
        } else if adt_substs.is_empty() {
            (0, field_resolution::resolve_adt_field_tys(cx, adt_def_id))
        } else {
            (
                0,
                field_resolution::resolve_adt_field_tys_with_substs(cx, adt_def_id, &adt_substs),
            )
        };
        // For enum variants, the Aggregate operands need to include
        // the discriminant as the first element. For structs,
        // variant_idx = 0 and field_tys are the struct's fields.
        let mut all_operands = Vec::new();
        if variant_idx > 0
            || (cx
                .hir
                .and_then(|h| h.find_owner(adt_def_id))
                .is_some_and(|o| {
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
        let dest_ty = Ty::new(TyKind::Adt(adt_def_id, adt_substs.clone()), expr.span);
        let dest = cx.mir.new_local(dest_ty, None, expr.span);
        cx.push_assign(
            Place::local(dest, expr.span),
            Rvalue::Aggregate(
                AggregateKind::Adt(adt_def_id, variant_idx, adt_substs, field_tys),
                all_operands,
            ),
            expr.span,
        );
        dest
    } else {
        // Stage 16.30 (通解 — dead code cleanup):
        // The old Stage 4.13 `is_closure` inline path is removed.
        // It was dead code because:
        // 1. If the closure is a literal or let-bound from a literal
        //    → `closure_bodies.contains_key` is TRUE → handled above
        //    by `lower_closure_call_to_synthesized`.
        // 2. If the closure is a call result (e.g., `f()()`) → the
        //    type is Infer at lowering time → `is_closure` was FALSE.
        //
        // For case 2, the "Real function call" path below emits a
        // generic `Call { func: Copy(func_local), args }`. The
        // codegen (Stage 16.30) handles Closure-typed func operands
        // by resolving the synthesized function name and prepending
        // the closure struct as self.
        //
        // Per §1.0 原則 5 "去除兼容思维": dead code is removed.
        // Per §1.0 原則 6 "通用 > 特例": one codegen path for
        // all closure-typed calls.
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

/// Lower a For expression to a MIR operand (Stage 18.133: extracted from lower_expr_to_operand).
pub(super) fn lower_for_expr(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    pat: &HirPat,
    iter: &HirExpr,
    body: &HirBlock,
) -> LocalId {
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
            return cx
                .mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span);
        }
    };

    // Range must have a start and end (we don't support open ranges yet).
    let start_expr = match start_expr {
        Some(s) => s,
        None => {
            cx.type_errors.push(crate::typeck::TypeError::new(
                "for-loop over open range (..start / start..) is not supported in v0.1".to_string(),
                expr.span,
            ));
            return cx
                .mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span);
        }
    };
    let end_expr = match end_expr {
        Some(e) => e,
        None => {
            cx.type_errors.push(crate::typeck::TypeError::new(
                "for-loop over open range (..start / start..) is not supported in v0.1".to_string(),
                expr.span,
            ));
            return cx
                .mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span);
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
// This is a soundness bug — `let f = |x| { println!(...); x + 1 }

/// Lower a MethodCall expression to a MIR operand (Stage 18.133: extracted from lower_expr_to_operand).
pub(super) fn lower_method_call_expr(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    receiver: &HirExpr,
    method: &Ident,
    args: &[HirExpr],
) -> LocalId {
    let recv_local = lower_expr_to_operand(cx, receiver);
    let arg_locals: Vec<LocalId> = args.iter().map(|a| lower_expr_to_operand(cx, a)).collect();

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
        if resolve_inherent_method_from_hir_expr(cx, hir, receiver, &method.name).is_some() {
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
        if let Some(did) = resolve_inherent_method_from_hir_expr(cx, hir, receiver, &method.name) {
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
            let is_already_ref = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _))
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
                // Stage 16.85: use cx.format_ty for resolver-backed names.
                format!(
                    "no method `{}` found for type `{}`",
                    method_name_str,
                    cx.format_ty(&recv_ty)
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
