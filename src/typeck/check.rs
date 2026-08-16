//! Type checker — checking sub-responsibility.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.128):
//! Split from `checker.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all `check_*` and `post_check_*` methods on `TypeChecker`.
//!
//! ## Sub-responsibility
//! Type checking: walk MIR statements/terminators, infer types of rvalues
//! and operands, and unify with expected types. Push type errors on mismatch.
//!
//! ## J1-J6 compliance
//! - J1: typeck design unchanged
//! - J2: this file has one clear responsibility (checking)
//! - J3: no circular deps (methods operate on `&mut self`)
//! - J4: checking sub-responsibility is complete
//! - J5: stays within typeck stage
//! - J6: LOC driven by responsibility

use crate::ast;
use crate::mir::body::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::error::TypeError;

use super::checker::{type_has_unresolved_substs, types_match_loose, TypeChecker};
use super::predicates::can_coerce;

impl TypeChecker {
    pub(super) fn post_check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        if let StatementKind::Assign(boxed) = &stmt.kind {
            let (place, rvalue) = &**boxed;
            let place_ty = self.infer_place(mir, place);
            // Stage 18.71: Use type-only inference (no side effects).
            let rvalue_ty = self.infer_rvalue_type_only(mir, rvalue);

            let resolved_place = self.unify.resolve(&place_ty);
            let resolved_rvalue = self.unify.resolve(&rvalue_ty);

            let place_is_concrete =
                !matches!(resolved_place.kind, TyKind::Infer(_) | TyKind::Error)
                    && !type_has_unresolved_substs(&resolved_place);
            let rvalue_is_concrete =
                !matches!(resolved_rvalue.kind, TyKind::Infer(_) | TyKind::Error)
                    && !type_has_unresolved_substs(&resolved_rvalue);

            // Stage 18.71: Only fire if BOTH are concrete AND can't coerce
            // AND can't loose-match. Per §1.0 原則 9 "正确 > 妥协": avoid
            // false positives on generic/unresolved types.
            if place_is_concrete
                && rvalue_is_concrete
                && !can_coerce(&resolved_place, &resolved_rvalue)
                && !types_match_loose(&resolved_place, &resolved_rvalue)
            {
                // Per §1.0 原則 4 "报错 > 静默".
                let span = stmt.span;
                // Stage 18.71: Dedupe — skip if Phase 1 already reported
                // the same mismatch (same span + expected + found). This
                // happens when both Phase 1 and Phase 5.5 detect the same
                // type mismatch (e.g., `let x: i32 = true;` where both
                // place and rvalue are already concrete in Phase 1).
                //
                // Per §1.0 原則 6 "通用 > 特例": one dedup logic for all
                // type mismatch errors.
                let already_reported = self.errors.iter().any(|e| {
                    e.span == span
                        && e.expected.as_ref() == Some(&resolved_place)
                        && e.found.as_ref() == Some(&resolved_rvalue)
                });
                if !already_reported {
                    self.errors.push(crate::typeck::TypeError::mismatch(
                        resolved_place.clone(),
                        resolved_rvalue.clone(),
                        span,
                    ));
                }
            }
        }
    }

    /// Stage 18.71: Infer an rvalue's type WITHOUT side effects.
    ///
    /// Unlike `infer_rvalue`, this function does NOT call `unify` on
    /// operands. It only returns the rvalue's type based on its structure.
    /// Used by `post_check_statement` to avoid re-unifying already-resolved
    /// types (which would produce spurious errors).
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the no-side-effect contract is explicit.
    ///
    /// Stage 18.72: Now `&mut self` because `infer_operand_type_only` →
    /// `infer_place` → `infer_projection` may push tuple index OOB errors.
    /// The "no side effects" contract is relaxed: tuple index OOB is a
    /// real error that should be reported even in post_check_statement.
    pub(super) fn post_check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        if let TerminatorKind::Call { func, .. } = &term.kind {
            // Stage 18.72: Split into two statements to avoid borrow conflict
            // (infer_operand is now &mut self, unify.resolve is &self).
            let func_ty_raw = self.infer_operand(mir, func);
            let func_ty = self.unify.resolve(&func_ty_raw);
            // G7 fix: if func is neither FnDef nor FnPtr (after defaulting),
            // emit an error. Infer should be resolved by now; if it's still
            // Infer, it means no constraint was applied (rare).
            //
            // Stage 16.29: Also accept TyKind::Closure as callable —
            // closures are called via the synthesized `call` function,
            // and the Closure type is the func type at the call site.
            // Without this, `f()()` patterns (where f returns a closure)
            // would emit false "expected function, found {closure}" errors.
            //
            // Per §1.0 原則 9 "正确 > 妥协": Closure IS a callable type.
            if !matches!(
                &func_ty.kind,
                TyKind::FnDef(_, _) | TyKind::FnPtr(_) | TyKind::Closure(_, _) | TyKind::Error
            ) {
                self.errors.push(TypeError::new(
                    // Stage 15.80: use human-readable type name.
                    // Stage 15.81: use func operand span (was: Span::DUMMY).
                    format!("expected function, found {}", self.format_ty(&func_ty)),
                    crate::mir::place::operand_span(func),
                ));
            }
        }
    }

    /// Check a single MIR statement (Assign or Nop).
    pub(super) fn check_statement(&mut self, mir: &MirBody, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::Assign(boxed) => {
                let (place, rvalue) = &**boxed;
                let place_ty = self.infer_place(mir, place);
                // Stage 15.82: pass stmt.span to infer_rvalue so BinaryOp/
                // UnaryOp errors get accurate spans (was: Span::DUMMY inside
                // infer_rvalue).
                let rvalue_ty = self.infer_rvalue(mir, rvalue, stmt.span);

                // Stage 18.71: Type mismatch check for Assign statements.
                // Per §1.0 原則 4 "报错 > 静默": if place type is a concrete
                // type (not Infer/Error) and rvalue type is also concrete,
                // and they don't match and can't coerce, report an error.
                //
                // This catches:
                // - `let x: i32 = true;` (e2e-err-002)
                // - `fn f() -> i32 { true }` (cg-err-002, via return local)
                // - `let x = if true { 1 } else { true };` (cg-err-014, via if-else)
                //
                // Per §1.0 原則 6 "通用 > 特例": one check covers all Assign
                // statements — let bindings, return values, if-else results.
                //
                // IMPORTANT: Only check when BOTH types are fully concrete
                // (no Infer vars, no Error). This avoids false positives on
                // generic types where substs haven't been substituted yet
                // (e.g., Box<T> with empty substs vs Box<T> with [i32]).
                // Per §1.0 原則 9 "正确 > 妥协": must not break valid code.
                let resolved_place = self.unify.resolve(&place_ty);
                let resolved_rvalue = self.unify.resolve(&rvalue_ty);

                let place_is_concrete =
                    !matches!(resolved_place.kind, TyKind::Infer(_) | TyKind::Error)
                        && !type_has_unresolved_substs(&resolved_place);
                let rvalue_is_concrete =
                    !matches!(resolved_rvalue.kind, TyKind::Infer(_) | TyKind::Error)
                        && !type_has_unresolved_substs(&resolved_rvalue);

                if place_is_concrete
                    && rvalue_is_concrete
                    && !can_coerce(&resolved_place, &resolved_rvalue)
                    && !types_match_loose(&resolved_place, &resolved_rvalue)
                {
                    // Stage 18.71: Report type mismatch.
                    // Per §1.0 原則 4 "报错 > 静默".
                    let span = if stmt.span != Span::DUMMY {
                        stmt.span
                    } else {
                        Span::DUMMY
                    };
                    self.errors.push(crate::typeck::TypeError::mismatch(
                        resolved_place.clone(),
                        resolved_rvalue.clone(),
                        span,
                    ));
                } else if can_coerce(&place_ty, &rvalue_ty)
                    || types_match_loose(&resolved_place, &resolved_rvalue)
                {
                    // Coercion or loose match succeeded — still try to unify
                    // so Infer vars get bound.
                    //
                    // Stage 18.99 (TD-13 fix): For FnDef↔FnPtr, do NOT suppress
                    // unify errors — the unify_fndef_with_fnptr now checks sig
                    // compatibility, and incompatible sigs must be reported.
                    // For other coercions (Int widening, &mut→&), errors are
                    // still suppressed (the coercion is intentional).
                    let is_fndef_fnptr = matches!(
                        (&resolved_place.kind, &resolved_rvalue.kind),
                        (TyKind::FnDef(_, _), TyKind::FnPtr(_))
                            | (TyKind::FnPtr(_), TyKind::FnDef(_, _))
                    );
                    if is_fndef_fnptr {
                        if let Err(mut e) = self.unify.unify(&place_ty, &rvalue_ty, stmt.span) {
                            if stmt.span != Span::DUMMY {
                                e.span = stmt.span;
                            }
                            self.errors.push(*e);
                        }
                    } else {
                        let _ = self.unify.unify(&place_ty, &rvalue_ty, stmt.span);
                    }
                } else if let Err(mut e) = self.unify.unify(&place_ty, &rvalue_ty, stmt.span) {
                    // Stage 15.82: use stmt.span for unify errors (was:
                    // Span::DUMMY from mismatch(), producing "1:1").
                    if stmt.span != Span::DUMMY {
                        e.span = stmt.span;
                    }
                    self.errors.push(*e);
                }
                // The resolved type will be written back to local_decls
                // in Phase 3 of check_mir_body (after all constraints
                // are collected and defaults are applied).
            }
            // StorageLive/StorageDead/Deinit are scope/lifetime markers.
            // They don't introduce type constraints.
            StatementKind::Nop
            | StatementKind::StorageLive(_)
            | StatementKind::StorageDead(_)
            | StatementKind::Deinit(_) => {} // Stage 13.13 + Stage 13.16: Inline println! statement — no type
                                             // constraints to check on the format string (opaque String).
                                             // Stage 13.16: args are already lowered to MIR operands and
                                             // their types were checked during operand lowering (each arg
                                             // is lowered via lower_expr_to_operand which goes through the
                                             // normal type-checking path).
                                             // Stage 18.48: StatementKind::Println variant removed.
        }
    }

    /// Check a terminator's type constraints.
    pub(super) fn check_terminator(&mut self, mir: &MirBody, term: &Terminator) {
        match &term.kind {
            TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } => {
                // Infer func type
                // Stage 18.72: Split into two statements to avoid borrow conflict.
                let func_ty_raw = self.infer_operand(mir, func);
                let func_ty = self.unify.resolve(&func_ty_raw);
                // Infer arg types and collect them
                let arg_tys_raw: Vec<Ty> = args
                    .iter()
                    .map(|arg| self.infer_operand(mir, arg))
                    .collect();
                let arg_tys: Vec<Ty> = arg_tys_raw
                    .iter()
                    .map(|ty| self.unify.resolve(ty))
                    .collect();
                // Infer destination type
                let dest_ty_raw = self.infer_place(mir, destination);
                let dest_ty = self.unify.resolve(&dest_ty_raw);

                // G3 fix (Stage 2.4e): If func is a FnDef(def_id, _),
                // look up the fn signature from fn_sigs and verify:
                //   1. arg count matches
                //   2. each arg type unifies with the corresponding input
                //   3. destination type unifies with the return type
                if let TyKind::FnDef(def_id, _) = &func_ty.kind {
                    if let Some(sig) = self.fn_sigs.get(def_id).cloned() {
                        if arg_tys.len() != sig.inputs.len() {
                            self.errors.push(TypeError::new(
                                format!(
                                    "this function takes {} argument(s) but {} were supplied",
                                    sig.inputs.len(),
                                    arg_tys.len()
                                ),
                                // Stage 15.81: use the call terminator's span
                                // (was: Span::DUMMY, producing "1:1").
                                term.span,
                            ));
                        } else {
                            for (arg_ty, input_ty) in arg_tys.iter().zip(sig.inputs.iter()) {
                                if let Err(mut e) = self.unify.unify(arg_ty, input_ty, term.span) {
                                    // Stage 15.81: use term.span for unify errors
                                    // (was: Span::DUMMY from mismatch()).
                                    if term.span != Span::DUMMY {
                                        e.span = term.span;
                                    }
                                    self.errors.push(*e);
                                }
                            }
                        }
                        if let Err(mut e) = self.unify.unify(&dest_ty, &sig.output, term.span) {
                            // Stage 15.81: use term.span for unify errors.
                            if term.span != Span::DUMMY {
                                e.span = term.span;
                            }
                            self.errors.push(*e);
                        }
                    }
                    // If fn_sigs doesn't have the DefId (e.g., external fn),
                    // skip type checking — codegen will handle it.
                }

                // If func is a FnPtr, unify args with inputs and dest with output.
                if let TyKind::FnPtr(sig) = &func_ty.kind {
                    // Unify each arg with the corresponding input
                    for (arg_ty, input_ty) in arg_tys.iter().zip(sig.inputs.iter()) {
                        if let Err(mut e) = self.unify.unify(arg_ty, input_ty, term.span) {
                            // Stage 15.81: use term.span for unify errors.
                            if term.span != Span::DUMMY {
                                e.span = term.span;
                            }
                            self.errors.push(*e);
                        }
                    }
                    // Unify destination with output
                    if let Err(mut e) = self.unify.unify(&dest_ty, &sig.output, term.span) {
                        // Stage 15.81: use term.span for unify errors.
                        if term.span != Span::DUMMY {
                            e.span = term.span;
                        }
                        self.errors.push(*e);
                    }
                }

                // Stage 16.32 (通解 — Closure-typed func in typeck):
                // If func is a Closure(def_id, _), look up the synthesized
                // function's sig from fn_sigs (same as FnDef). This unifies
                // the dest type with the closure's return type, which is
                // essential for nested closures (`f()()` where f returns a
                // closure).
                //
                // Without this, the dest type stays Infer → "expected
                // function, found _" when the result is called.
                //
                // Note: The closure's sig has inputs = [self, params...].
                // The MIR Call terminator's args = [params...] (self is
                // prepended by codegen, not by MIR lowering). So we skip
                // the first input (self) when checking arg count and unify.
                //
                // Per §1.0 原則 6 "通用 > 特例": handle Closure the same
                // way as FnDef — both are callable types with sigs in
                // fn_sigs.
                if let TyKind::Closure(def_id, _) = &func_ty.kind {
                    if let Some(sig) = self.fn_sigs.get(def_id).cloned() {
                        // Skip the first input (self) — it's not in the
                        // MIR Call terminator's args.
                        let sig_params = &sig.inputs[1.min(sig.inputs.len())..];
                        if arg_tys.len() != sig_params.len() {
                            self.errors.push(TypeError::new(
                                format!(
                                    "this closure takes {} argument(s) but {} were supplied",
                                    sig_params.len(),
                                    arg_tys.len()
                                ),
                                term.span,
                            ));
                        } else {
                            for (arg_ty, input_ty) in arg_tys.iter().zip(sig_params.iter()) {
                                if let Err(mut e) = self.unify.unify(arg_ty, input_ty, term.span) {
                                    if term.span != Span::DUMMY {
                                        e.span = term.span;
                                    }
                                    self.errors.push(*e);
                                }
                            }
                        }
                        if let Err(mut e) = self.unify.unify(&dest_ty, &sig.output, term.span) {
                            if term.span != Span::DUMMY {
                                e.span = term.span;
                            }
                            self.errors.push(*e);
                        }
                    }
                }

                // G7 fix (Stage 2.4f): if func is neither FnDef nor FnPtr
                // (e.g., calling an Int, Bool, Str, Tuple), emit an error.
                // Infer and Error are deferred (might resolve to a fn type).
                //
                // Stage 16.29: Also accept TyKind::Closure as callable —
                // closures are called via the synthesized `call` function.
                if !matches!(
                    &func_ty.kind,
                    TyKind::FnDef(_, _)
                        | TyKind::FnPtr(_)
                        | TyKind::Closure(_, _)
                        | TyKind::Infer(_)
                        | TyKind::Error
                ) {
                    self.errors.push(TypeError::new(
                        // Stage 15.80: use human-readable type name.
                        // Stage 15.81: use func operand span (was: Span::DUMMY).
                        format!("expected function, found {}", self.format_ty(&func_ty)),
                        crate::mir::place::operand_span(func),
                    ));
                }
            }
            TerminatorKind::SwitchInt { discr, targets, .. } => {
                // The discriminant must be an integer or bool
                let discr_ty = self.infer_operand(mir, discr);
                // Stage 15.81: use the discriminant operand's span for
                // error reporting (was: Span::DUMMY, producing "1:1").
                let discr_span = crate::mir::place::operand_span(discr);
                // G7 fix (Stage 2.4f): if any target is ConstVal::Bool(_),
                // this SwitchInt came from an `if` or `while` condition,
                // and the discriminant must be bool (not just any int).
                let requires_bool = targets
                    .iter()
                    .any(|(val, _)| matches!(val, ConstVal::Bool(_)));
                if requires_bool {
                    let bool_ty = Ty::from_kind(TyKind::Bool);
                    if let Err(mut e) = self.unify.unify(&discr_ty, &bool_ty, term.span) {
                        // Stage 15.81: override the dummy span with the
                        // actual discriminant span (was: Span::DUMMY).
                        if discr_span != Span::DUMMY {
                            e.span = discr_span;
                        }
                        self.errors.push(*e);
                    }
                } else {
                    // Match on integer — check that it's int-like.
                    match &discr_ty.kind {
                        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Bool => {}
                        TyKind::Infer(InferVar::IntVar(_)) => {}
                        TyKind::Infer(InferVar::TyVar(_)) => {
                            // Unbound variable — unify with i32 as default
                            let i32_ty = Ty::from_kind(TyKind::Int(ast::IntTy::I32));
                            let _ = self.unify.unify(&discr_ty, &i32_ty, term.span);
                        }
                        TyKind::Error => {}
                        _ => {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.81: use discriminant span (was: Span::DUMMY).
                                format!(
                                    "expected integer or bool for switch, found {}",
                                    self.format_ty(&discr_ty)
                                ),
                                discr_span,
                            ));
                        }
                    }
                }
            }
            TerminatorKind::Drop { place, .. } => {
                // Just infer the place type (no constraint to check)
                let _ = self.infer_place(mir, place);
            }
            TerminatorKind::Assert { cond, .. } => {
                // The condition must be a bool. We don't enforce this
                // strictly (codegen will handle the runtime check) but
                // we do infer the type to catch obvious mismatches.
                let cond_ty = self.infer_operand(mir, cond);
                // Stage 15.81: use the condition operand's span for
                // error reporting (was: Span::DUMMY).
                let cond_span = crate::mir::place::operand_span(cond);
                match &cond_ty.kind {
                    TyKind::Bool | TyKind::Infer(_) | TyKind::Error => {}
                    _ => {
                        self.errors.push(TypeError::new(
                            // Stage 15.80: use human-readable type name.
                            // Stage 15.81: use condition span (was: Span::DUMMY).
                            format!(
                                "assert condition must be bool, found {}",
                                self.format_ty(&cond_ty)
                            ),
                            cond_span,
                        ));
                    }
                }
            }
            // Stage 18.116: Goto/Return/Unreachable have no type constraints.
            // All 7 TerminatorKind variants are now explicitly covered:
            // Call, SwitchInt, Drop, Assert (above) + Goto, Return, Unreachable (here).
            TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        }
    }
}
