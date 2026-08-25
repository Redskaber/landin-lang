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
use super::compute_type_size_with_fallback;
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
                    let variant_name = super::method_resolution::variant_name_from_path(path);
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
                            // Stage 18.159 (TD-SPAN-DUMMY-CLEANUP): use expr.span
                            // for the discriminant constant (was: Span::DUMMY).
                            // Per §2 原则 4 (报错>静默): better error location.
                            let discr = Operand::Constant(Const {
                                ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), expr.span),
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

    // Stage 18.185 (TD-STRING-INTRINSICS): Intercept String::from_str(s)
    // as a builtin intrinsic before falling through to ADT ctor / call paths.
    //
    // String::from_str(s: &str) -> String does:
    //   1. len = s.len (extract from fat pointer field 1)
    //   2. ptr = __landin_alloc(len) (allocate heap buffer)
    //   3. __landin_memcpy(ptr, s.ptr, len) (copy bytes)
    //   4. Construct String { ptr, len, cap: len }
    //
    // Per §1.0 原則 6 (通解>特例): one intrinsic for String::from_str,
    // not a special-case per string literal.
    // Per §2 原則 9 (正确>妥协): proper alloc+copy, not a stub.
    if let HirExprKind::Path(path) = &func.kind {
        if path.segments.len() == 2 {
            let type_name = cx.interner.resolve(&path.segments[0].ident.name);
            let method_name = cx.interner.resolve(&path.segments[1].ident.name);
            if type_name == "String" && method_name == "from_str" && args.len() == 1 {
                return lower_string_from_str_intrinsic(cx, expr, arg_locals[0]);
            }
            // Stage 18.189 (TD-BOX-AUTO-DROP partial): Intercept Box::new(x).
            // Box::new(x) does:
            //   1. size = sizeof(T) (from x's type, hardcoded per type for MVP)
            //   2. ptr = __landin_alloc(size) (allocate heap buffer)
            //   3. *ptr = x (store x into the heap buffer)
            //   4. Construct Box { ptr }
            // Per §1.0 原則 6 (通解>特例): one intrinsic for all Box::new calls.
            if type_name == "Box" && method_name == "new" && args.len() == 1 {
                return lower_box_new_intrinsic(cx, expr, arg_locals[0]);
            }
            // Stage 18.195 (TD-VEC-MVP): Intercept Vec::new() — returns empty Vec.
            // Vec::new() creates Vec { ptr: null, len: 0, cap: 0 }.
            // Per §1.0 原則 6 (通解>特例): one intrinsic for all Vec::new calls.
            if type_name == "Vec" && method_name == "new" && args.is_empty() {
                return lower_vec_new_intrinsic(cx, expr);
            }
        }
    }

    // Stage 18.186 (TD-FORMAT-MACRO): Intercept __landin_format(fmt, ...) calls.
    //
    // format!("hello") expands to __landin_format("hello"), which we
    // intercept here and lower to String::from_str(fmt) — reusing the
    // Stage 18.185 intrinsic (alloc + memcpy + construct).
    //
    // MVP limitation: only format! with a single literal string (no {})
    // is supported. format!("x={}", x) requires variadic arg type handling
    // (TD-FORMAT-VARIADIC, deferred to Stage 18.187+).
    //
    // Per §1.0 原則 6 (通解>特例): reuse String::from_str intrinsic.
    // Per §2 原則 9 (正确>妥协): MVP is a temporary compromise for literals.
    if let HirExprKind::Path(path) = &func.kind {
        if path.segments.len() == 1 {
            let name = cx.interner.resolve(&path.segments[0].ident.name);
            if name == "__landin_format" && args.len() == 1 {
                // format!("literal") → String::from_str(literal)
                return lower_string_from_str_intrinsic(cx, expr, arg_locals[0]);
            }
            if name == "__landin_format" && args.len() > 1 {
                // Stage 18.202 (TD-FORMAT-VARIADIC): format!("x={}", x) → call
                // __landin_format_variadic(out_str, fmt_ptr, fmt_len, n_args,
                //   arg_types, arg_vals).
                // Per §1.0 原則 6 (通解>特例): one C helper for all format! calls.
                return lower_format_variadic_intrinsic(cx, expr, &arg_locals);
            }
        }
    }

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
                if let Some((idx, tys)) = resolve_enum_variant(
                    cx,
                    adt_def_id,
                    super::method_resolution::variant_name_from_path(path),
                ) {
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
            } else if path.segments.len() == 1 {
                // Stage 18.167 (TD-VARIANT-CONSTRUCTOR): 1-segment path
                // (e.g., `Some(42)` without `Option::` prefix). Use the
                // first segment as the variant name.
                // Per §1.0 原則 6 (通解>特例): same resolve_enum_variant call,
                // just using segments[0] instead of segments[1].
                if let Some((idx, tys)) =
                    resolve_enum_variant(cx, adt_def_id, &path.segments[0].ident.name)
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
            // Stage 18.159 (TD-SPAN-DUMMY-CLEANUP): use expr.span (was: Span::DUMMY).
            let discr = Operand::Constant(Const {
                ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), expr.span),
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
        // Stage 18.173 (TD-STR-LEN): Handle str::len() as a builtin intrinsic.
        // &str is a fat pointer {ptr, i64} — not an ADT, so resolve_inherent_method
        // can't find it. We intercept `s.len()` on &str/str and extract the i64
        // length field directly.
        // Per §1.0 原則 6 (通解>特例): one intrinsic check for all str methods.
        let method_name_str = cx.interner.resolve(&method.name);
        let recv_ty = cx.mir.local(recv_local).ty.clone();

        if method_name_str == "len" && args.is_empty() {
            let is_str = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Str)
                || matches!(&recv_ty.kind,
                    crate::mir::ty::TyKind::Ref(_, _, inner)
                        if matches!(&inner.kind, crate::mir::ty::TyKind::Str)
                );
            if is_str {
                let dest_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);
                let dest = cx.mir.new_local(dest_ty.clone(), None, expr.span);
                let cont = cx.new_block();
                cx.push_assign(
                    Place::local(dest, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(1), dest_ty),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );
                cx.terminate_and_goto(
                    Terminator {
                        kind: TerminatorKind::Goto(cont),
                        span: expr.span,
                    },
                    cont,
                );
                return dest;
            }
        }

        // Stage 18.184 (TD-STR-METHODS-RUNTIME): str::is_empty() intrinsic.
        // `s.is_empty()` → `s.len() == 0` (returns bool).
        // Per §1.0 原則 6 (通解>特例): reuse the len() Field projection pattern,
        // then compare to 0 via BinaryOp.
        if method_name_str == "is_empty" && args.is_empty() {
            let is_str = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Str)
                || matches!(&recv_ty.kind,
                    crate::mir::ty::TyKind::Ref(_, _, inner)
                        if matches!(&inner.kind, crate::mir::ty::TyKind::Str)
                );
            if is_str {
                // Step 1: Extract len field (same as len() intrinsic).
                let len_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);
                let len_local = cx.mir.new_local(len_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(len_local, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(1), len_ty.clone()),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );
                // Step 2: Compare len == 0 → bool.
                let bool_ty = Ty::new(TyKind::Bool, expr.span);
                let zero_local = cx.mir.new_local(len_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(zero_local, expr.span),
                    Rvalue::Use(Operand::Constant(Const {
                        val: crate::mir::ty::ConstVal::Int(0),
                        ty: len_ty.clone(),
                    })),
                    expr.span,
                );
                let dest = cx.mir.new_local(bool_ty, None, expr.span);
                let cont = cx.new_block();
                cx.push_assign(
                    Place::local(dest, expr.span),
                    Rvalue::BinaryOp(
                        crate::mir::place::BinOp::Eq,
                        Operand::Copy(Place::local(len_local, expr.span)),
                        Operand::Copy(Place::local(zero_local, expr.span)),
                    ),
                    expr.span,
                );
                cx.terminate_and_goto(
                    Terminator {
                        kind: TerminatorKind::Goto(cont),
                        span: expr.span,
                    },
                    cont,
                );
                return dest;
            }
        }

        // Stage 18.184: str::as_bytes() intrinsic.
        // `s.as_bytes()` → return the same fat pointer as &[u8].
        // &str and &[u8] have the SAME LLVM layout ({ ptr, i64 }), so this
        // is a no-op at the MIR level — just return the receiver.
        // Per §1.0 原則 6 (通解>特例): one fat pointer layout for all byte slices.
        if method_name_str == "as_bytes" && args.is_empty() {
            let is_str = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Str)
                || matches!(&recv_ty.kind,
                    crate::mir::ty::TyKind::Ref(_, _, inner)
                        if matches!(&inner.kind, crate::mir::ty::TyKind::Str)
                );
            if is_str {
                // &[u8] has the same fat pointer layout as &str.
                // Return the receiver directly (no-op).
                return recv_local;
            }
        }

        // Stage 18.189 (TD-STRING-INTRINSICS): String::as_str() intrinsic.
        // `s.as_str()` → construct &str fat pointer { ptr, len } from String fields.
        // String.ptr (field 0) is the data pointer, String.len (field 1) is the length.
        // Per §1.0 原則 6 (通解>特例): one intrinsic for all String::as_str calls.
        if method_name_str == "as_str" && args.is_empty() {
            let is_string = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Adt(_, _))
                && cx.hir.is_some_and(|hir| {
                    // Check if the Adt is the String struct by looking up its DefId.
                    if let crate::mir::ty::TyKind::Adt(did, _) = &recv_ty.kind {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(*did)
                        {
                            let name = cx.interner.resolve(&s.ident.name);
                            return name == "String";
                        }
                    }
                    false
                });
            if is_string {
                use crate::mir::place::AggregateKind;
                let u8_ptr_ty = Ty::new(
                    TyKind::RawPtr(
                        crate::mir::ty::Mutability::Mutable,
                        Box::new(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), expr.span)),
                    ),
                    expr.span,
                );
                let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);

                // Extract ptr (field 0) and len (field 1) from String.
                let ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(ptr_local, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );
                let len_local = cx.mir.new_local(i64_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(len_local, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(1), i64_ty.clone()),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );

                // Construct &str fat pointer { ptr, i64 }.
                // Stage 18.189: We build a Tuple first, then Cast it to &str.
                // The Tuple and &str have the same LLVM layout ({ ptr, i64 }),
                // so the cast is a no-op at codegen level, but it makes the
                // MIR type correct for typeck.
                let str_ty = Ty::new(
                    TyKind::Ref(
                        crate::mir::ty::Region::Erased,
                        crate::mir::ty::Mutability::Immutable,
                        Box::new(Ty::new(TyKind::Str, expr.span)),
                    ),
                    expr.span,
                );
                let tuple_ty = Ty::new(
                    TyKind::Tuple(vec![u8_ptr_ty.clone(), i64_ty.clone()]),
                    expr.span,
                );
                let tuple_local = cx.mir.new_local(tuple_ty.clone(), None, expr.span);
                cx.push_assign(
                    Place::local(tuple_local, expr.span),
                    Rvalue::Aggregate(
                        AggregateKind::Tuple,
                        vec![
                            Operand::Copy(Place::local(ptr_local, expr.span)),
                            Operand::Copy(Place::local(len_local, expr.span)),
                        ],
                    ),
                    expr.span,
                );
                // Cast the Tuple to &str (same layout, different MIR type).
                let dest = cx.mir.new_local(str_ty.clone(), None, expr.span);
                let cont = cx.new_block();
                cx.push_assign(
                    Place::local(dest, expr.span),
                    Rvalue::Cast(
                        crate::mir::place::CastKind::Unsize,
                        Operand::Copy(Place::local(tuple_local, expr.span)),
                        str_ty.clone(),
                    ),
                    expr.span,
                );
                cx.terminate_and_goto(
                    Terminator {
                        kind: TerminatorKind::Goto(cont),
                        span: expr.span,
                    },
                    cont,
                );
                return dest;
            }
        }

        // Stage 18.195 (TD-VEC-MVP): Vec::len() intrinsic.
        // `v.len()` → extract the len field (field 1) from the Vec struct.
        // Per §1.0 原則 6 (通解>特例): same Field projection pattern as str::len.
        if method_name_str == "len" && args.is_empty() {
            let is_vec = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Adt(_, _))
                && cx.hir.is_some_and(|hir| {
                    if let crate::mir::ty::TyKind::Adt(did, _) = &recv_ty.kind {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(*did)
                        {
                            let name = cx.interner.resolve(&s.ident.name);
                            return name == "Vec";
                        }
                    }
                    false
                });
            if is_vec {
                let len_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);
                let dest = cx.mir.new_local(len_ty.clone(), None, expr.span);
                let cont = cx.new_block();
                cx.push_assign(
                    Place::local(dest, expr.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(recv_local, receiver.span)),
                            ProjectionElem::Field(FieldId(1), len_ty),
                        ),
                        span: expr.span,
                    })),
                    expr.span,
                );
                cx.terminate_and_goto(
                    Terminator {
                        kind: TerminatorKind::Goto(cont),
                        span: expr.span,
                    },
                    cont,
                );
                return dest;
            }
        }

        // Stage 18.198 (TD-STRING-INTRINSICS): String::push_str(s) intrinsic.
        // `s.push_str(src)` → call __landin_string_push_str(&s, src.ptr, src.len).
        // Per §1.0 原則 6 (通解>特例): one intrinsic for all String::push_str calls.
        if method_name_str == "push_str" && args.len() == 1 {
            let is_string = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Adt(_, _))
                && cx.hir.is_some_and(|hir| {
                    if let crate::mir::ty::TyKind::Adt(did, _) = &recv_ty.kind {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(*did)
                        {
                            let name = cx.interner.resolve(&s.ident.name);
                            return name == "String";
                        }
                    }
                    false
                });
            if is_string {
                return lower_string_push_str_intrinsic(cx, expr, recv_local, arg_locals[0]);
            }
        }

        // Stage 18.200: Vec::get(index) intrinsic.
        // `v.get(i)` → call __landin_vec_get(&v, i, &out, elem_size).
        // Returns the element at index i (panics on OOB).
        // Per §1.0 原則 6 (通解>特例): one intrinsic for all Vec::get calls.
        if method_name_str == "get" && args.len() == 1 {
            let is_vec = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Adt(_, _))
                && cx.hir.is_some_and(|hir| {
                    if let crate::mir::ty::TyKind::Adt(did, _) = &recv_ty.kind {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(*did)
                        {
                            let name = cx.interner.resolve(&s.ident.name);
                            return name == "Vec";
                        }
                    }
                    false
                });
            if is_vec {
                return lower_vec_get_intrinsic(cx, expr, recv_local, arg_locals[0]);
            }
        }

        // Stage 18.195 (TD-VEC-MVP): Vec::push(x) intrinsic.
        // `v.push(x)` → check if len == cap, realloc if needed, store x at [len], len++.
        // Per §1.0 原則 6 (通解>特例): one intrinsic for all Vec::push calls.
        if method_name_str == "push" && args.len() == 1 {
            let is_vec = matches!(&recv_ty.kind, crate::mir::ty::TyKind::Adt(_, _))
                && cx.hir.is_some_and(|hir| {
                    if let crate::mir::ty::TyKind::Adt(did, _) = &recv_ty.kind {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(*did)
                        {
                            let name = cx.interner.resolve(&s.ident.name);
                            return name == "Vec";
                        }
                    }
                    false
                });
            if is_vec {
                return lower_vec_push_intrinsic(cx, expr, recv_local, arg_locals[0]);
            }
        }

        // Stage 14.30: Per "报错 > 静默" principle — emit a compile error
        // instead of silently producing an Error placeholder.
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
        } else if matches!(&recv_ty.kind, crate::mir::ty::TyKind::Infer(_)) {
            // Stage 18.234 (TD-METHOD-RESOLVE-STRICT fix): For Infer receiver
            // types, defer method resolution to typeck. Record the call info
            // so typeck can re-check after type defaulting (Phase 5.5).
            //
            // Per §1.0 原則 4 (报错>静默): unresolved methods must be reported.
            // Per §1.0 原則 6 (通解>特例): one deferred path for all Infer receivers.
            // Per §17.6 (同类型整体修复): tracks method resolution through typeck.
            cx.mir
                .deferred_method_calls
                .push(crate::mir::body::DeferredMethodCall {
                    recv_local,
                    method_name: method.name,
                    span: expr.span,
                });
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

/// Stage 18.185 (TD-STRING-INTRINSICS): Lower `String::from_str(s: &str) -> String`.
///
/// Generates MIR for:
///   1. len = s.len (extract from &str fat pointer field 1)
///   2. ptr = __landin_alloc(len) (allocate heap buffer)
///   3. data_ptr = s.ptr (extract from &str fat pointer field 0)
///   4. __landin_memcpy(ptr, data_ptr, len) (copy bytes)
///   5. Construct String { ptr, len, cap: len }
///
/// Per §1.0 原則 6 (通解>特例): one intrinsic for all String::from_str calls.
/// Per §2 原則 9 (正确>妥协): proper alloc+memcpy, not a stub.
/// Per §10: `lower_string_from_str_intrinsic` follows `<verb>_<noun>_<noun>_<noun>` pattern.
fn lower_string_from_str_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    src_local: LocalId,
) -> LocalId {
    use crate::mir::place::AggregateKind;

    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), expr.span)),
        ),
        expr.span,
    );

    // Step 1: Extract len from &str fat pointer (field 1).
    let len_local = cx.mir.new_local(i64_ty.clone(), None, expr.span);
    cx.push_assign(
        Place::local(len_local, expr.span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, expr.span)),
                ProjectionElem::Field(FieldId(1), i64_ty.clone()),
            ),
            span: expr.span,
        })),
        expr.span,
    );

    // Step 2: Extract data ptr from &str fat pointer (field 0).
    let data_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, expr.span);
    cx.push_assign(
        Place::local(data_ptr_local, expr.span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, expr.span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span: expr.span,
        })),
        expr.span,
    );

    // Step 3: Call __landin_alloc(len) to get heap buffer.
    // Stage 18.185: Use synthetic DefIds (u32::MAX - 100, u32::MAX - 101)
    // for __landin_alloc and __landin_memcpy. These are registered in
    // driver_validations.rs::register_builtin_macros so codegen can resolve
    // them. The offsets (100, 101) are well outside the BUILTIN_MACRO_NAMES
    // range (max 28 entries) to avoid collision.
    let alloc_def_id = crate::hir::DefId::new(u32::MAX - 100);
    let alloc_fn_ty = Ty::new(
        TyKind::FnDef(alloc_def_id, std::vec::Vec::new().into()),
        expr.span,
    );
    let alloc_fn_local = cx.mir.new_local(alloc_fn_ty, None, expr.span);
    let alloc_ret_ty = u8_ptr_ty.clone();
    let alloc_dest = cx.mir.new_local(alloc_ret_ty.clone(), None, expr.span);
    let alloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(alloc_fn_local, expr.span)),
            args: vec![Operand::Copy(Place::local(len_local, expr.span))],
            destination: Place::local(alloc_dest, expr.span),
            target: Some(alloc_cont),
            dyn_trait_call: None,
        },
        alloc_cont,
    );

    // Step 4: Call __landin_memcpy(alloc_dest, data_ptr, len).
    let memcpy_def_id = crate::hir::DefId::new(u32::MAX - 101);
    let memcpy_fn_ty = Ty::new(
        TyKind::FnDef(memcpy_def_id, std::vec::Vec::new().into()),
        expr.span,
    );
    let memcpy_fn_local = cx.mir.new_local(memcpy_fn_ty, None, expr.span);
    let memcpy_dest = cx.mir.new_local(
        Ty::new(TyKind::Tuple(std::vec![]), expr.span),
        None,
        expr.span,
    );
    let memcpy_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(memcpy_fn_local, expr.span)),
            args: vec![
                Operand::Copy(Place::local(alloc_dest, expr.span)),
                Operand::Copy(Place::local(data_ptr_local, expr.span)),
                Operand::Copy(Place::local(len_local, expr.span)),
            ],
            destination: Place::local(memcpy_dest, expr.span),
            target: Some(memcpy_cont),
            dyn_trait_call: None,
        },
        memcpy_cont,
    );

    // Step 5: Construct String { ptr: alloc_dest, len: len_local, cap: len_local }.
    // Look up the String struct's DefId from HIR by name.
    // Per §1.0 原則 6 (通解>特例): one lookup for all String::from_str calls.
    let string_def_id = {
        let mut found = None;
        if let Some(hir) = cx.hir {
            let string_spur = cx.interner.get("String");
            if let Some(target_name) = string_spur {
                for (def_id, owner) in &hir.owners {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
                        if s.ident.name == target_name {
                            found = Some(*def_id);
                            break;
                        }
                    }
                }
            }
        }
        found
    };

    let string_ty = if let Some(did) = string_def_id {
        Ty::new(TyKind::Adt(did, std::vec::Vec::new().into()), expr.span)
    } else {
        Ty::new(TyKind::Error, expr.span)
    };

    let dest = cx.mir.new_local(string_ty.clone(), None, expr.span);
    let cont = cx.new_block();
    cx.push_assign(
        Place::local(dest, expr.span),
        Rvalue::Aggregate(
            AggregateKind::Adt(
                string_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                std::vec::Vec::new().into(),
                vec![u8_ptr_ty.clone(), i64_ty.clone(), i64_ty.clone()],
            ),
            vec![
                Operand::Copy(Place::local(alloc_dest, expr.span)),
                Operand::Copy(Place::local(len_local, expr.span)),
                Operand::Copy(Place::local(len_local, expr.span)),
            ],
        ),
        expr.span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(cont),
            span: expr.span,
        },
        cont,
    );
    dest
}

