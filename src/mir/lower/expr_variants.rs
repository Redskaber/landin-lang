//! Expression variant lowering: Path, Call, For.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.133):
//! Extracted from `expr_operand.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains the 3 largest HirExprKind match arms (Path, Call, For),
//! extracted as functions.
//!
//! Stage 18.309 update: the 4th variant (`MethodCall`) was extracted to
//! `method_call_lower.rs` per §13.4 J1-J6 (LOC reduction). See
//! `src/mir/lower/method_call_lower.rs` for the MethodCall lowering.
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

use crate::hir::*;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::call_lower::lower_closure_call_to_synthesized;
use super::lower_expr_to_operand;
use super::method_resolution::resolve_enum_variant;
use super::ty_lower::lower_path_generic_args;
use super::MirLowerCtxt;
use super::{control_flow, field_resolution, pattern_bindings};
// Stage 18.273+18.305 (TD-LOC-EXPR-VARIANTS): intrinsic lowering functions
// extracted to 4 sub-modules per type. Per §13.4 J2 (单一职责).
// Stage 31.6f: lower_box_new_intrinsic import removed — Box::new now in prelude.
use super::format_intrinsics::lower_format_variadic_intrinsic;
use super::string_intrinsics::lower_string_from_str_intrinsic;
// Stage 18.284 (TD-INTRINSIC-OVERUSE Phase 2-A): primitive intrinsic dispatch
// (post-resolution). Per §13.4 J2 (单一职责).

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
                                                lower_expr_to_operand(cx, &body.value, None);
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
                                                lower_expr_to_operand(cx, &body.value, None);
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
///
/// Stage 18.258 (TD-TUPLE-CTOR-TYPECK Phase 2c): added `expected_ty` param.
/// When the call is a tuple struct ctor (e.g., `Holder(true)`) and turbofish
/// is absent, `expected_ty` carries the expected type from the let-binding
/// (e.g., `let w: Holder<i32> = Holder(true)`). We extract substs from
/// `expected_ty` and use them to resolve field_tys correctly.
///
/// Per §1.0 原則 6 (通解 > 特解): one expected_ty-based path for all
/// ctor calls without turbofish, not a per-type special case.
/// Per §2 原則 9 (正确 > 妥协): proper field type substitution at lower
/// time, not relying on typeck back-propagation.
pub(super) fn lower_call_expr(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    func: &HirExpr,
    args: &[HirExpr],
    expected_ty: Option<&crate::mir::ty::Ty>,
) -> LocalId {
    // Lower func first — this determines whether the call is a real
    // function call or an ADT construction (struct/enum ctor).
    let func_local = lower_expr_to_operand(cx, func, None);

    // Stage 18.262 (TD-TUPLE-CTOR-CALL-ARG Phase 2e): If func resolves
    // to a FnDef and cx.fn_sigs is set, look up the callee's sig and
    // thread each arg's expected type (sig.inputs[i]) into its
    // lower_expr_to_operand call. This closes the soundness hole where
    // `take_holder(Holder(true))` (where fn take_holder(h: Holder<i32>))
    // silently accepted type mismatches.
    //
    // Per §11.2 (allowed cross-stage access — pre-computed data
    // contract): fn_sigs is built upstream by the driver.
    // Per §1.0 原則 6 (通解 > 特解): one fn_sigs-based path for all
    // call args, not a per-type special case.
    // Per §2 原則 9 (正确 > 妥协): proper expected-ty propagation at
    // lower time, not relying on typeck back-propagation (which was
    // unsound for generic tuple struct ctors).
    let callee_sig_inputs: Option<&Vec<crate::mir::ty::Ty>> = {
        let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
        if let Some(ld) = func_local_decl {
            if let TyKind::FnDef(def_id, _) = &ld.ty.kind {
                if let Some(fn_sigs) = cx.fn_sigs {
                    fn_sigs.get(def_id).map(|sig| &sig.inputs)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    // Stage 18.267 (TD-ENUM-VARIANT-CTOR-EXPECTED-TY): Pre-resolve
    // field_tys for Adt ctor path BEFORE lowering args, so we can
    // thread field_tys[i] as expected_ty into each arg's
    // lower_expr_to_operand. This closes the soundness hole where
    // `Some(Holder(true))` (where `let x: Option<Holder<i32>>`)
    // silently accepted type mismatches because args were lowered
    // before field_tys were resolved.
    //
    // Per §17.6 (缺陷纳入 — same class as TD-STRUCT-LITERAL-FIELD-EXPECTED-TY
    // + TD-TUPLE-CTOR-CALL-ARG): when one expected-ty propagation bug
    // is found, audit ALL similar paths until no more found.
    // Per §1.0 原則 6 (通解 > 特解): one Adt-ctor pre-resolution path
    // for all enum variant + tuple struct ctors.
    //
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

    // Pre-resolve field_tys (PAYLOAD ONLY — discriminant excluded for
    // enum variants). For struct tuple ctors, field_tys are the struct's
    // fields directly. For enum variants, field_tys are the variant's
    // payload fields (discriminant is prepended later in the Aggregate
    // construction).
    let pre_adt_field_tys: Option<Vec<Ty>> = if is_adt_ctor {
        let func_local_decl = cx
            .mir
            .local_decls
            .get(func_local.0 as usize)
            .expect("func local must exist");
        let (adt_def_id, adt_substs_from_func) = match &func_local_decl.ty.kind {
            TyKind::Adt(def_id, substs) => (*def_id, substs.clone()),
            _ => unreachable!("checked is_adt_ctor above"),
        };
        // Compute adt_substs using same logic as below (extract from
        // expected_ty if func substs are empty).
        let adt_substs = if adt_substs_from_func.is_empty() {
            if let Some(expected) = expected_ty {
                if let TyKind::Adt(exp_def_id, exp_substs) = &expected.kind {
                    if *exp_def_id == adt_def_id && !exp_substs.is_empty() {
                        exp_substs.clone()
                    } else {
                        adt_substs_from_func.clone()
                    }
                } else {
                    adt_substs_from_func.clone()
                }
            } else {
                adt_substs_from_func.clone()
            }
        } else {
            adt_substs_from_func.clone()
        };
        // Resolve variant_idx + field_tys (with discriminant) using same
        // logic as below, then strip discriminant to get payload-only
        // field_tys for arg expected_ty.
        let (variant_idx, field_tys_with_discr) = if let HirExprKind::Path(path) = &func.kind {
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
        // Stage 18.267 (TD-ENUM-VARIANT-CTOR-EXPECTED-TY): Apply
        // substitution to enum variant field_tys. resolve_enum_variant
        // returns field_tys with Param (not substituted), so we need to
        // apply substitution manually to get concrete types. Without this,
        // `Some(Holder(true))` (with `let x: Option<Holder<i32>>`) would
        // get field_tys = [i32, Param(T)] instead of [i32, Holder<i32>].
        let is_enum = variant_idx > 0
            || cx.hir.is_some_and(|h| {
                h.find_owner(adt_def_id).is_some_and(|o| {
                    matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
                })
            });
        let field_tys_with_discr: Vec<Ty> = if is_enum && !adt_substs.is_empty() {
            // Apply substitution to each field type.
            field_tys_with_discr
                .iter()
                .map(|ty| crate::mir::substitute::substitute(ty, &adt_substs))
                .collect()
        } else {
            field_tys_with_discr
        };
        // Strip discriminant for enum variants: if variant_idx > 0 OR
        // the Adt is an enum, field_tys[0] is the discriminant (i32).
        // For structs, field_tys[0] is the first real field.
        let payload_tys = if is_enum && !field_tys_with_discr.is_empty() {
            field_tys_with_discr[1..].to_vec()
        } else {
            field_tys_with_discr
        };
        Some(payload_tys)
    } else {
        None
    };

    let arg_locals: Vec<LocalId> = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            // Stage 18.264 (TD-BOX-NEW-EXPECTED-TY): For Box::new(x),
            // expected_ty from outer context is `Box<T>` but the arg
            // expects `T`. Extract T from the outer expected Box<T>.
            //
            // Per §17.6 (缺陷纳入 — same class as TD-TUPLE-CTOR-CALL-ARG):
            // when one expected-ty propagation bug is found, audit all
            // similar paths. Box::new intrinsic uses the same pattern
            // as fn call args — needs expected_ty from outer context.
            // Per §1.0 原則 6 (通解 > 特解): one Box-specific extraction
            // path for all Box::new args, not a per-type special case.
            let arg_expected_ty = if i == 0 {
                // Check if this is a Box::new intrinsic call.
                let is_box_new = if let HirExprKind::Path(path) = &func.kind {
                    if path.segments.len() == 2 {
                        let type_name = cx.interner.resolve(&path.segments[0].ident.name);
                        let method_name = cx.interner.resolve(&path.segments[1].ident.name);
                        type_name == "Box" && method_name == "new" && args.len() == 1
                    } else {
                        false
                    }
                } else {
                    false
                };
                if is_box_new {
                    // Extract T from `Box<T>` (the outer expected_ty).
                    if let Some(expected) = expected_ty {
                        if let TyKind::Adt(_, substs) = &expected.kind {
                            if !substs.is_empty() {
                                Some(&substs[0])
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else if let Some(field_tys) = pre_adt_field_tys.as_ref() {
                    // Stage 18.267 (TD-ENUM-VARIANT-CTOR-EXPECTED-TY):
                    // For Adt ctor path, use field_tys[i] as expected_ty
                    // (payload only — discriminant already stripped in
                    // pre_adt_field_tys computation above).
                    field_tys.get(i)
                } else {
                    callee_sig_inputs.as_ref().and_then(|inputs| inputs.get(i))
                }
            } else if let Some(field_tys) = pre_adt_field_tys.as_ref() {
                // Stage 18.267: Adt ctor with multiple args — use
                // field_tys[i] for non-first args too.
                field_tys.get(i)
            } else {
                callee_sig_inputs.as_ref().and_then(|inputs| inputs.get(i))
            };
            lower_expr_to_operand(cx, a, arg_expected_ty)
        })
        .collect();
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
            // Stage 31.6f: All 2-segment path intrinsics (from_str, Box::new)
            // have been migrated to prelude impls. The type_name/method_name
            // variables are no longer needed for intrinsic dispatch.
            // Per §1.0 原則 5 (去除兼容思维): dead dispatch code removed.
            //
            // `from_str` is now implemented in the prelude using `.ptr`/`.len`
            // fat pointer field access + extern "C" calls to __landin_alloc +
            // __landin_memcpy. Standard static method resolution handles it.
            //
            // Per §1.0 原則 6 (通解 > 特解): standard method resolution, no
            // per-method intrinsic dispatch.
            // Per §1.0 原則 5 (去除兼容思维): dead intrinsic dispatch removed.
            // Per §12 (最优 > 最小): root-cause fix via language features.
            // Stage 31.6f (v0.19): Box::new intrinsic REMOVED.
            // Now handled by prelude impl using sizeof(T) + extern C alloc + Deref store.
            // Per §1.0 原則 6 (通解 > 特解): standard method resolution, no intrinsic.
            // Per §1.0 原則 5 (去除兼容思维): dead intrinsic dispatch removed.
            // Stage 18.238 (TD-INTRINSIC-OVERUSE Phase 1): Vec::new() removed.
            // Now handled by prelude impl: `impl<T> Vec<T> { fn new() -> Vec<T> { ... } }`
            // Per §1.0 原則 6 (通解 > 特解): standard method resolution, not hardcoded.
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
    // Stage 18.267: `is_adt_ctor` moved earlier (above pre_adt_field_tys)
    // so we can pre-resolve field_tys before lowering args.
    if is_adt_ctor {
        // Struct/enum ctor: lower as Aggregate(Adt, operands).
        let func_local_decl = cx
            .mir
            .local_decls
            .get(func_local.0 as usize)
            .expect("func local must exist");
        let (adt_def_id, adt_substs_from_func) = match &func_local_decl.ty.kind {
            TyKind::Adt(def_id, substs) => (*def_id, substs.clone()),
            _ => unreachable!("checked is_adt_ctor above"),
        };
        // Stage 18.258 (TD-TUPLE-CTOR-TYPECK Phase 2c): if substs from
        // turbofish are empty (no turbofish) AND expected_ty is
        // Some(Adt with same def_id) with non-empty substs, use the
        // expected substs. This closes the soundness hole where
        // `Holder(true)` with `let : Holder<i32>` would silently
        // accept because field_tys were resolved with empty substs
        // (Param T unifies with anything).
        //
        // Per §1.0 原則 6 (通解 > 特解): one expected_ty-based path for
        // all Adt ctors without turbofish, not a per-type special case.
        // Per §2 原則 9 (正确 > 妥协): proper field type substitution at
        // lower time, not relying on typeck back-propagation (which
        // was unsound).
        // Per §13.4 J3 (one-way flow): expected_ty flows IN from the
        // let-binding context — no back-edges.
        let adt_substs = if adt_substs_from_func.is_empty() {
            if let Some(expected) = expected_ty {
                if let TyKind::Adt(exp_def_id, exp_substs) = &expected.kind {
                    if *exp_def_id == adt_def_id && !exp_substs.is_empty() {
                        exp_substs.clone()
                    } else {
                        adt_substs_from_func.clone()
                    }
                } else {
                    adt_substs_from_func.clone()
                }
            } else {
                adt_substs_from_func.clone()
            }
        } else {
            adt_substs_from_func.clone()
        };
        // Stage 18.346 (P2 soundness fix): If adt_substs is still empty
        // (no turbofish, no expected_ty), infer substs from operand types.
        //
        // Root cause (per §2.2 根因思维 + Stage 18.345 analysis):
        // When `let w = Wrapper { inner: 42i64 }` is lowered without
        // turbofish, Path resolution returns empty adt_substs. This
        // causes codegen to use raw field_tys (containing Param(0))
        // instead of monomorphized types (containing i64).
        //
        // Fix: After computing adt_substs from Path/expected_ty, if
        // still empty, infer substs by matching field_tys (which
        // contain Param(N)) against operand types (which are concrete).
        //
        // Algorithm:
        // 1. Resolve field_tys (unsubstituted — contain Param(N))
        // 2. For each field_ty[i] that is Param(N), look at operand[i]'s type
        // 3. Build substitution: Param(N) → operand_type
        // 4. Convert to Vec<Ty> indexed by Param index
        //
        // Per §1.0 原則 6 (通解 > 特解): one inference path for all
        // generic structs without turbofish.
        // Per §12 (最优 > 最小): root-cause fix at MIR lower, not codegen.
        // Per §20 (iterative audit): Stage 18.345 root cause verified.
        let adt_substs = if adt_substs.is_empty() {
            // Infer substs from operand types + field_tys.
            // First resolve field_tys (unsubstituted — contain Param(N)).
            let unresolved_field_tys = field_resolution::resolve_adt_field_tys(cx, adt_def_id);
            // Count generic params from HIR.
            let generic_params = cx
                .hir
                .map(|h| crate::hir::generics::find_generics(adt_def_id, h))
                .unwrap_or_default();
            if generic_params.is_empty() {
                // Non-generic struct — no substs needed.
                adt_substs
            } else {
                // Generic struct — infer substs from operands.
                // For each field_ty that is Param(N), the operand's type
                // tells us what T should be.
                let mut inferred_substs: Vec<Ty> = Vec::with_capacity(generic_params.len());
                for _ in 0..generic_params.len() {
                    inferred_substs.push(Ty::new(TyKind::Error, expr.span));
                }
                for (i, field_ty) in unresolved_field_tys.iter().enumerate() {
                    if let TyKind::Param(param) = &field_ty.kind {
                        let param_idx = param.index as usize;
                        if param_idx < inferred_substs.len() {
                            // Get the operand's type for this field.
                            // arg_operands are the lowered operand LocalIds.
                            // We need to read the local's type from MIR.
                            if i < arg_locals.len() {
                                let op_local = arg_locals[i];
                                if let Some(ld) = cx.mir.local_decls.get(op_local.0 as usize) {
                                    let op_ty = ld.ty.clone();
                                    // Only use concrete types (not Infer/Error).
                                    if !matches!(op_ty.kind, TyKind::Infer(_) | TyKind::Error) {
                                        inferred_substs[param_idx] = op_ty;
                                    }
                                }
                            }
                        }
                    }
                }
                // Check if all substs were inferred (no Error remaining).
                if inferred_substs
                    .iter()
                    .all(|t| !matches!(t.kind, TyKind::Error))
                {
                    inferred_substs.into()
                } else {
                    // Couldn't infer all substs — keep empty.
                    adt_substs
                }
            }
        } else {
            adt_substs
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
        // Stage 18.267 (TD-ENUM-VARIANT-CTOR-EXPECTED-TY): Apply
        // substitution to enum variant field_tys. resolve_enum_variant
        // returns field_tys with Param (not substituted), so we need to
        // apply substitution manually. Without this, the Aggregate's
        // field_tys would be [i32, Param(T)] instead of [i32, T_substituted],
        // and typeck's unify would silently accept Param(T) with anything.
        let is_enum_for_subst = variant_idx > 0
            || cx.hir.is_some_and(|h| {
                h.find_owner(adt_def_id).is_some_and(|o| {
                    matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
                })
            });
        let field_tys: Vec<Ty> = if is_enum_for_subst && !adt_substs.is_empty() {
            field_tys
                .iter()
                .map(|ty| crate::mir::substitute::substitute(ty, &adt_substs))
                .collect()
        } else {
            field_tys
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
        // Stage 18.404 (v0.5+ Phase 2 L3): Use expected_ty for dest_ty when
        // available AND when it's not from a type annotation (let : T = call()).
        // When expected_ty comes from `let x: i32 = g()` and g() returns bool,
        // using expected_ty (i32) as dest_ty would make typeck see dest=i32,
        // rvalue=bool → mismatch. But the error message direction would be
        // "expected i32, found bool" — which is CORRECT per §2 原則 9.
        //
        // However, this changes the error message direction from the test's
        // expectation. The test expects "expected i32, found bool" but with
        // expected_ty as dest_ty, the rvalue (call result) would have the
        // fn's return type (bool) while the place has expected_ty (i32).
        // So the message would be "expected i32, found bool" — correct!
        //
        // The actual issue is that with expected_ty, the dest_local.ty is
        // already i32 (concrete), so typeck doesn't need the rvalue type at
        // all — it sees dest=i32, rvalue=bool → mismatch with correct direction.
        //
        // Per §1.6 终极检验: this IS the root-cause fix.
        // Per §12 (最优 > 最小): root-cause fix at the lower site.
        // Per §1.0 原則 6 (通解 > 特解): one expected_ty path for all call dests.
        let dest_ty = expected_ty
            .cloned()
            .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
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
    let start_local = lower_expr_to_operand(cx, start_expr, None);
    let end_local = lower_expr_to_operand(cx, end_expr, None);

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
    control_flow::lower_block(cx, body, None);
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
