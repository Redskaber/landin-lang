//! Stage 6.6: Control flow lowering extraction from mir/lower/mod.rs (TD-011 split step 6).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (2452 → ~1980).
//! Contains functions for lowering control flow constructs: short-circuit
//! evaluation, deref expressions, blocks, if/match expressions.

use crate::hir::*;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

use super::pattern_bindings::{collect_pat_bindings_for_mir, lower_enum_variant_pattern_bindings};
use super::{lower_expr_to_operand, MirLowerCtxt};

/// Lower a short-circuit `&&` / `||` expression to MIR control flow.
pub(crate) fn lower_short_circuit(
    cx: &mut MirLowerCtxt,
    op: HirBinOp,
    lhs: &HirExpr,
    rhs: &HirExpr,
    span: Span,
) -> LocalId {
    let lhs_local = lower_expr_to_operand(cx, lhs);
    let eval_rhs_block = cx.new_block();
    let short_circuit_block = cx.new_block();
    let result_true_block = cx.new_block();
    let result_false_block = cx.new_block();
    let cont_block = cx.new_block();
    let result_local = cx.mir.new_local_with_mut(
        Ty::new(TyKind::Bool, span),
        None,
        span,
        crate::mir::ty::Mutability::Mutable,
    );

    // bb_curr: switchInt(lhs) → {true: eval_rhs, _: short_circuit}
    // For `&&`: short-circuit value is `false` (if lhs is false, result is false).
    // For `||`: short-circuit value is `true` (if lhs is true, result is true).
    let (true_target, false_target) = match op {
        HirBinOp::And => (eval_rhs_block, short_circuit_block),
        HirBinOp::Or => (short_circuit_block, eval_rhs_block),
        _ => unreachable!("lower_short_circuit called with non-And/Or op"),
    };
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Place::local(lhs_local, lhs.span)),
        targets: vec![(ConstVal::Bool(true), true_target)],
        otherwise: false_target,
    });

    // short_circuit_block: result = (op == Or); goto cont
    //   For `&&`: lhs was false → result = false
    //   For `||`: lhs was true  → result = true
    cx.current_block = short_circuit_block;
    let short_val = matches!(op, HirBinOp::Or);
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            val: ConstVal::Bool(short_val),
        })),
        span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // eval_rhs_block: evaluate rhs, switchInt(rhs) → {true: result_true, _: result_false}
    cx.current_block = eval_rhs_block;
    let rhs_local = lower_expr_to_operand(cx, rhs);
    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Place::local(rhs_local, rhs.span)),
        targets: vec![(ConstVal::Bool(true), result_true_block)],
        otherwise: result_false_block,
    });

    // result_true_block: result = true; goto cont
    cx.current_block = result_true_block;
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            val: ConstVal::Bool(true),
        })),
        span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // result_false_block: result = false; goto cont
    cx.current_block = result_false_block;
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            val: ConstVal::Bool(false),
        })),
        span,
    );
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation
    cx.current_block = cont_block;
    result_local
}