/// Stage 18.189 (TD-BOX-AUTO-DROP partial): Lower `Box::new(x) -> Box<T>`.
///
/// Generates MIR for:
///   1. size = sizeof(T) (hardcoded per primitive type for MVP)
///   2. ptr = __landin_alloc(size) (allocate heap buffer)
///   3. *ptr = x (store x into the heap buffer via Deref projection)
///   4. Construct Box { ptr } via Aggregate
///
/// Per §1.0 原則 6 (通解>特例): one intrinsic for all Box::new calls.
/// Per §2 原則 9 (正确>妥协): proper alloc+store, not a stub.
/// Per §10: `lower_box_new_intrinsic` follows `<verb>_<noun>_<noun>_<noun>` pattern.
fn lower_box_new_intrinsic(cx: &mut MirLowerCtxt, expr: &HirExpr, val_local: LocalId) -> LocalId {
    use crate::mir::place::AggregateKind;
    use crate::mir::ty::ConstVal;

    // Step 1: Determine sizeof(T) from the value's type.
    // Stage 18.203: delegates to compute_type_size (single source of truth)
    // - handles primitives, Adt (struct/enum via HIR walk), Tuple, Array.
    // Fallback 8 for Infer/Param/Error (TD-TYPECK-GENERIC-INST, v0.2 P2+).
    let val_ty = cx.mir.local(val_local).ty.clone();
    // Box::new: fallback 8 (safe over-allocation - extra bytes unused by Deref load).
    let size: i64 = compute_type_size_with_fallback(&val_ty, cx.hir, 8);

    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);

    // Step 2: Create size constant and call __landin_alloc(size).
    let size_local = cx.mir.new_local(i64_ty.clone(), None, expr.span);
    cx.push_assign(
        Place::local(size_local, expr.span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(size as u128),
            ty: i64_ty.clone(),
        })),
        expr.span,
    );

    let alloc_def_id = crate::hir::DefId::new(u32::MAX - 100);
    let alloc_fn_ty = Ty::new(
        TyKind::FnDef(alloc_def_id, std::vec::Vec::new().into()),
        expr.span,
    );
    let alloc_fn_local = cx.mir.new_local(alloc_fn_ty, None, expr.span);
    // Stage 18.212: alloc_dest receives the return from __landin_alloc
    // (which is *mut u8), but for typeck purposes we want it to be *mut T
    // so that the store `*alloc_dest = x` type-checks correctly.
    // The actual LLVM codegen handles the bitcast from *mut u8 to *mut T
    // via emit_store (Stage 18.190 TD-BOX-NEW-TYPE-COERCE fix).
    let val_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(val_ty.clone()),
        ),
        expr.span,
    );
    let alloc_dest = cx.mir.new_local(val_ptr_ty, None, expr.span);
    let alloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(alloc_fn_local, expr.span)),
            args: vec![Operand::Copy(Place::local(size_local, expr.span))],
            destination: Place::local(alloc_dest, expr.span),
            target: Some(alloc_cont),
            dyn_trait_call: None,
        },
        alloc_cont,
    );

    // Step 3: Store x into the heap buffer (*alloc_dest = x).
    // Stage 18.212: alloc_dest is *mut u8 (from __landin_alloc return type),
    // but we store val_ty through it. The codegen emit_store handles the
    // pointer bitcast (Stage 18.190 TD-BOX-NEW-TYPE-COERCE fix).
    // Per §1.0 原則 9 (正确>妥协): store the actual value type, not u8.
    cx.push_assign(
        Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(alloc_dest, expr.span)),
                ProjectionElem::Deref,
            ),
            span: expr.span,
        },
        Rvalue::Use(Operand::Move(Place::local(val_local, expr.span))),
        expr.span,
    );

    // Step 4: Construct Box { ptr: alloc_dest }.
    // Look up Box struct's DefId from HIR by name.
    let box_def_id = {
        let mut found = None;
        if let Some(hir) = cx.hir {
            let box_spur = cx.interner.get("Box");
            if let Some(target_name) = box_spur {
                for (def_id, owner) in &hir.owners {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
                        if s.ident.name == target_name {
                            found = Some(*def_id);
                            break;
                        }
                    }
                }
            }
        }
        found
    };

    // Stage 18.212 (TD-TUPLE-CTOR-TYPECK fix): Construct Box<T> with the
    // correct element type substs. Previously hardcoded `Vec::new().into()`
    // (empty substs) and `u8_ptr_ty` as the field type — causing typeck to
    // see Box<u8> regardless of the actual T.
    //
    // Now we extract the element type from the value's type and:
    // 1. Set substs = [val_ty] so Box<Point> has substs [Point]
    // 2. Set field_ty = *mut T (matching the prelude `struct Box<T>(*mut T)`)
    //
    // Per §1.0 原則 6 (通解>特例): one path for all Box<T> types.
    // Per §12 (最优 > 最小): root-cause fix — use actual substs, not empty.
    let box_ty = if let Some(did) = box_def_id {
        Ty::new(TyKind::Adt(did, vec![val_ty.clone()].into()), expr.span)
    } else {
        Ty::new(TyKind::Error, expr.span)
    };

    // The field type is *mut T (matching `struct Box<T>(*mut T)`).
    let box_field_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(val_ty.clone()),
        ),
        expr.span,
    );

    let dest = cx.mir.new_local(box_ty.clone(), None, expr.span);
    let cont = cx.new_block();
    cx.push_assign(
        Place::local(dest, expr.span),
        Rvalue::Aggregate(
            AggregateKind::Adt(
                box_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                vec![val_ty.clone()].into(),
                vec![box_field_ty.clone()],
            ),
            vec![Operand::Copy(Place::local(alloc_dest, expr.span))],
        ),
        expr.span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(cont),
            span: expr.span,
        },
        cont,
    );
    dest
}

