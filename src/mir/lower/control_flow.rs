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
/// Stage 14.50: Recursively destructure a nested pattern from a source local.
///
/// Given a source local holding a value and a pattern, if the pattern is a
/// Struct or Tuple, extract fields and recursively destructure nested patterns.
/// For Ident patterns, the local has already been created by the caller —
/// this function does nothing.
///
/// Per §13.4: handles nested struct-in-struct, tuple-in-struct, struct-in-tuple,
/// and tuple-in-tuple patterns to any depth.
fn lower_nested_pattern_destructure(cx: &mut MirLowerCtxt, src_local: LocalId, pat: &HirPat) {
    match &pat.kind {
        HirPatKind::Struct(path, fields, _) => {
            // Resolve the struct DefId to look up field indices
            if let crate::hir::Res::Def(struct_def_id, _) = path.res {
                let field_indices: std::collections::HashMap<crate::lexer::token::Symbol, usize> = {
                    let mut map = std::collections::HashMap::new();
                    if let Some(hir) = cx.hir {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.owner(struct_def_id)
                        {
                            for (i, f) in s.fields.iter().enumerate() {
                                if let Some(name) = f.ident {
                                    map.insert(name.name, i);
                                }
                            }
                        }
                    }
                    map
                };
                for field_pat in fields {
                    if let Some(field_idx) = field_indices.get(&field_pat.ident.name).copied() {
                        let field_ty = cx.fresh_infer_ty(field_pat.pat.span);
                        let sub_local = cx.new_local(field_pat.pat.hir_id, field_ty.clone(), None);
                        cx.mir
                            .block_mut(cx.current_block)
                            .statements
                            .push(Statement {
                                kind: StatementKind::StorageLive(sub_local),
                                span: field_pat.pat.span,
                            });
                        cx.push_assign(
                            Place::local(sub_local, field_pat.pat.span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(src_local, pat.span)),
                                    ProjectionElem::Field(FieldId(field_idx as u32), field_ty),
                                ),
                                span: field_pat.pat.span,
                            })),
                            field_pat.pat.span,
                        );
                        // Recurse for nested patterns
                        lower_nested_pattern_destructure(cx, sub_local, &field_pat.pat);
                    }
                }
            }
        }
        HirPatKind::Tuple(sub_pats) => {
            // Use the existing nested tuple destructure helper
            lower_nested_tuple_destructure(cx, src_local, sub_pats, pat.span);
        }
        _ => {
            // Ident or other pattern — local already created by caller
        }
    }
}