/// Lower a HIR deref expression `*inner` to MIR.
///
/// `*p` reads the value at the place `Projection(p, Deref)`. We:
/// 1. Lower `inner` to a local (the pointer/reference value)
/// 2. Construct `Place::Projection(local, Deref)`
/// 3. Assign `result = Use(Copy(projection))` to a fresh temp
///
/// The temp's type is left as a fresh inference variable — typeck will
/// unify it with the pointee type via `infer_projection(Deref)`.
pub(crate) fn lower_deref_expr(cx: &mut MirLowerCtxt, inner: &HirExpr, span: Span) -> LocalId {
    let inner_local = lower_expr_to_operand(cx, inner);
    let proj = Place {
        kind: PlaceKind::Projection(
            Box::new(Place::local(inner_local, inner.span)),
            ProjectionElem::Deref,
        ),
        span,
    };
    let result_ty = cx.fresh_infer_ty(span);
    cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Copy(proj)), result_ty, span)
}
/// evaluates the trailing expression (if any). Returns the LocalId
/// of the block's result value.
pub(crate) fn lower_block(cx: &mut MirLowerCtxt, block: &HirBlock) -> LocalId {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Local(local) => {
                // Lower the init expression first (if present)
                if let Some(init) = &local.init {
                    let init_local = lower_expr_to_operand(cx, init);
                    // Allocate a local for this binding. If the let has
                    // an explicit type annotation (`let x: T = ...`), use
                    // it directly; this lets typeck unify the init's type
                    // with the annotation, catching mismatches like
                    // `let x: bool = 42`.
                    let ty = match &local.ty {
                        Some(t) => super::lower_hir_ty_to_mir_ty(t),
                        None => cx.fresh_infer_ty(local.span),
                    };
                    // G1 fix (Stage 2.4e): use `local.pat.hir_id` (not
                    // `local.hir_id`) as the local_map key. The resolver
                    // inserts bindings into the scope keyed by `pat.hir_id`,
                    // so Path expressions resolve to `pat.hir_id`. Using
                    // `local.hir_id` would create a mismatch and cause all
                    // let-bound variables to be unresolvable in Path
                    // expressions.
                    //
                    // G5 fix (Stage 2.4e): extract mutability from the
                    // pattern's BindingMode. `let mut x = ...` produces
                    // `ByValue(Mutable)`. Without this, all locals are
                    // immutable and the borrow checker can't catch
                    // `let x = 1; x = 2;`.
                    let mutability = super::pattern_bindings::pat_mutability(&local.pat);
                    let local_id = cx.new_local_with_mut(local.pat.hir_id, ty, None, mutability);
                    // Emit StorageLive to mark the local as in-scope.
                    // Codegen uses this to allocate stack space.
                    cx.mir
                        .block_mut(cx.current_block)
                        .statements
                        .push(Statement {
                            kind: StatementKind::StorageLive(local_id),
                            span: local.span,
                        });
                    // Use Operand::Move instead of Operand::Copy. For Copy
                    // types, Move is equivalent to Copy (the source remains
                    // valid). For non-Copy types, Move correctly transfers
                    // ownership. Using Copy here would fail the borrow
                    // checker's Copy-ness check on non-Copy types (e.g.,
                    // `let s = "hello"` where s : Str — Str is not Copy).
                    cx.push_assign(
                        Place::local(local_id, local.span),
                        Rvalue::Use(Operand::Move(Place::local(init_local, init.span))),
                        local.span,
                    );

                    // Stage 13.3a (TD-030): Propagate closure info from
                    // init_local to local_id. If the init was a closure
                    // literal (or another let-bound closure), the closure
                    // body info is registered in `cx.closure_bodies` keyed
                    // by init_local. When the closure is later called via
                    // the let-bound name (e.g., `let g = |x| ...; g(5);`),
                    // the `HirExprKind::Call` arm looks up the info by
                    // the let_local (local_id) — so we propagate the info
                    // from init_local to local_id here.
                    //
                    // This handles:
                    //   - `let g = |x| ...;`  (init_local = closure_local)
                    //   - `let h = g;`        (init_local = g's local_id)
                    //
                    // It does NOT handle:
                    //   - `let h = if cond { |x| ... } else { |x| ... };`
                    //     (both branches have different closures; we'd need
                    //     a more sophisticated approach — deferred to
                    //     Stage 13.5+ when full Strategy A is implemented)
                    //   - `let h = some_func();` where some_func returns a
                    //     closure (we don't track closure info across fn
                    //     boundaries — would require Strategy A)
                    //
                    // The propagation is by reference (cloned), so subsequent
                    // mutations to `cx.closure_bodies` (e.g., another
                    // closure literal in the same scope) don't affect
                    // already-registered entries.
                    if let Some(info) = cx.closure_bodies.get(&init_local).cloned() {
                        cx.closure_bodies.insert(local_id, info);
                    }
                } else {
                    // No init: just allocate the local. If a type annotation
                    // is present, use it; otherwise fresh Infer var.
                    let ty = match &local.ty {
                        Some(t) => super::lower_hir_ty_to_mir_ty(t),
                        None => cx.fresh_infer_ty(local.span),
                    };
                    // G1 fix: use pat.hir_id (see comment above).
                    // G5 fix: extract mutability.
                    let mutability = super::pattern_bindings::pat_mutability(&local.pat);
                    let local_id = cx.new_local_with_mut(local.pat.hir_id, ty, None, mutability);
                    // Emit StorageLive even for uninit locals (codegen
                    // still needs to allocate stack space).
                    cx.mir
                        .block_mut(cx.current_block)
                        .statements
                        .push(Statement {
                            kind: StatementKind::StorageLive(local_id),
                            span: local.span,
                        });
                }
            }
            HirStmt::Expr(e, _) => {
                lower_expr_to_operand(cx, e);
            }
            _ => {}
        }
    }
    // Trailing expression
    if let Some(expr) = &block.expr {
        lower_expr_to_operand(cx, expr)
    } else {
        // No trailing expr → unit
        cx.eval_rvalue_to_temp(
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            Ty::new(TyKind::Tuple(vec![]), block.span),
            block.span,
        )
    }
}