/// Stage 18.195 (TD-VEC-MVP): Lower `Vec::new() -> Vec<T>`.
///
/// Creates Vec { ptr: null, len: 0, cap: 0 } — an empty Vec with no allocation.
/// When push is called, alloc/realloc will be invoked as needed.
///
/// Per §1.0 原則 6 (通解>特例): one intrinsic for all Vec::new calls.
fn lower_vec_new_intrinsic(cx: &mut MirLowerCtxt, expr: &HirExpr) -> LocalId {
    use crate::mir::place::AggregateKind;

    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), expr.span)),
        ),
        expr.span,
    );

    // Look up Vec struct's DefId from HIR by name.
    let vec_def_id = {
        let mut found = None;
        if let Some(hir) = cx.hir {
            let vec_spur = cx.interner.get("Vec");
            if let Some(target_name) = vec_spur {
                for (def_id, owner) in &hir.owners {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
                        if s.ident.name == target_name {
                            found = Some(*def_id);
                            break;
                        }
                    }
                }
            }
        }
        found
    };

    let vec_ty = if let Some(did) = vec_def_id {
        Ty::new(TyKind::Adt(did, std::vec::Vec::new().into()), expr.span)
    } else {
        Ty::new(TyKind::Error, expr.span)
    };

    let dest = cx.mir.new_local(vec_ty.clone(), None, expr.span);
    let cont = cx.new_block();

    // Construct Vec { ptr: null, len: 0, cap: 0 }
    cx.push_assign(
        Place::local(dest, expr.span),
        Rvalue::Aggregate(
            AggregateKind::Adt(
                vec_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                std::vec::Vec::new().into(),
                vec![u8_ptr_ty.clone(), i64_ty.clone(), i64_ty.clone()],
            ),
            vec![
                Operand::Constant(Const {
                    val: ConstVal::Int(0),
                    ty: u8_ptr_ty.clone(),
                }),
                Operand::Constant(Const {
                    val: ConstVal::Int(0),
                    ty: i64_ty.clone(),
                }),
                Operand::Constant(Const {
                    val: ConstVal::Int(0),
                    ty: i64_ty.clone(),
                }),
            ],
        ),
        expr.span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(cont),
            span: expr.span,
        },
        cont,
    );
    dest
}