/// Stage 14.49: Recursively destructure a nested tuple pattern.
///
/// Given a source local holding a tuple value and a list of sub-patterns,
/// extract each field and bind it. If a sub-pattern is itself a Tuple,
/// recurse into it.
///
/// Per §13.4: handles arbitrary nesting depth (e.g., `((a, (b, c)), d)`).
/// Per §"显式 > 隐式": field types are extracted from the source local's
/// declared type (Tuple), not inferred — ensuring correct LLVM alloca types.
fn lower_nested_tuple_destructure(
    cx: &mut MirLowerCtxt,
    src_local: LocalId,
    sub_pats: &[HirPat],
    span: Span,
) {
    // Get the source local's type to extract field types
    let src_ty = cx.mir.local(src_local).ty.clone();
    let field_tys: Vec<Ty> = match &src_ty.kind {
        TyKind::Tuple(tys) => tys.clone(),
        _ => {
            // Source is not a tuple (may be Infer) — fall back to fresh infer
            sub_pats.iter().map(|_| cx.fresh_infer_ty(span)).collect()
        }
    };
    for (field_idx, sub_pat) in sub_pats.iter().enumerate() {
        let field_ty = field_tys
            .get(field_idx)
            .cloned()
            .unwrap_or_else(|| cx.fresh_infer_ty(sub_pat.span));
        match &sub_pat.kind {
            HirPatKind::Ident(_mode, _ident, _) => {
                let sub_local = cx.new_local(sub_pat.hir_id, field_ty.clone(), None);
                cx.mir
                    .block_mut(cx.current_block)
                    .statements
                    .push(Statement {
                        kind: StatementKind::StorageLive(sub_local),
                        span: sub_pat.span,
                    });
                cx.push_assign(
                    Place::local(sub_local, sub_pat.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(src_local, span)),
                            ProjectionElem::Field(FieldId(field_idx as u32), field_ty),
                        ),
                        span: sub_pat.span,
                    })),
                    sub_pat.span,
                );
            }
            HirPatKind::Tuple(inner_sub_pats) => {
                // Recurse into nested tuple
                let inner_local = cx.new_local(sub_pat.hir_id, field_ty.clone(), None);
                cx.mir
                    .block_mut(cx.current_block)
                    .statements
                    .push(Statement {
                        kind: StatementKind::StorageLive(inner_local),
                        span: sub_pat.span,
                    });
                cx.push_assign(
                    Place::local(inner_local, sub_pat.span),
                    Rvalue::Use(Operand::Copy(Place {
                        kind: PlaceKind::Projection(
                            Box::new(Place::local(src_local, span)),
                            ProjectionElem::Field(FieldId(field_idx as u32), field_ty),
                        ),
                        span: sub_pat.span,
                    })),
                    sub_pat.span,
                );
                lower_nested_tuple_destructure(cx, inner_local, inner_sub_pats, sub_pat.span);
            }
            _ => {}
        }
    }
}