/// Lower an if expression.
pub(crate) fn lower_if(
    cx: &mut MirLowerCtxt,
    cond: &HirExpr,
    then: &HirBlock,
    else_: Option<&HirExpr>,
    span: Span,
) -> LocalId {
    let cond_local = lower_expr_to_operand(cx, cond);
    let then_block = cx.new_block();
    let else_block = cx.new_block();
    let cont_block = cx.new_block();
    let result_ty = cx.fresh_infer_ty(span);
    let result_local =
        cx.mir
            .new_local_with_mut(result_ty, None, span, crate::mir::ty::Mutability::Mutable);

    cx.terminate(Terminator::SwitchInt {
        discr: Operand::Copy(Place::local(cond_local, cond.span)),
        targets: vec![(ConstVal::Bool(true), then_block)],
        otherwise: else_block,
    });

    // Then block
    cx.current_block = then_block;
    let then_result = lower_block(cx, then);
    // Stage 13.21: If the then block ended with `return`/`break`/`continue`,
    // the block is already terminated — skip the assign and Goto.
    if !cx.is_terminated() {
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Use(Operand::Copy(Place::local(then_result, then.span))),
            then.span,
        );
        cx.terminate(Terminator::Goto(cont_block));
    }

    // Else block
    cx.current_block = else_block;
    if let Some(else_expr) = else_ {
        let else_result = lower_expr_to_operand(cx, else_expr);
        if !cx.is_terminated() {
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(else_result, else_expr.span))),
                else_expr.span,
            );
            cx.terminate(Terminator::Goto(cont_block));
        }
    } else {
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            span,
        );
        cx.terminate(Terminator::Goto(cont_block));
    }

    // Continuation block
    cx.current_block = cont_block;
    result_local
}