/// Stage 18.229 (v0.2.5e): Lower `Vec::push(x)` via MIR intrinsics.
///
/// Replaces the previous `__landin_vec_push` C helper Call with a pure MIR
/// sequence using the MIR intrinsic ops (Load + GetElementPtr + Store) added
/// in Stage 18.226 and codegen-enabled in Stage 18.227. The growth logic
/// (conditional realloc) is expressed via `SwitchInt` + `Call(__landin_realloc)`.
///
/// Generates MIR for:
///   1. Extract `vec.ptr` (field 0), `vec.len` (field 1), `vec.cap` (field 2)
///   2. `need_grow = BinaryOp(Ge, len, cap)` — len >= cap → must grow
///   3. `SwitchInt(need_grow, [(0, store_bb)], otherwise=grow_bb)` — branch
///   4. grow_bb: `is_zero = BinaryOp(Eq, cap, 0)`; `SwitchInt(is_zero, [(1, zero_cap_bb)], otherwise=nonzero_cap_bb)`
///   5. zero_cap_bb: `new_cap = 4` (initial capacity); goto alloc_bb
///   6. nonzero_cap_bb: `new_cap = cap + cap` (2x growth); goto alloc_bb
///   7. alloc_bb: `new_bytes = new_cap * elem_size`; `old_bytes = cap * elem_size`;
///      `new_ptr = Call(__landin_realloc, [data_ptr, old_bytes, new_bytes])`;
///      `Store(vec.ptr, new_ptr)`; `Store(vec.cap, new_cap)`; goto store_bb
///   8. store_bb: `current_ptr = Use(Projection(recv, Field(0)))` (reload — handles growth);
///      `elem_ptr = GetElementPtr(current_ptr, [len], *mut T)`;
///      `Store(Projection(elem_ptr, Deref), val)` — `*elem_ptr = val`;
///      `new_len = BinaryOp(Add, len, 1)`; `Store(vec.len, new_len)`; goto after
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all Vec<T> types —
/// the element type T flows through `extract_vec_element_type` (Stage 18.208).
/// Per §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible).
/// Per §10 DRY: reuses `extract_vec_element_type`, `compute_type_size_with_fallback`,
/// `MemoryEmitter` methods, `Place::Projection(Deref)` pattern (Stage 14.66).
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR.
/// Per §12 (最优 > 最小): typed Load + Store replaces byte-by-byte memcpy loop.
///
/// **MVP scope (§17.6 record)**:
/// - **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
///   When `cap == 0`, `vec.ptr` is NULL, so `__landin_realloc(NULL, 0, new_bytes)`
///   is equivalent to `malloc(new_bytes)`. One Call path instead of two.
/// - **No OOM check**: `__landin_realloc` itself panics on OOM (runtime.rs:185).
/// - **PHI avoidance**: Reload `vec.ptr` in store_bb via `Projection(recv, Field(0))`.
///   Handles both growth (field updated) and no-growth (field unchanged) cases.
///   Simpler MIR — no PHI support needed.
fn lower_vec_push_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    recv_local: LocalId,
    val_local: LocalId,
) -> LocalId {
    use crate::mir::ty::ConstVal;

    let span = expr.span;
    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), span);
    let bool_ty = Ty::new(TyKind::Bool, span);

    // Stage 18.208: Extract the element type T from `Vec<T>` (or `&Vec<T>`).
    let recv_ty = cx.mir.local(recv_local).ty.clone();
    let val_ty = {
        let raw_val_ty = cx.mir.local(val_local).ty.clone();
        if matches!(raw_val_ty.kind, TyKind::Infer(_)) {
            extract_vec_element_type(&recv_ty, span)
        } else {
            raw_val_ty
        }
    };

    // The Vec.ptr field has type `*mut T`. Construct it explicitly so the
    // Projection carries the right field type (per §1.0 原則 3 显式 > 隐式).
    let elem_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(val_ty.clone()),
        ),
        span,
    );

    // Step 1: Extract vec.ptr (field 0, *mut T) via Place::Projection.
    let data_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(data_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 2: Extract vec.len (field 1, i64).
    let len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 3: Extract vec.cap (field 2, i64).
    let cap_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(cap_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(2), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 4: Compute elem_size from val type (single source of truth).
    let elem_size: i64 = compute_type_size_with_fallback(&val_ty, cx.hir, 4);
    let elem_size_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(elem_size_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(elem_size as u128),
            ty: i64_ty.clone(),
        })),
        span,
    );

    // Step 5: need_grow = BinaryOp(Ge, len, cap) — len >= cap → must grow.
    let need_grow_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(need_grow_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Ge,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Copy(Place::local(cap_local, span)),
        ),
        span,
    );

    // Step 6: SwitchInt(need_grow, [(0, store_bb)], otherwise=grow_bb).
    // need_grow is bool: 1 (true) → grow_bb; 0 (false) → store_bb.
    let grow_bb = cx.new_block();
    let store_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(need_grow_local, span)),
            targets: vec![(ConstVal::Bool(false), store_bb)],
            otherwise: grow_bb,
        },
        span,
    );

    // === grow_bb: compute new_cap (4 if cap==0, else cap*2) ===
    cx.current_block = grow_bb;

    // Step 7: is_zero = BinaryOp(Eq, cap, 0).
    let is_zero_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(is_zero_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Eq,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(0),
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );

    // Step 8: SwitchInt(is_zero, [(1, zero_cap_bb)], otherwise=nonzero_cap_bb).
    let zero_cap_bb = cx.new_block();
    let nonzero_cap_bb = cx.new_block();
    let alloc_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(is_zero_local, span)),
            targets: vec![(ConstVal::Bool(true), zero_cap_bb)],
            otherwise: nonzero_cap_bb,
        },
        span,
    );

    // === zero_cap_bb: new_cap = 4 (initial capacity) ===
    cx.current_block = zero_cap_bb;
    // Stage 18.229: new_cap_local is Mutable because it's assigned in both
    // zero_cap_bb (= 4) and nonzero_cap_bb (= cap * 2). Without Mutable,
    // the borrowck flags the second assignment as "assign twice to immutable".
    // Per §1.0 原則 6 (通解>特例): same pattern as if/else result locals
    // (control_flow.rs:31 uses new_local_with_mut for PHI-like assignments).
    let new_cap_local = cx.mir.new_local_with_mut(
        i64_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(4),
            ty: i64_ty.clone(),
        })),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(alloc_bb), span);

    // === nonzero_cap_bb: new_cap = cap + cap (2x growth) ===
    cx.current_block = nonzero_cap_bb;
    let doubled_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(doubled_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Copy(Place::local(cap_local, span)),
        ),
        span,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Copy(Place::local(doubled_local, span))),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(alloc_bb), span);

    // === alloc_bb: realloc + update vec.ptr + vec.cap ===
    cx.current_block = alloc_bb;

    // new_bytes = new_cap * elem_size
    let new_bytes_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(new_bytes_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Mul,
            Operand::Copy(Place::local(new_cap_local, span)),
            Operand::Copy(Place::local(elem_size_local, span)),
        ),
        span,
    );

    // old_bytes = cap * elem_size (passed to __landin_realloc for diagnostics)
    let old_bytes_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(old_bytes_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Mul,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Copy(Place::local(elem_size_local, span)),
        ),
        span,
    );

    // Call __landin_realloc(data_ptr, old_bytes, new_bytes) → new_ptr.
    // libc realloc(NULL, size) == malloc(size), so this handles cap==0 case.
    // Per §16.5 (06-mir.md): __landin_realloc is a primitive C helper (not migrated).
    let realloc_def_id = crate::hir::DefId::new(u32::MAX - 102);
    let realloc_fn_ty = Ty::new(
        TyKind::FnDef(realloc_def_id, std::vec::Vec::new().into()),
        span,
    );
    let realloc_fn_local = cx.mir.new_local(realloc_fn_ty, None, span);
    let new_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    let realloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(realloc_fn_local, span)),
            args: vec![
                Operand::Copy(Place::local(data_ptr_local, span)),
                Operand::Copy(Place::local(old_bytes_local, span)),
                Operand::Copy(Place::local(new_bytes_local, span)),
            ],
            destination: Place::local(new_ptr_local, span),
            target: Some(realloc_cont),
            dyn_trait_call: None,
        },
        realloc_cont,
    );

    // Store new_ptr to vec.ptr (field 0).
    // Per §10 DRY: reuses StatementKind::Store + Field projection pattern.
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_ptr_local, span)),
                val_ty: elem_ptr_ty.clone(),
            },
            span,
        },
        span,
    );

    // Store new_cap to vec.cap (field 2).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(2), i64_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_cap_local, span)),
                val_ty: i64_ty.clone(),
            },
            span,
        },
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(store_bb), span);

    // === store_bb: store val + increment len ===
    cx.current_block = store_bb;

    // Reload vec.ptr (handles both growth and no-growth cases via Field projection).
    let current_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(current_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // elem_ptr = GetElementPtr(current_ptr, [len], *mut T)
    let elem_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(elem_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(current_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(len_local, span))],
            result_ty: elem_ptr_ty.clone(),
        },
        span,
    );

    // *elem_ptr = val — Store through the pointer via Projection(Deref).
    // Reuses the Box::new pattern (Stage 14.66 Deref + RawPtr handling).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(elem_ptr_local, span)),
                        ProjectionElem::Deref,
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(val_local, span)),
                val_ty: val_ty.clone(),
            },
            span,
        },
        span,
    );

    // new_len = BinaryOp(Add, len, 1)
    let new_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(new_len_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );

    // Store new_len to vec.len (field 1).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(1), i64_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_len_local, span)),
                val_ty: i64_ty.clone(),
            },
            span,
        },
        span,
    );

    // Return unit.
    let unit_ty = Ty::new(TyKind::Tuple(vec![]), span);
    let dest = cx.mir.new_local(unit_ty, None, span);
    let after = cx.new_block();
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(after),
            span,
        },
        after,
    );
    dest
}