/// Lower a block: lower all statements, then
/// evaluates the trailing expression (if any). Returns the LocalId
/// of the block's result value.
pub(crate) fn lower_block(cx: &mut MirLowerCtxt, block: &HirBlock) -> LocalId {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Local(local) => {
                // Lower the init expression first (if present)
                if let Some(init) = &local.init {
                    let init_local = lower_expr_to_operand(cx, init);

                    // Stage 14.46: Handle tuple destructuring patterns.
                    // `let (a, b, c) = (10, 20, 30)` should:
                    // 1. Create locals for a, b, c
                    // 2. Extract each field from the init tuple and assign
                    //
                    // Per §13.4 (design alignment): Rust's tuple destructuring
                    // creates separate bindings for each sub-pattern. The
                    // previous code only created ONE local (for the tuple
                    // pattern's hir_id) and assigned the whole tuple to it —
                    // the individual bindings a, b, c were never created,
                    // causing them to resolve to Error/0.
                    if let HirPatKind::Tuple(sub_pats) = &local.pat.kind {
                        // Create a temp local for the whole tuple
                        let tuple_ty = cx.mir.local(init_local).ty.clone();
                        let tuple_local = cx.new_local(local.pat.hir_id, tuple_ty.clone(), None);
                        cx.mir
                            .block_mut(cx.current_block)
                            .statements
                            .push(Statement {
                                kind: StatementKind::StorageLive(tuple_local),
                                span: local.span,
                            });
                        // Assign the init tuple to the temp local
                        let init_ty = cx.mir.local(init_local).ty.clone();
                        let is_copy = matches!(
                            &init_ty.kind,
                            crate::mir::ty::TyKind::Bool
                                | crate::mir::ty::TyKind::Char
                                | crate::mir::ty::TyKind::Int(_)
                                | crate::mir::ty::TyKind::Uint(_)
                                | crate::mir::ty::TyKind::Float(_)
                                | crate::mir::ty::TyKind::Ref(_, _, _)
                                | crate::mir::ty::TyKind::RawPtr(_, _)
                                | crate::mir::ty::TyKind::FnDef(_, _)
                                | crate::mir::ty::TyKind::FnPtr(_)
                                | crate::mir::ty::TyKind::Never
                                | crate::mir::ty::TyKind::Infer(_)
                                | crate::mir::ty::TyKind::Error
                                | crate::mir::ty::TyKind::Foreign
                        );
                        let operand = if is_copy {
                            Operand::Copy(Place::local(init_local, init.span))
                        } else {
                            Operand::Move(Place::local(init_local, init.span))
                        };
                        cx.push_assign(
                            Place::local(tuple_local, local.span),
                            Rvalue::Use(operand),
                            local.span,
                        );
                        // Now create locals for each sub-pattern and extract fields
                        for (field_idx, sub_pat) in sub_pats.iter().enumerate() {
                            if let HirPatKind::Ident(_mode, _ident, _) = &sub_pat.kind {
                                let field_ty = cx.fresh_infer_ty(sub_pat.span);
                                let sub_local =
                                    cx.new_local(sub_pat.hir_id, field_ty.clone(), None);
                                cx.mir
                                    .block_mut(cx.current_block)
                                    .statements
                                    .push(Statement {
                                        kind: StatementKind::StorageLive(sub_local),
                                        span: sub_pat.span,
                                    });
                                // Extract field from the tuple: tuple_local.field_idx
                                cx.push_assign(
                                    Place::local(sub_local, sub_pat.span),
                                    Rvalue::Use(Operand::Copy(Place {
                                        kind: PlaceKind::Projection(
                                            Box::new(Place::local(tuple_local, local.span)),
                                            ProjectionElem::Field(
                                                FieldId(field_idx as u32),
                                                field_ty,
                                            ),
                                        ),
                                        span: sub_pat.span,
                                    })),
                                    sub_pat.span,
                                );
                            } else if let HirPatKind::Tuple(inner_sub_pats) = &sub_pat.kind {
                                // Stage 14.49: Nested tuple destructure.
                                // `let ((a, b), c) = ((1, 2), 3)` — the sub-pattern
                                // at field_idx is itself a Tuple `(a, b)`.
                                // Extract the inner tuple to a temp local, then
                                // recursively destructure it.
                                //
                                // Per §"显式 > 隐式": use the field type from the
                                // outer tuple's type (not fresh_infer_ty) so the
                                // inner local gets the correct Tuple type.
                                let outer_ty = cx.mir.local(tuple_local).ty.clone();
                                let inner_ty = match &outer_ty.kind {
                                    TyKind::Tuple(tys) => tys
                                        .get(field_idx)
                                        .cloned()
                                        .unwrap_or_else(|| cx.fresh_infer_ty(sub_pat.span)),
                                    _ => cx.fresh_infer_ty(sub_pat.span),
                                };
                                let inner_local =
                                    cx.new_local(sub_pat.hir_id, inner_ty.clone(), None);
                                cx.mir
                                    .block_mut(cx.current_block)
                                    .statements
                                    .push(Statement {
                                        kind: StatementKind::StorageLive(inner_local),
                                        span: sub_pat.span,
                                    });
                                // Extract the inner tuple from the outer tuple
                                cx.push_assign(
                                    Place::local(inner_local, sub_pat.span),
                                    Rvalue::Use(Operand::Copy(Place {
                                        kind: PlaceKind::Projection(
                                            Box::new(Place::local(tuple_local, local.span)),
                                            ProjectionElem::Field(
                                                FieldId(field_idx as u32),
                                                inner_ty,
                                            ),
                                        ),
                                        span: sub_pat.span,
                                    })),
                                    sub_pat.span,
                                );
                                // Recursively destructure the inner tuple
                                lower_nested_tuple_destructure(
                                    cx,
                                    inner_local,
                                    inner_sub_pats,
                                    sub_pat.span,
                                );
                            }
                        }
                        // Skip the normal single-local path
                        continue;
                    }

                    // Stage 14.48: Handle struct destructuring patterns.
                    // `let Point { x, y } = p` should:
                    // 1. Create a temp local for the whole struct
                    // 2. For each field pattern, create a local and extract the field
                    //
                    // Per §13.4: mirrors tuple destructuring but uses field NAMES
                    // (looked up from HIR struct definition) instead of indices.
                    if let HirPatKind::Struct(path, fields, _has_rest) = &local.pat.kind {
                        // Resolve the struct DefId to look up field indices
                        if let crate::hir::Res::Def(struct_def_id, _) = path.res {
                            // Get field names from HIR to compute indices
                            let field_indices: std::collections::HashMap<
                                crate::lexer::token::Symbol,
                                usize,
                            > = {
                                let mut map = std::collections::HashMap::new();
                                if let Some(hir) = cx.hir {
                                    if let Some(crate::hir::OwnerNode::Item(
                                        crate::hir::HirItem::Struct(s),
                                    )) = hir.owner(struct_def_id)
                                    {
                                        for (i, f) in s.fields.iter().enumerate() {
                                            if let Some(name) = f.ident {
                                                map.insert(name.name, i);
                                            }
                                        }
                                    }
                                }
                                map
                            };
                            // Create a temp local for the whole struct
                            let struct_ty = cx.mir.local(init_local).ty.clone();
                            let struct_local =
                                cx.new_local(local.pat.hir_id, struct_ty.clone(), None);
                            cx.mir
                                .block_mut(cx.current_block)
                                .statements
                                .push(Statement {
                                    kind: StatementKind::StorageLive(struct_local),
                                    span: local.span,
                                });
                            // Assign the init struct to the temp local
                            cx.push_assign(
                                Place::local(struct_local, local.span),
                                Rvalue::Use(Operand::Copy(Place::local(init_local, init.span))),
                                local.span,
                            );
                            // For each field pattern, create a local and extract
                            for field_pat in fields {
                                if let Some(field_idx) =
                                    field_indices.get(&field_pat.ident.name).copied()
                                {
                                    let field_ty = cx.fresh_infer_ty(field_pat.pat.span);
                                    // Create a temp local for the field value
                                    let sub_local =
                                        cx.new_local(field_pat.pat.hir_id, field_ty.clone(), None);
                                    cx.mir
                                        .block_mut(cx.current_block)
                                        .statements
                                        .push(Statement {
                                            kind: StatementKind::StorageLive(sub_local),
                                            span: field_pat.pat.span,
                                        });
                                    // Extract the field from the struct
                                    cx.push_assign(
                                        Place::local(sub_local, field_pat.pat.span),
                                        Rvalue::Use(Operand::Copy(Place {
                                            kind: PlaceKind::Projection(
                                                Box::new(Place::local(struct_local, local.span)),
                                                ProjectionElem::Field(
                                                    FieldId(field_idx as u32),
                                                    field_ty,
                                                ),
                                            ),
                                            span: field_pat.pat.span,
                                        })),
                                        field_pat.pat.span,
                                    );
                                    // Stage 14.50: Handle nested patterns within struct fields.
                                    // If the field pattern is itself a Struct or Tuple,
                                    // recursively destructure it from the extracted local.
                                    lower_nested_pattern_destructure(cx, sub_local, &field_pat.pat);
                                }
                            }
                            // Skip the normal single-local path
                            continue;
                        }
                    }

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
                    //
                    // Stage 13.25 fix: Use Copy for Copy types, Move for non-Copy.
                    // Before Stage 13.25, this always used Operand::Move, which
                    // marked the init local as moved — breaking subsequent uses
                    // of Copy types (e.g., `let x = i; i += 1;` where i is i32).
                    // Now we check the type: if it's Copy (i32, bool, etc.), use
                    // Operand::Copy (no move recorded); otherwise use Move.
                    let init_ty = cx.mir.local(init_local).ty.clone();
                    let is_copy = matches!(
                        &init_ty.kind,
                        crate::mir::ty::TyKind::Bool
                            | crate::mir::ty::TyKind::Char
                            | crate::mir::ty::TyKind::Int(_)
                            | crate::mir::ty::TyKind::Uint(_)
                            | crate::mir::ty::TyKind::Float(_)
                            | crate::mir::ty::TyKind::Ref(_, _, _)
                            | crate::mir::ty::TyKind::RawPtr(_, _)
                            | crate::mir::ty::TyKind::FnDef(_, _)
                            | crate::mir::ty::TyKind::FnPtr(_)
                            | crate::mir::ty::TyKind::Never
                            | crate::mir::ty::TyKind::Infer(_)
                            | crate::mir::ty::TyKind::Error
                            | crate::mir::ty::TyKind::Foreign
                    );
                    let operand = if is_copy {
                        Operand::Copy(Place::local(init_local, init.span))
                    } else {
                        Operand::Move(Place::local(init_local, init.span))
                    };
                    cx.push_assign(
                        Place::local(local_id, local.span),
                        Rvalue::Use(operand),
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
        // Stage 14.22: Check if the last statement diverges (return with value,
        // break, continue). If so, the block type is Never (control flow
        // doesn't reach here). This allows `fn f() -> i32 { return 42; }` to
        // typecheck (Never unifies with i32).
        // Note: `return;` (no value) is NOT treated as diverging here — typeck
        // will catch the mismatch when the function expects i32 but return
        // provides ().
        let last_diverges = block.stmts.iter().rev().any(|stmt| {
            if let HirStmt::Expr(e, _) = stmt {
                matches!(
                    &e.kind,
                    HirExprKind::Return { expr: Some(_), .. }
                        | HirExprKind::Break { .. }
                        | HirExprKind::Continue
                )
            } else {
                false
            }
        });
        if last_diverges {
            // Block diverges — return Never type (unifies with anything)
            cx.mir
                .new_local(Ty::new(TyKind::Never, block.span), None, block.span)
        } else {
            // No trailing expr, no divergence → unit
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
                Ty::new(TyKind::Tuple(vec![]), block.span),
                block.span,
            )
        }
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
        // Stage 14.66: If scrut_local is a Ref (e.g., `match self { ... }`
        // where `self: &Self`), dereference it before extracting the discriminant.
        let scrut_place = {
            let scrut_ty = cx.mir.local(scrut_local).ty.clone();
            if matches!(scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(scrut_local, scrutinee.span)),
                        ProjectionElem::Deref,
                    ),
                    span,
                }
            } else {
                Place::local(scrut_local, scrutinee.span)
            }
        };
        // Create a temp local for the extracted discriminant.
        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
        cx.push_assign(
            Place::local(discr_local, span),
            Rvalue::Use(Operand::Move(Place {
                kind: PlaceKind::Projection(
                    Box::new(scrut_place),
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

        // Stage 14.45: Handle Or-patterns (e.g., `1 | 2 => { ... }`).
        // Each literal sub-pattern becomes a switch case pointing to the
        // SAME arm_block. Non-literal sub-patterns are not supported in
        // Or-patterns (deferred — would need more complex lowering).
        //
        // Per §13.4 (design alignment): Rust's Or-pattern semantics require
        // all sub-patterns to match the same binding set. For literals, this
        // is straightforward — each is a separate switch case.
        // Per §"报错 > 静默": if a sub-pattern is non-literal, we emit a
        // compile error instead of silently falling through to otherwise.
        if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
            let mut all_lit = true;
            for sub_pat in sub_pats {
                if let HirPatKind::Lit(expr) = &sub_pat.kind {
                    if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                        targets.push((ConstVal::Int(*n), arm_block));
                        continue;
                    }
                    if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                        targets.push((ConstVal::Bool(*b), arm_block));
                        continue;
                    }
                }
                // Non-literal sub-pattern in Or — not supported yet.
                all_lit = false;
            }
            if all_lit {
                continue;
            }
            // Fall through to otherwise (will execute arm body for any value —
            // this is a known limitation for non-literal Or sub-patterns).
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

    // Stage 14.47: If there are no switch targets (no literal/enum patterns),
    // don't emit a SwitchInt — just goto the otherwise block directly.
    // This handles `match t { (a, b) => ... }` where the pattern is a tuple
    // destructure (not a literal/enum). Was: emitted SwitchInt on a tuple
    // scrutinee → "expected integer or bool for switch, found Tuple" typeck error.
    if targets.is_empty() {
        cx.terminate(Terminator::Goto(otherwise_block));
    } else {
        // Terminate current block with SwitchInt
        cx.terminate(Terminator::SwitchInt {
            discr: switch_discr,
            targets: targets.clone(),
            otherwise: otherwise_block,
        });
    }

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
        // Stage 14.34: If the arm body diverged (e.g. `return` inside match arm),
        // the current block is already terminated. Skip the assignment + Goto.
        if !cx.is_terminated() {
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            cx.terminate(Terminator::Goto(cont_block));
        }
    }

    // Lower the otherwise block (for non-literal patterns)
    // Stage 14.67: Handle tuple patterns with literal sub-patterns by
    // generating conditional checks (if-else chain).
    //
    // For `match p { (0, 0) => 0, (0, _) => 1, (_, 0) => 2, (a, b) => ... }`:
    // - (0, 0): check p.0 == 0 AND p.1 == 0
    // - (0, _): check p.0 == 0 (p.1 is wildcard, always matches)
    // - (_, 0): check p.1 == 0 (p.0 is wildcard)
    // - (a, b): no check (binding, always matches) — catch-all
    //
    // Each arm with a tuple pattern generates a conditional block. If the
    // condition matches, execute the arm body; otherwise, fall through to
    // the next arm.
    cx.current_block = otherwise_block;
    let mut fallthrough_block = otherwise_block;
    for arm in arms {
        let is_literal = matches!(&arm.pat.kind, HirPatKind::Lit(_));
        // Stage 14.45: Or-pattern with all-literal sub-patterns is already
        // handled as switch cases — treat as "literal" for otherwise purposes.
        let is_or_all_lit = if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
            sub_pats
                .iter()
                .all(|sp| matches!(&sp.kind, HirPatKind::Lit(_)))
        } else {
            false
        };
        // Stage 14.75: Enum variant patterns (Path, TupleStruct, Struct)
        // that resolve to enum variants are already handled as switch cases
        // above. Skip them in the otherwise block — otherwise their body
        // would be executed as a catch-all, causing wrong behavior.
        let is_enum_variant = matches!(
            &arm.pat.kind,
            HirPatKind::Path(_) | HirPatKind::TupleStruct(_, _) | HirPatKind::Struct(_, _, _)
        ) && is_enum;
        if is_literal || is_or_all_lit || is_enum_variant {
            continue;
        }

        // Stage 14.67: For tuple patterns with literal sub-patterns,
        // generate a conditional check.
        let has_tuple_lit = matches!(&arm.pat.kind, HirPatKind::Tuple(_));
        if has_tuple_lit {
            // Generate condition: AND of all literal sub-field checks
            let next_block = cx.new_block();
            let match_block = cx.new_block();

            // Build the condition by checking each literal sub-pattern
            cx.current_block = fallthrough_block;
            if let HirPatKind::Tuple(sub_pats) = &arm.pat.kind {
                build_tuple_pattern_condition(cx, scrut_local, sub_pats, match_block, next_block);
            }
            // match_block: pattern matched — execute arm body
            cx.current_block = match_block;
            collect_pat_bindings_for_mir(cx, &arm.pat);
            lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body);
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            cx.terminate(Terminator::Goto(cont_block));
            fallthrough_block = next_block;
            continue;
        }

        // Non-tuple, non-literal pattern (Wild, Ident, etc.) — catch-all
        cx.current_block = fallthrough_block;
        collect_pat_bindings_for_mir(cx, &arm.pat);
        lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
        let arm_result = lower_expr_to_operand(cx, &arm.body);
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
            arm.span,
        );
        // Stage 14.71: Don't reset cx.current_block to fallthrough_block
        // after the loop — lower_expr_to_operand may have created new blocks
        // (e.g., overflow checks), and cx.current_block points to the LAST
        // block. Resetting to fallthrough_block would overwrite the original
        // block with a Goto, orphaning the overflow check blocks.
        // Instead, terminate the CURRENT block (the last one from
        // lower_expr_to_operand) with Goto(cont_block).
        if !cx.is_terminated() {
            cx.terminate(Terminator::Goto(cont_block));
        }
        break;
    }
    // Stage 14.71: Only terminate fallthrough_block if no catch-all was found.
    // If a catch-all was found, the loop already terminated the last block.
    // If no catch-all, fallthrough_block needs a terminator.
    // Check if fallthrough_block is the same as cx.current_block — if so,
    // it hasn't been terminated yet (no catch-all found).
    if !cx.is_terminated() {
        cx.terminate(Terminator::Goto(cont_block));
    }

    // Continuation
    cx.current_block = cont_block;
    result_local
}