pub(crate) fn lower_match(
    cx: &mut MirLowerCtxt,
    scrutinee: &HirExpr,
    arms: &[HirArm],
    span: Span,
) -> LocalId {
    let scrut_local = lower_expr_to_operand(cx, scrutinee);
    let cont_block = cx.new_block();
    let result_ty = cx.fresh_infer_ty(span);
    let result_local =
        cx.mir
            .new_local_with_mut(result_ty, None, span, crate::mir::ty::Mutability::Mutable);

    // Stage 3.40 (L-ENUM-MATCH): Check if the scrutinee is an enum type.
    // If so, extract the discriminant (field 0 of the enum struct) and
    // switch on that instead of the enum value itself.
    //
    // We check both the MIR local type AND the HIR enum owners — the
    // local type may be Infer (if typeck hasn't resolved it yet at lower
    // time) but the HIR owner can tell us it's an enum.
    let scrut_ty = cx
        .mir
        .local_decls
        .get(scrut_local.0 as usize)
        .map(|ld| ld.ty.clone())
        .unwrap_or_else(|| Ty::new(TyKind::Error, span));
    let is_enum = matches!(&scrut_ty.kind, TyKind::Adt(def_id, _) if cx.hir.and_then(|h| h.owner(*def_id)).is_some_and(|o| {
        matches!(o, crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(_)))
    }));
    // Also check: if any arm pattern is an enum variant path, treat as enum.
    let has_enum_pat = arms.iter().any(|arm| {
        matches!(&arm.pat.kind, HirPatKind::Path(p) | HirPatKind::TupleStruct(p, _) | HirPatKind::Struct(p, _, _)
            if matches!(p.res, Res::Def(_, crate::resolve::DefKind::Enum)))
    });
    let is_enum = is_enum || has_enum_pat;

    // If enum, extract discriminant: discr = scrut.0 (field 0 of the struct).
    let switch_discr = if is_enum {
        // Create a temp local for the extracted discriminant.
        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
        cx.push_assign(
            Place::local(discr_local, span),
            Rvalue::Use(Operand::Move(Place {
                kind: PlaceKind::Projection(
                    Box::new(Place::local(scrut_local, scrutinee.span)),
                    ProjectionElem::Field(FieldId(0), discr_ty.clone()),
                ),
                span,
            })),
            span,
        );
        Operand::Copy(Place::local(discr_local, span))
    } else {
        Operand::Copy(Place::local(scrut_local, scrutinee.span))
    };

    // Collect targets: (constant, arm_block) pairs
    let mut targets: Vec<(ConstVal, BasicBlockId)> = Vec::new();
    let mut arm_blocks: Vec<BasicBlockId> = Vec::new();
    let otherwise_block = cx.new_block();

    for arm in arms {
        let arm_block = cx.new_block();
        arm_blocks.push(arm_block);

        // Check if this arm's pattern is a literal
        if let HirPatKind::Lit(expr) = &arm.pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                targets.push((ConstVal::Int(*n), arm_block));
                continue;
            }
            if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                targets.push((ConstVal::Bool(*b), arm_block));
                continue;
            }
        }

        // Stage 3.40 (L-ENUM-MATCH): Handle enum variant patterns.
        // `Color::Red` → HirPatKind::Path(path) where path resolves to enum.
        // `Opt::Some(x)` → HirPatKind::TupleStruct(path, sub_pats).
        // Resolve the variant index and use it as the switch target.
        if is_enum {
            let variant_idx = match &arm.pat.kind {
                HirPatKind::Path(path) => {
                    // Unit variant pattern: `Color::Red`
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(cx, def_id, &path.segments[1].ident.name)
                                .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::TupleStruct(path, _) => {
                    // Tuple variant pattern: `Opt::Some(x)`
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(cx, def_id, &path.segments[1].ident.name)
                                .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::Struct(path, _, _) => {
                    // Struct variant pattern: `Shape::Circle { r: x }`
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(cx, def_id, &path.segments[1].ident.name)
                                .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(idx) = variant_idx {
                targets.push((ConstVal::Uint(idx as u128), arm_block));
                continue;
            }
        }

        // Non-literal patterns (Wild, Ident, etc.) → go to otherwise
    }

    // Terminate current block with SwitchInt
    cx.terminate(Terminator::SwitchInt {
        discr: switch_discr,
        targets: targets.clone(),
        otherwise: otherwise_block,
    });

    // Lower each arm body
    for (i, arm) in arms.iter().enumerate() {
        let arm_block = arm_blocks[i];
        cx.current_block = arm_block;

        // Collect pattern bindings (for Ident patterns)
        collect_pat_bindings_for_mir(cx, &arm.pat);
        // Stage 3.48 (L-ENUM-BINDING): generate payload-extraction projections
        // for enum tuple/struct variant patterns (e.g., `Opt::Some(x)`).
        // Before this fix, the binding `x` was never assigned — reading
        // uninitialized memory (P0 soundness bug).
        lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);

        // Lower the arm body
        let arm_result = lower_expr_to_operand(cx, &arm.body);
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
            arm.span,
        );
        cx.terminate(Terminator::Goto(cont_block));
    }

    // Lower the otherwise block (for non-literal patterns)
    cx.current_block = otherwise_block;
    // Find the first arm with a non-literal pattern
    for arm in arms {
        let is_literal = matches!(&arm.pat.kind, HirPatKind::Lit(_));
        if !is_literal {
            collect_pat_bindings_for_mir(cx, &arm.pat);
            // Stage 3.48 (L-ENUM-BINDING): same as above, for the otherwise arm.
            lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body);
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            break;
        }
    }
    cx.terminate(Terminator::Goto(cont_block));

    // Continuation
    cx.current_block = cont_block;
    result_local
}