/// Stage 18.230 (v0.2.5f): Lower `String::push_str(src: &str)` via MIR intrinsics.
///
/// Replaces the previous `__landin_string_push_str` C helper Call with a pure
/// MIR sequence using MIR intrinsic ops (Load + GetElementPtr + Store) plus
/// `__landin_realloc` (primitive, per §16.5) and `__landin_memcpy` (primitive,
/// per §16.5). The growth while loop is expressed via a MIR back-edge.
///
/// Generates MIR for (10 basic blocks):
///   1. bb0: Extract str fields + src fields; compute new_len; need_grow check
///   2. grow_init_bb: is_zero = (cap == 0); SwitchInt
///   3. zero_cap_bb: new_cap = 4; goto grow_loop_bb
///   4. nonzero_cap_bb: new_cap = cap; goto grow_loop_bb
///   5. grow_loop_bb: cond = (new_cap < new_len); SwitchInt ← BACK-EDGE TARGET
///   6. grow_body_bb: new_cap = new_cap + new_cap; goto grow_loop_bb ← BACK-EDGE
///   7. alloc_bb: realloc + Store str.ptr + Store str.cap; goto copy_bb
///   8. copy_bb: reload str.ptr; GEP(dest, len); Call memcpy; Store str.len
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all String::push_str calls.
/// Per §1.0 原則 4 (报错>静默): OOM panics via `__landin_realloc` (visible).
/// Per §10 DRY: reuses `__landin_realloc` + `__landin_memcpy` (primitive helpers),
/// `MemoryEmitter` methods, `push_statement` API (Stage 18.229).
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR.
/// Per §12 (最优 > 最小): typed Load + Store + memcpy replaces byte-by-byte C loop.
///
/// **MVP scope (§17.6 record)**:
/// - **Always realloc**: libc `realloc(NULL, size) == malloc(size)` per C standard.
/// - **No OOM check**: `__landin_realloc` itself panics on OOM (runtime.rs:185).
/// - **PHI avoidance**: Reload `str.ptr` in copy_bb via `Projection(recv, Field(0))`.
/// - **memcpy via C helper**: `__landin_memcpy` is a primitive C helper (per §16.5).
/// - **Growth while loop**: Expressed via MIR back-edge (first MIR loop in an intrinsic).
fn lower_string_push_str_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    recv_local: LocalId,
    src_local: LocalId,
) -> LocalId {
    use crate::mir::ty::ConstVal;

    let span = expr.span;
    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), span);
    let bool_ty = Ty::new(TyKind::Bool, span);
    let u8_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::U8), span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(crate::mir::ty::Mutability::Mutable, Box::new(u8_ty.clone())),
        span,
    );

    // Step 1: Extract str.ptr (field 0, *mut u8).
    let data_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(data_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 2: Extract str.len (field 1, i64).
    let len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 3: Extract str.cap (field 2, i64).
    let cap_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(cap_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(2), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 4: Extract src.ptr (field 0) from &str fat pointer.
    let src_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(src_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 5: Extract src.len (field 1) from &str fat pointer.
    let src_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(src_len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(src_local, span)),
                ProjectionElem::Field(FieldId(1), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 6: new_len = len + src_len.
    let new_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(new_len_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(len_local, span)),
            Operand::Copy(Place::local(src_len_local, span)),
        ),
        span,
    );

    // Step 7: need_grow = (new_len > cap).
    let need_grow_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(need_grow_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Gt,
            Operand::Copy(Place::local(new_len_local, span)),
            Operand::Copy(Place::local(cap_local, span)),
        ),
        span,
    );

    // Step 8: SwitchInt(need_grow, [(0, copy_bb)], otherwise=grow_init_bb).
    let grow_init_bb = cx.new_block();
    let copy_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(need_grow_local, span)),
            targets: vec![(ConstVal::Bool(false), copy_bb)],
            otherwise: grow_init_bb,
        },
        span,
    );

    // === grow_init_bb: is_zero = (cap == 0) ===
    cx.current_block = grow_init_bb;
    let is_zero_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(is_zero_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Eq,
            Operand::Copy(Place::local(cap_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(0),
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );

    let zero_cap_bb = cx.new_block();
    let nonzero_cap_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(is_zero_local, span)),
            targets: vec![(ConstVal::Bool(true), zero_cap_bb)],
            otherwise: nonzero_cap_bb,
        },
        span,
    );

    // === zero_cap_bb: new_cap = 4 (initial capacity) ===
    cx.current_block = zero_cap_bb;
    // Mutable because assigned in zero_cap_bb, nonzero_cap_bb, and grow_body_bb.
    let new_cap_local = cx.mir.new_local_with_mut(
        i64_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(4),
            ty: i64_ty.clone(),
        })),
        span,
    );
    let grow_loop_bb = cx.new_block();
    cx.terminate_kind_span(TerminatorKind::Goto(grow_loop_bb), span);

    // === nonzero_cap_bb: new_cap = cap ===
    cx.current_block = nonzero_cap_bb;
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Copy(Place::local(cap_local, span))),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(grow_loop_bb), span);

    // === grow_loop_bb: while (new_cap < new_len) new_cap *= 2  ← BACK-EDGE TARGET ===
    cx.current_block = grow_loop_bb;
    let loop_cond_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(loop_cond_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Lt,
            Operand::Copy(Place::local(new_cap_local, span)),
            Operand::Copy(Place::local(new_len_local, span)),
        ),
        span,
    );
    let alloc_bb = cx.new_block();
    let grow_body_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(loop_cond_local, span)),
            targets: vec![(ConstVal::Bool(false), alloc_bb)],
            otherwise: grow_body_bb,
        },
        span,
    );

    // === grow_body_bb: new_cap = new_cap + new_cap (2x)  ← BACK-EDGE ===
    cx.current_block = grow_body_bb;
    let doubled_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(doubled_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(new_cap_local, span)),
            Operand::Copy(Place::local(new_cap_local, span)),
        ),
        span,
    );
    cx.push_assign(
        Place::local(new_cap_local, span),
        Rvalue::Use(Operand::Copy(Place::local(doubled_local, span))),
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(grow_loop_bb), span);

    // === alloc_bb: realloc + update str.ptr + str.cap ===
    cx.current_block = alloc_bb;

    // new_bytes = new_cap (String stores bytes, elem_size = 1).
    // old_bytes = cap.
    // Call __landin_realloc(data_ptr, old_bytes, new_bytes) → new_ptr.
    let realloc_def_id = crate::hir::DefId::new(u32::MAX - 102);
    let realloc_fn_ty = Ty::new(
        TyKind::FnDef(realloc_def_id, std::vec::Vec::new().into()),
        span,
    );
    let realloc_fn_local = cx.mir.new_local(realloc_fn_ty, None, span);
    let new_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    let realloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(realloc_fn_local, span)),
            args: vec![
                Operand::Copy(Place::local(data_ptr_local, span)),
                Operand::Copy(Place::local(cap_local, span)),
                Operand::Copy(Place::local(new_cap_local, span)),
            ],
            destination: Place::local(new_ptr_local, span),
            target: Some(realloc_cont),
            dyn_trait_call: None,
        },
        realloc_cont,
    );

    // Store new_ptr to str.ptr (field 0).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_ptr_local, span)),
                val_ty: u8_ptr_ty.clone(),
            },
            span,
        },
        span,
    );

    // Store new_cap to str.cap (field 2).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(2), i64_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_cap_local, span)),
                val_ty: i64_ty.clone(),
            },
            span,
        },
        span,
    );
    cx.terminate_kind_span(TerminatorKind::Goto(copy_bb), span);

    // === copy_bb: reload str.ptr + GEP(dest, len) + memcpy + update len ===
    cx.current_block = copy_bb;

    // Reload str.ptr (handles both growth and no-growth cases).
    let current_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(current_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // dest_ptr = GetElementPtr(current_ptr, [len], *mut u8).
    let dest_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(dest_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(current_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(len_local, span))],
            result_ty: u8_ptr_ty.clone(),
        },
        span,
    );

    // Call __landin_memcpy(dest_ptr, src_ptr, src_len).
    // Per §16.5: __landin_memcpy is a primitive C helper (not migrated).
    let memcpy_def_id = crate::hir::DefId::new(u32::MAX - 101);
    let memcpy_fn_ty = Ty::new(
        TyKind::FnDef(memcpy_def_id, std::vec::Vec::new().into()),
        span,
    );
    let memcpy_fn_local = cx.mir.new_local(memcpy_fn_ty, None, span);
    let memcpy_dest = cx
        .mir
        .new_local(Ty::new(TyKind::Tuple(vec![]), span), None, span);
    let memcpy_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(memcpy_fn_local, span)),
            args: vec![
                Operand::Copy(Place::local(dest_ptr_local, span)),
                Operand::Copy(Place::local(src_ptr_local, span)),
                Operand::Copy(Place::local(src_len_local, span)),
            ],
            destination: Place::local(memcpy_dest, span),
            target: Some(memcpy_cont),
            dyn_trait_call: None,
        },
        memcpy_cont,
    );

    // Store new_len to str.len (field 1).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(recv_local, span)),
                        ProjectionElem::Field(FieldId(1), i64_ty.clone()),
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(new_len_local, span)),
                val_ty: i64_ty.clone(),
            },
            span,
        },
        span,
    );

    // Return unit.
    let unit_ty = Ty::new(TyKind::Tuple(vec![]), span);
    let dest = cx.mir.new_local(unit_ty, None, span);
    let after = cx.new_block();
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(after),
            span,
        },
        after,
    );
    dest
}