/// Stage 14.67: Build a conditional check for a tuple pattern.
///
/// For each literal sub-pattern at index `i`, check `scrut.i == literal`.
/// Wildcard sub-patterns (`_`) are skipped (always match).
/// Ident sub-patterns (bindings) are skipped (always match, binding done later).
///
/// Generates an if-else chain: if first check fails, goto next_block;
/// if all checks pass, goto match_block.
fn build_tuple_pattern_condition(
    cx: &mut MirLowerCtxt,
    scrut_local: LocalId,
    sub_pats: &[HirPat],
    match_block: BasicBlockId,
    next_block: BasicBlockId,
) {
    use crate::mir::body::StatementKind;
    use crate::mir::place::{BinOp, Operand, Place, PlaceKind, ProjectionElem, Rvalue};

    let span = crate::session::Span::DUMMY;
    let mut current = cx.current_block;

    for (i, sub_pat) in sub_pats.iter().enumerate() {
        // Only check literal sub-patterns
        let lit_val = if let HirPatKind::Lit(expr) = &sub_pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                Some(*n)
            } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                // Bool literals — compare as i32 (0 or 1)
                Some(if *b { 1 } else { 0 })
            } else {
                None
            }
        } else {
            None
        };

        if let Some(n) = lit_val {
            // Extract field i from scrutinee: scrut.i
            let field_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
            let field_local = cx.mir.new_local(field_ty.clone(), None, span);
            cx.mir.block_mut(current).statements.push(Statement {
                kind: StatementKind::StorageLive(field_local),
                span,
            });
            // push_assign uses current_block, so set it to `current`
            cx.current_block = current;
            cx.push_assign(
                Place::local(field_local, span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(Place::local(scrut_local, span)),
                        ProjectionElem::Field(FieldId(i as u32), field_ty.clone()),
                    ),
                    span,
                })),
                span,
            );
            current = cx.current_block;

            // Compare: field_local == n
            let cmp_result = cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
            cx.mir.block_mut(current).statements.push(Statement {
                kind: StatementKind::StorageLive(cmp_result),
                span,
            });
            cx.current_block = current;
            cx.push_assign(
                Place::local(cmp_result, span),
                Rvalue::BinaryOp(
                    BinOp::Eq,
                    Operand::Copy(Place::local(field_local, span)),
                    Operand::Constant(crate::mir::ty::Const {
                        ty: Box::new(field_ty),
                        val: crate::mir::ty::ConstVal::Int(n),
                    }),
                ),
                span,
            );
            current = cx.current_block;

            // Switch on cmp_result: if true, continue to next check (or match);
            // if false, goto next_block (pattern didn't match)
            let continue_block = cx.new_block();
            cx.mir.block_mut(current).terminator = Terminator::SwitchInt {
                discr: Operand::Copy(Place::local(cmp_result, span)),
                targets: vec![(ConstVal::Bool(true), continue_block)],
                otherwise: next_block,
            };
            current = continue_block;
        }
        // Wildcard or Ident — skip (always matches)
    }

    // All checks passed — goto match_block
    cx.mir.block_mut(current).terminator = Terminator::Goto(match_block);
    cx.current_block = current;
}