/// Stage 18.208 (TD-VEC-GET-TYPE-INFERENCE fix): Extract the element type
/// from a `Vec<T>` receiver type.
///
/// Given the receiver's type (e.g., `Adt(Vec_def_id, [Point])`), returns
/// `substs[0]` (the element type `T`). If the receiver type is not a
/// generic Adt with at least one substitution, falls back to `i32`
/// (the canonical `Vec<i32>` case).
///
/// Per §1.0 原則 6 (通解>特例): one extraction path for all Vec<T> types.
/// Per §12 (最优 > 最小): root-cause fix — read substs[0] from the type.
/// Per §10 (DRY): single helper, used by `lower_vec_get_intrinsic`.
///
/// Stage 18.208 addendum: The receiver type may be wrapped in a `Ref`
/// (e.g., `&Vec<T>` for by-ref method calls). We unwrap one level of Ref.
fn extract_vec_element_type(recv_ty: &Ty, span: Span) -> Ty {
    // Unwrap one level of Ref (&Vec<T> → Vec<T>).
    let inner_ty = match &recv_ty.kind {
        TyKind::Ref(_, _, inner) => inner.as_ref(),
        _ => recv_ty,
    };
    match &inner_ty.kind {
        TyKind::Adt(_def_id, substs) => {
            if let Some(elem_ty) = substs.first() {
                elem_ty.clone()
            } else {
                // substs is empty — fallback to i32 (canonical Vec<i32> case).
                Ty::new(TyKind::Int(crate::ast::IntTy::I32), span)
            }
        }
        _ => Ty::new(TyKind::Int(crate::ast::IntTy::I32), span),
    }
}

/// Stage 18.228 (v0.2.5d): Lower `Vec::get(index) -> T` via MIR intrinsics.
///
/// Replaces the previous `__landin_vec_get` C helper Call with a pure MIR
/// sequence using the new MIR intrinsic ops (Load + GetElementPtr) added
/// in Stage 18.226 and codegen-enabled in Stage 18.227.
///
/// Generates MIR for:
///   1. Extract `vec.ptr` (field 0, `*mut T`) via `Place::Projection(Field(0))`
///   2. Extract `vec.len` (field 1, `i64`) via `Place::Projection(Field(1))`
///   3. Cast `index` to `i64` (if needed)
///   4. Compute `cond = (idx < len)` via `BinaryOp(Lt)`
///   5. `Assert(cond, expected=true, target=ok_bb, msg=BoundsCheck)` —
///      branches to panic block (calls `__landin_panic_bounds_check`) on
///      OOB. Reuses existing Assert infra (Stage 3.24).
///   6. ok_bb: `elem_ptr = GetElementPtr(data_ptr, [idx])` (Stage 18.226)
///   7. `dest = Load(elem_ptr, T)` (Stage 18.226) — typed load, no memcpy
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all Vec<T> types —
/// the element type T flows through `extract_vec_element_type` (Stage 18.208).
/// Per §1.0 原則 4 (报错>静默): bounds check via `Assert(BoundsCheck)` —
/// OOB panics with `__landin_panic_bounds_check` (visible, not silent).
/// Per §10 DRY: reuses `extract_vec_element_type`, `MemoryEmitter` methods,
/// `AssertMessage::BoundsCheck` — no new infrastructure.
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only
/// translates MIR (no C helper Call).
/// Per §12 (最优 > 最小): typed `Load` replaces byte-by-byte `memcpy` loop.
///
/// **MVP scope (§17.6 record)**: only checks `idx < len` (upper bound).
/// The `idx < 0` check is deferred — Landin's `Vec::get` index is `usize`-like
/// in idiomatic usage (negative indices impossible in Rust convention).
/// Recorded in task-review §2.5; will be revisited if a test exercises
/// negative index behavior.
fn lower_vec_get_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    recv_local: LocalId,
    idx_local: LocalId,
) -> LocalId {
    let span = expr.span;
    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), span);

    // Stage 18.208: Extract the element type T from `Vec<T>` (or `&Vec<T>`).
    // Per §10 DRY: single source of truth for element-type extraction.
    let recv_ty = cx.mir.local(recv_local).ty.clone();
    let elem_ty = extract_vec_element_type(&recv_ty, span);

    // The Vec.ptr field has type `*mut T`. Construct it explicitly so the
    // Projection carries the right field type (per §1.0 原則 3 显式 > 隐式).
    let elem_ptr_ty = Ty::new(
        TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(elem_ty.clone()),
        ),
        span,
    );

    // Step 1: Extract `vec.ptr` (field 0, `*mut T`) via Place::Projection.
    // Reuses the AdtLayout system (Stage 18.200 lower_vec_push_intrinsic pattern).
    let data_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(data_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(0), elem_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 2: Extract `vec.len` (field 1, `i64`) via Place::Projection.
    let len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(recv_local, span)),
                ProjectionElem::Field(FieldId(1), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 3: Cast `index` to `i64` (if needed). Numeric cast handles
    // i32→i64, u32→i64, etc. Per §1.0 原則 6 (通解>特例): one cast path
    // for all integer index types.
    let idx_i64 = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(idx_i64, span),
        Rvalue::Cast(
            crate::mir::place::CastKind::Numeric,
            Operand::Copy(Place::local(idx_local, span)),
            i64_ty.clone(),
        ),
        span,
    );

    // Step 4: Compute `cond = (idx < len)` via BinaryOp(Lt).
    // Per §1.0 原則 6 (通解>特例): one BinaryOp for all bounds checks.
    let cond_local = cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
    cx.push_assign(
        Place::local(cond_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Lt,
            Operand::Copy(Place::local(idx_i64, span)),
            Operand::Copy(Place::local(len_local, span)),
        ),
        span,
    );

    // Step 5: `Assert(cond, expected=true, target=ok_bb, msg=BoundsCheck)`.
    // Reuses existing Assert infra (Stage 3.24) + `__landin_panic_bounds_check`
    // C helper. On OOB, codegen emits a panic block that calls the helper
    // and emits `unreachable`.
    //
    // Per §1.0 原則 4 (报错>静默): OOB panics visibly, not silent skip.
    // Per §10 DRY: reuses AssertMessage::BoundsCheck (no new panic path).
    let ok_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::Assert {
            cond: Operand::Copy(Place::local(cond_local, span)),
            expected: true,
            target: ok_bb,
            msg: crate::mir::body::AssertMessage::BoundsCheck,
        },
        span,
    );
    cx.current_block = ok_bb;

    // Step 6: `elem_ptr = GetElementPtr(data_ptr, [idx])` (Stage 18.226).
    // Computes `&data_ptr[idx]` as `*mut T`. Codegen (Stage 18.227) emits
    // `getelementptr inbounds` via `MemoryEmitter::emit_gep_index_ptr`.
    //
    // Per §1.0 原則 6 (通解>特例): one GEP for all element types — the
    // element type is encoded in the LLVM IR GEP instruction's source type.
    // Per §16.2 (06-mir.md): MIR intrinsic ops design.
    let elem_ptr_local = cx.mir.new_local(elem_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(elem_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(data_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(idx_i64, span))],
            result_ty: elem_ptr_ty.clone(),
        },
        span,
    );

    // Step 7: `dest = Load(elem_ptr, T)` (Stage 18.226). Typed load — no
    // byte-by-byte memcpy needed (unlike the C helper). Codegen (Stage
    // 18.227) emits `load T, ptr %elem_ptr` via `MemoryEmitter::emit_load`.
    //
    // Per §1.0 原則 6 (通解>特例): one Load for all element types.
    // Per §12 (最优 > 最小): typed load, not memcpy loop.
    let dest = cx.mir.new_local(elem_ty.clone(), None, span);
    let after = cx.new_block();
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::Load(Operand::Copy(Place::local(elem_ptr_local, span)), elem_ty),
        span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(after),
            span,
        },
        after,
    );
    dest
}

/// Stage 18.231 (v0.2.5g): Lower `format!("x={}", x, ...)` via MIR intrinsics.
///
/// Replaces the previous `__landin_format_variadic` C helper Call with a pure
/// MIR sequence that walks the format string byte-by-byte and builds the
/// output String using MIR intrinsic ops (Load + GetElementPtr + Store) plus
/// `__landin_alloc` + `__landin_i64_to_str` (primitive, per §16.5).
///
/// Generates MIR for:
/// 1. Allocate a fixed-size output buffer (4096 bytes, matching C helper MVP)
/// 2. Extract fmt.ptr and fmt.len from the &str format string (arg[0])
/// 3. Initialize: out_len = 0, fmt_idx = 0, arg_idx = 1
/// 4. Loop (fmt_loop_bb): while (fmt_idx < fmt_len)
///    - Load byte at fmt_ptr[fmt_idx] via GEP + Load
///    - If byte == '{': dispatch on arg_idx (SwitchInt per arg)
///      - Call __landin_i64_to_str, advance fmt_idx by 2, arg_idx by 1
///    - Else: Store byte to out_ptr[out_len], out_len++, fmt_idx++
/// 5. Construct String { ptr: out_ptr, len: out_len, cap: out_len + 1 }
/// 6. Return the output String
///
/// Per §1.0 原則 6 (通解>特解): one MIR sequence for all format! calls.
/// Per §1.0 原則 4 (报错>静默): OOM panics via `__landin_alloc` (visible).
/// Per §10 DRY: reuses `__landin_alloc` + `__landin_i64_to_str` (primitives),
/// `MemoryEmitter` methods, `push_statement` API (Stage 18.229).
/// Per §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR.
/// Per §12 (最优 > 最小): MIR-level format walker replaces C's snprintf + buffer walk.
///
/// **MVP scope (§17.6 record)**:
/// - **Fixed-size buffer (4096 bytes)**: matches C helper MVP (runtime.rs:351).
///   Dynamic growth deferred — same limitation as the C helper.
/// - **i64 args only**: The C helper supports &str args via `%s`, but the MIR
///   migration supports i64 only (the most common case). All format args are
///   cast to i64 and formatted via `__landin_i64_to_str`. &str arg support
///   deferred to v0.3 (requires fat pointer handling in the format walker).
/// - **No arg_types array**: Type dispatch inferred from MIR (all i64).
fn lower_format_variadic_intrinsic(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    arg_locals: &[LocalId],
) -> LocalId {
    use crate::mir::ty::ConstVal;

    let span = expr.span;
    let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), span);
    let u8_ty = Ty::new(TyKind::Uint(crate::ast::UintTy::U8), span);
    let u8_ptr_ty = Ty::new(
        TyKind::RawPtr(crate::mir::ty::Mutability::Mutable, Box::new(u8_ty.clone())),
        span,
    );
    let bool_ty = Ty::new(TyKind::Bool, span);

    // arg_locals[0] = format string (&str fat pointer)
    // arg_locals[1..] = format arguments (all cast to i64 for MVP)
    let fmt_local = arg_locals[0];

    // Look up String struct's DefId from HIR by name.
    let string_def_id = {
        let mut found = None;
        if let Some(hir) = cx.hir {
            let string_spur = cx.interner.get("String");
            if let Some(target_name) = string_spur {
                for (def_id, owner) in &hir.owners {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) = owner {
                        if s.ident.name == target_name {
                            found = Some(*def_id);
                            break;
                        }
                    }
                }
            }
        }
        found
    };

    let string_ty = if let Some(did) = string_def_id {
        Ty::new(TyKind::Adt(did, std::vec::Vec::new().into()), span)
    } else {
        Ty::new(TyKind::Error, span)
    };

    // Step 1: Allocate a fixed-size output buffer (4096 bytes).
    // Matches C helper MVP (runtime.rs:351: char buffer[4096]).
    let buf_size: i64 = 4096;
    let buf_size_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(buf_size_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(buf_size as u128),
            ty: i64_ty.clone(),
        })),
        span,
    );

    let alloc_def_id = crate::hir::DefId::new(u32::MAX - 100);
    let alloc_fn_ty = Ty::new(
        TyKind::FnDef(alloc_def_id, std::vec::Vec::new().into()),
        span,
    );
    let alloc_fn_local = cx.mir.new_local(alloc_fn_ty, None, span);
    let out_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    let alloc_cont = cx.new_block();
    cx.terminate_kind_and_goto(
        TerminatorKind::Call {
            func: Operand::Move(Place::local(alloc_fn_local, span)),
            args: vec![Operand::Copy(Place::local(buf_size_local, span))],
            destination: Place::local(out_ptr_local, span),
            target: Some(alloc_cont),
            dyn_trait_call: None,
        },
        alloc_cont,
    );

    // Step 2: Extract fmt.ptr (field 0) from &str fat pointer.
    let fmt_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(fmt_ptr_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(fmt_local, span)),
                ProjectionElem::Field(FieldId(0), u8_ptr_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 3: Extract fmt.len (field 1) from &str fat pointer.
    let fmt_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(fmt_len_local, span),
        Rvalue::Use(Operand::Copy(Place {
            kind: PlaceKind::Projection(
                Box::new(Place::local(fmt_local, span)),
                ProjectionElem::Field(FieldId(1), i64_ty.clone()),
            ),
            span,
        })),
        span,
    );

    // Step 4: Initialize loop variables.
    // out_len = 0 (current write position in output buffer)
    // fmt_idx = 0 (current read position in format string)
    // arg_idx = 1 (next arg to consume, 1-based; arg_locals[0] is fmt)
    let out_len_local = cx.mir.new_local_with_mut(
        i64_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(out_len_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(0),
            ty: i64_ty.clone(),
        })),
        span,
    );

    let fmt_idx_local = cx.mir.new_local_with_mut(
        i64_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(fmt_idx_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(0),
            ty: i64_ty.clone(),
        })),
        span,
    );

    let arg_idx_local = cx.mir.new_local_with_mut(
        i64_ty.clone(),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );
    cx.push_assign(
        Place::local(arg_idx_local, span),
        Rvalue::Use(Operand::Constant(Const {
            val: ConstVal::Int(1),
            ty: i64_ty.clone(),
        })),
        span,
    );

    // Step 5: fmt_loop_bb: while (fmt_idx < fmt_len)  ← BACK-EDGE TARGET
    let fmt_loop_bb = cx.new_block();
    cx.terminate_kind_span(TerminatorKind::Goto(fmt_loop_bb), span);
    cx.current_block = fmt_loop_bb;

    // Loop condition: fmt_idx < fmt_len
    let loop_cond_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(loop_cond_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Lt,
            Operand::Copy(Place::local(fmt_idx_local, span)),
            Operand::Copy(Place::local(fmt_len_local, span)),
        ),
        span,
    );

    let loop_body_bb = cx.new_block();
    let loop_exit_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(loop_cond_local, span)),
            targets: vec![(ConstVal::Bool(false), loop_exit_bb)],
            otherwise: loop_body_bb,
        },
        span,
    );

    // === loop_body_bb: load byte at fmt_ptr[fmt_idx] and dispatch ===
    cx.current_block = loop_body_bb;

    // Compute byte address: fmt_ptr + fmt_idx via GEP.
    let byte_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(byte_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(fmt_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(fmt_idx_local, span))],
            result_ty: u8_ptr_ty.clone(),
        },
        span,
    );

    // Load the byte.
    let byte_local = cx.mir.new_local(u8_ty.clone(), None, span);
    cx.push_assign(
        Place::local(byte_local, span),
        Rvalue::Load(
            Operand::Copy(Place::local(byte_ptr_local, span)),
            u8_ty.clone(),
        ),
        span,
    );

    // Cast byte to i64 for comparison (BinaryOp needs matching types).
    let byte_i64_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(byte_i64_local, span),
        Rvalue::Cast(
            crate::mir::place::CastKind::Numeric,
            Operand::Copy(Place::local(byte_local, span)),
            i64_ty.clone(),
        ),
        span,
    );

    // Check if byte == '{' (ASCII 123).
    let is_open_brace_local = cx.mir.new_local(bool_ty.clone(), None, span);
    cx.push_assign(
        Place::local(is_open_brace_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Eq,
            Operand::Copy(Place::local(byte_i64_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(123), // '{'
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );

    let placeholder_bb = cx.new_block();
    let literal_bb = cx.new_block();
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(is_open_brace_local, span)),
            targets: vec![(ConstVal::Bool(true), placeholder_bb)],
            otherwise: literal_bb,
        },
        span,
    );

    // === placeholder_bb: handle {} placeholder ===
    // For MVP: assume it's "{}" (open + close brace). We don't check the next
    // byte — the format string is validated at parse time. Just consume 2 bytes
    // and format the next arg as i64.
    cx.current_block = placeholder_bb;

    // Get the arg value at arg_idx (1-based → arg_locals[arg_idx - 1 + 1] = arg_locals[arg_idx]).
    // For MVP, we cast all args to i64.
    // Since MIR can't dynamically index arg_locals, we emit a SwitchInt to
    // select the arg based on arg_idx. For MVP with ≤4 args, this is feasible.
    //
    // Actually, for MVP, let's handle the common case: format! with args known
    // at compile time. We emit a specialized block per arg position.
    //
    // Simpler MVP approach: since arg_locals is known at lower time, we can
    // emit a switch on arg_idx_local with one case per known arg.
    let mut arg_switch_targets: Vec<(ConstVal, BasicBlockId)> = Vec::new();
    let mut arg_format_blocks: Vec<BasicBlockId> = Vec::new();
    for (i, _arg_local) in arg_locals.iter().enumerate().skip(1) {
        let arg_block = cx.new_block();
        arg_switch_targets.push((ConstVal::Int(i as u128), arg_block));
        arg_format_blocks.push(arg_block);
    }
    let no_arg_bb = cx.new_block(); // arg_idx > n_args → no more args
    cx.terminate_kind_span(
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(arg_idx_local, span)),
            targets: arg_switch_targets,
            otherwise: no_arg_bb,
        },
        span,
    );

    // Emit per-arg format blocks.
    for (i, arg_local) in arg_locals.iter().enumerate().skip(1) {
        let arg_block = arg_format_blocks[i - 1];
        cx.current_block = arg_block;

        // Cast arg to i64.
        let arg_i64_local = cx.mir.new_local(i64_ty.clone(), None, span);
        cx.push_assign(
            Place::local(arg_i64_local, span),
            Rvalue::Cast(
                crate::mir::place::CastKind::Numeric,
                Operand::Copy(Place::local(*arg_local, span)),
                i64_ty.clone(),
            ),
            span,
        );

        // Compute dest pointer: out_ptr + out_len via GEP.
        let dest_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
        cx.push_assign(
            Place::local(dest_ptr_local, span),
            Rvalue::GetElementPtr {
                base: Operand::Copy(Place::local(out_ptr_local, span)),
                indices: vec![Operand::Copy(Place::local(out_len_local, span))],
                result_ty: u8_ptr_ty.clone(),
            },
            span,
        );

        // Compute remaining capacity: buf_size - out_len.
        let remaining_local = cx.mir.new_local(i64_ty.clone(), None, span);
        cx.push_assign(
            Place::local(remaining_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Sub,
                Operand::Copy(Place::local(buf_size_local, span)),
                Operand::Copy(Place::local(out_len_local, span)),
            ),
            span,
        );

        // Call __landin_i64_to_str(dest_ptr, remaining, arg_i64) → written_len.
        let i64_to_str_def_id = crate::hir::DefId::new(u32::MAX - 107);
        let i64_to_str_fn_ty = Ty::new(
            TyKind::FnDef(i64_to_str_def_id, std::vec::Vec::new().into()),
            span,
        );
        let i64_to_str_fn_local = cx.mir.new_local(i64_to_str_fn_ty, None, span);
        let written_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
        let i64_to_str_cont = cx.new_block();
        cx.terminate_kind_and_goto(
            TerminatorKind::Call {
                func: Operand::Move(Place::local(i64_to_str_fn_local, span)),
                args: vec![
                    Operand::Copy(Place::local(dest_ptr_local, span)),
                    Operand::Copy(Place::local(remaining_local, span)),
                    Operand::Copy(Place::local(arg_i64_local, span)),
                ],
                destination: Place::local(written_len_local, span),
                target: Some(i64_to_str_cont),
                dyn_trait_call: None,
            },
            i64_to_str_cont,
        );

        // out_len += written_len
        let new_out_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
        cx.push_assign(
            Place::local(new_out_len_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Add,
                Operand::Copy(Place::local(out_len_local, span)),
                Operand::Copy(Place::local(written_len_local, span)),
            ),
            span,
        );
        cx.push_assign(
            Place::local(out_len_local, span),
            Rvalue::Use(Operand::Copy(Place::local(new_out_len_local, span))),
            span,
        );

        // fmt_idx += 2 (skip "{}")
        let new_fmt_idx_local = cx.mir.new_local(i64_ty.clone(), None, span);
        cx.push_assign(
            Place::local(new_fmt_idx_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Add,
                Operand::Copy(Place::local(fmt_idx_local, span)),
                Operand::Constant(Const {
                    val: ConstVal::Int(2),
                    ty: i64_ty.clone(),
                }),
            ),
            span,
        );
        cx.push_assign(
            Place::local(fmt_idx_local, span),
            Rvalue::Use(Operand::Copy(Place::local(new_fmt_idx_local, span))),
            span,
        );

        // arg_idx += 1
        let new_arg_idx_local = cx.mir.new_local(i64_ty.clone(), None, span);
        cx.push_assign(
            Place::local(new_arg_idx_local, span),
            Rvalue::BinaryOp(
                crate::mir::place::BinOp::Add,
                Operand::Copy(Place::local(arg_idx_local, span)),
                Operand::Constant(Const {
                    val: ConstVal::Int(1),
                    ty: i64_ty.clone(),
                }),
            ),
            span,
        );
        cx.push_assign(
            Place::local(arg_idx_local, span),
            Rvalue::Use(Operand::Copy(Place::local(new_arg_idx_local, span))),
            span,
        );

        // Back-edge to loop.
        cx.terminate_kind_span(TerminatorKind::Goto(fmt_loop_bb), span);
    }

    // === no_arg_bb: no more args to format — treat {} as literal ===
    cx.current_block = no_arg_bb;
    // Fall through to literal_bb (store '{' as literal byte).
    cx.terminate_kind_span(TerminatorKind::Goto(literal_bb), span);

    // === literal_bb: store byte to out_ptr[out_len], out_len++, fmt_idx++ ===
    cx.current_block = literal_bb;

    // Compute dest pointer: out_ptr + out_len via GEP.
    let lit_dest_ptr_local = cx.mir.new_local(u8_ptr_ty.clone(), None, span);
    cx.push_assign(
        Place::local(lit_dest_ptr_local, span),
        Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(out_ptr_local, span)),
            indices: vec![Operand::Copy(Place::local(out_len_local, span))],
            result_ty: u8_ptr_ty.clone(),
        },
        span,
    );

    // Store the byte via Projection(Deref).
    cx.push_statement(
        Statement {
            kind: StatementKind::Store {
                ptr: Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(lit_dest_ptr_local, span)),
                        ProjectionElem::Deref,
                    ),
                    span,
                },
                val: Operand::Copy(Place::local(byte_local, span)),
                val_ty: u8_ty.clone(),
            },
            span,
        },
        span,
    );

    // out_len += 1
    let lit_new_out_len_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(lit_new_out_len_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(out_len_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );
    cx.push_assign(
        Place::local(out_len_local, span),
        Rvalue::Use(Operand::Copy(Place::local(lit_new_out_len_local, span))),
        span,
    );

    // fmt_idx += 1
    let lit_new_fmt_idx_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(lit_new_fmt_idx_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(fmt_idx_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );
    cx.push_assign(
        Place::local(fmt_idx_local, span),
        Rvalue::Use(Operand::Copy(Place::local(lit_new_fmt_idx_local, span))),
        span,
    );

    // Back-edge to loop.
    cx.terminate_kind_span(TerminatorKind::Goto(fmt_loop_bb), span);

    // === loop_exit_bb: construct String { ptr, len, cap } ===
    cx.current_block = loop_exit_bb;

    // Stage 18.231: cap = out_len + 1 (matches C helper's `result_len + 1`
    // convention — the +1 accounts for the null terminator byte).
    let cap_val_local = cx.mir.new_local(i64_ty.clone(), None, span);
    cx.push_assign(
        Place::local(cap_val_local, span),
        Rvalue::BinaryOp(
            crate::mir::place::BinOp::Add,
            Operand::Copy(Place::local(out_len_local, span)),
            Operand::Constant(Const {
                val: ConstVal::Int(1),
                ty: i64_ty.clone(),
            }),
        ),
        span,
    );

    // Construct the String struct via Aggregate.
    // Stage 18.231: field_tys are [u8_ptr_ty, i64_ty, i64_ty] (ptr, len, cap).
    let dest = cx.mir.new_local(string_ty.clone(), None, span);
    let after = cx.new_block();
    cx.push_assign(
        Place::local(dest, span),
        Rvalue::Aggregate(
            crate::mir::place::AggregateKind::Adt(
                string_def_id.unwrap_or(crate::hir::DefId::new(0)),
                0,
                std::vec::Vec::new().into(),
                vec![u8_ptr_ty.clone(), i64_ty.clone(), i64_ty.clone()],
            ),
            vec![
                Operand::Copy(Place::local(out_ptr_local, span)),
                Operand::Copy(Place::local(out_len_local, span)),
                Operand::Copy(Place::local(cap_val_local, span)),
            ],
        ),
        span,
    );
    cx.terminate_and_goto(
        Terminator {
            kind: TerminatorKind::Goto(after),
            span,
        },
        after,
    );
    dest
}
