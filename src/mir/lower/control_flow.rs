//! Stage 6.6: Control flow lowering extraction from mir/lower/mod.rs (TD-011 split step 6).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (2452 → ~1980).
//! Contains functions for lowering control flow constructs: short-circuit
//! evaluation, deref expressions, blocks, if/match expressions.

use crate::hir::*;
use crate::mir::body::TerminatorKind;
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
    let lhs_local = lower_expr_to_operand(cx, lhs, None);
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
    cx.terminate_kind(TerminatorKind::SwitchInt {
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
            ty: Ty::new(TyKind::Bool, Span::DUMMY),
            val: ConstVal::Bool(short_val),
        })),
        span,
    );
    cx.terminate_kind(TerminatorKind::Goto(cont_block));

    // eval_rhs_block: evaluate rhs, switchInt(rhs) → {true: result_true, _: result_false}
    cx.current_block = eval_rhs_block;
    let rhs_local = lower_expr_to_operand(cx, rhs, None);
    cx.terminate_kind(TerminatorKind::SwitchInt {
        discr: Operand::Copy(Place::local(rhs_local, rhs.span)),
        targets: vec![(ConstVal::Bool(true), result_true_block)],
        otherwise: result_false_block,
    });

    // result_true_block: result = true; goto cont
    cx.current_block = result_true_block;
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Ty::new(TyKind::Bool, Span::DUMMY),
            val: ConstVal::Bool(true),
        })),
        span,
    );
    cx.terminate_kind(TerminatorKind::Goto(cont_block));

    // result_false_block: result = false; goto cont
    cx.current_block = result_false_block;
    cx.push_assign(
        Place::local(result_local, span),
        Rvalue::Use(Operand::Constant(Const {
            ty: Ty::new(TyKind::Bool, Span::DUMMY),
            val: ConstVal::Bool(false),
        })),
        span,
    );
    cx.terminate_kind(TerminatorKind::Goto(cont_block));

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
    let inner_local = lower_expr_to_operand(cx, inner, None);
    let proj = Place {
        kind: PlaceKind::Projection(
            Box::new(Place::local(inner_local, inner.span)),
            ProjectionElem::Deref,
        ),
        span,
    };
    // Stage 15.75: Use the inner local's type to resolve the deref result type.
    // If the inner is a reference (&T / &mut T), the result is T.
    // This avoids creating a fresh Infer type that stays unresolved at borrowck
    // time (writeback runs after borrowck), which caused "does not implement
    // Copy" errors for dereferenced non-Copy types.
    let inner_ty = cx.mir.local(inner_local).ty.clone();
    let result_ty = match &inner_ty.kind {
        TyKind::Ref(_, _, inner) => (**inner).clone(),
        TyKind::RawPtr(_, inner) => (**inner).clone(),
        _ => cx.fresh_infer_ty(span),
    };
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
                            hir.find_owner(struct_def_id)
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
                // Stage 18.257 (TD-TUPLE-CTOR-TYPECK Phase 2b): thread
                // expected_ty from `let : T = expr` annotation into the init
                // expression's lowering. This enables the ctor path to
                // extract substs from the expected type when turbofish is
                // absent (Phase 2c will use this in lower_call_expr).
                //
                // Per §13.4 J3 (one-way flow): expected_ty flows from let
                // annotation → lower_expr_to_operand → lower_call_expr →
                // arg operands. No back-edges.
                // Per §1.0 原則 3 (显式 > 隐式): the expected type is
                // explicit in the source (`let : T = ...`) and should be
                // propagated explicitly through MIR lower.
                //
                // NOTE: For tuple destructuring patterns (e.g.,
                // `let (a, b) = (1, 2)`), we don't have a single expected
                // type — each sub-pattern has its own. Phase 2b only
                // threads expected_ty for simple ident patterns; tuple
                // destructure remains using None (Phase 2e+ may extend).
                let expected_ty: Option<Ty> = if let Some(ty) = &local.ty {
                    if matches!(&local.pat.kind, HirPatKind::Ident(..)) {
                        Some(super::lower_hir_ty_to_mir_ty_with_hir(ty, cx.hir))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Lower the init expression first (if present)
                if let Some(init) = &local.init {
                    let init_local = lower_expr_to_operand(cx, init, expected_ty.as_ref());

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

                    // Stage 14.89 (Bug 1 fix): Handle tuple struct destructuring
                    // patterns. `let Pair(a, b) = Pair(10, 20)` should:
                    // 1. Create a temp local for the whole struct
                    // 2. For each positional field pattern, create a local and
                    //    extract the field by index
                    //
                    // Per §13.4: mirrors tuple destructuring but for tuple structs
                    // (positional fields, not named). Previously, tuple struct
                    // patterns fell through to the single-local path, creating
                    // ONE local for the whole struct — individual bindings a, b
                    // were never created, resolving to Error/0.
                    if let HirPatKind::TupleStruct(path, sub_pats) = &local.pat.kind {
                        // Only handle plain structs (not enum tuple variants — those
                        // are handled by the enum match path)
                        if let crate::hir::Res::Def(_struct_def_id, def_kind) = path.res {
                            if def_kind == crate::resolve::DefKind::Struct {
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
                                // For each positional field pattern, create a local and extract
                                for (i, sub_pat) in sub_pats.iter().enumerate() {
                                    let field_ty = cx.fresh_infer_ty(sub_pat.span);
                                    // Create a temp local for the field value
                                    let sub_local =
                                        cx.new_local(sub_pat.hir_id, field_ty.clone(), None);
                                    cx.mir
                                        .block_mut(cx.current_block)
                                        .statements
                                        .push(Statement {
                                            kind: StatementKind::StorageLive(sub_local),
                                            span: sub_pat.span,
                                        });
                                    // Extract the field from the struct by positional index
                                    cx.push_assign(
                                        Place::local(sub_local, sub_pat.span),
                                        Rvalue::Use(Operand::Copy(Place {
                                            kind: PlaceKind::Projection(
                                                Box::new(Place::local(struct_local, local.span)),
                                                ProjectionElem::Field(FieldId(i as u32), field_ty),
                                            ),
                                            span: sub_pat.span,
                                        })),
                                        sub_pat.span,
                                    );
                                    // Handle nested patterns within tuple struct fields
                                    lower_nested_pattern_destructure(cx, sub_local, sub_pat);
                                }
                                // Skip the normal single-local path
                                continue;
                            }
                        }
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
                                    )) = hir.find_owner(struct_def_id)
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
                    //
                    // Stage 15.73: If there's no annotation, use the init
                    // expression's type directly (from the init_local's
                    // local_decl). This avoids creating a fresh Infer type
                    // that remains unresolved at borrowck time (writeback
                    // runs AFTER borrowck). This fixes struct/enum move
                    // errors where `let s2 = s` used Operand::Copy because
                    // s2's type was Infer (treated as Copy).
                    let ty = match &local.ty {
                        Some(t) => super::lower_hir_ty_to_mir_ty_with_hir(t, cx.hir),
                        None => {
                            // Use the init_local's type if it's not Infer.
                            let init_ty = cx.mir.local(init_local).ty.clone();
                            if matches!(init_ty.kind, crate::mir::ty::TyKind::Infer(_)) {
                                cx.fresh_infer_ty(local.span)
                            } else {
                                init_ty
                            }
                        }
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
                    // Stage 16.06: Always use Operand::Move for let bindings.
                    // Previously (Stage 13.25), this used Copy for Copy types
                    // and Move for non-Copy types, based on the init local's
                    // type AT LOWERING TIME. But the type may be Infer at
                    // lowering time and only resolved to Adt (non-Copy) after
                    // typeck writeback. This caused "does not implement Copy"
                    // errors for let bindings of non-Copy structs (e.g.,
                    // `let c = make_counter()` where Counter has impl Drop).
                    //
                    // The borrow checker's Operand::Move path (Stage 15.73)
                    // already skips move recording for Copy types, so using
                    // Move is safe for both Copy and non-Copy types:
                    //   - Copy type: no move recorded (source remains valid)
                    //   - Non-Copy type: move recorded (ownership transferred)
                    //
                    // Per §1.0 原則 9 "正确 > 妥协": always-Move is the sound
                    // default. The borrow checker's `is_copy` (sound, via
                    // TraitResolver) decides whether to record the move.
                    let operand = Operand::Move(Place::local(init_local, init.span));
                    cx.push_assign(
                        Place::local(local_id, local.span),
                        Rvalue::Use(operand),
                        local.span,
                    );

                    // Stage 16.34 (Task 10 Step 5 — cleanup): Removed the
                    // `closure_bodies` side-table propagation. The closure
                    // dispatch at the call site now uses the type-based
                    // check (`TyKind::Closure(_, _)`) instead of the
                    // side-table lookup. The let_local's type is already
                    // set from init_local's type (line 598-604 above), so
                    // if init_local is a closure, let_local's type is
                    // `Closure(def_id, substs)` — no side-table needed.
                    //
                    // Per §1.0 原則 5 "去除兼容思维": dead side-table removed.
                    // Per §23 rule 5 (DRY): type is the single source of truth.
                } else {
                    // No init: just allocate the local. If a type annotation
                    // is present, use it; otherwise fresh Infer var.
                    let ty = match &local.ty {
                        Some(t) => super::lower_hir_ty_to_mir_ty_with_hir(t, cx.hir),
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
                lower_expr_to_operand(cx, e, None);
            }
            _ => {}
        }
    }
    // Trailing expression
    if let Some(expr) = &block.expr {
        lower_expr_to_operand(cx, expr, None)
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
    let cond_local = lower_expr_to_operand(cx, cond, None);
    let then_block = cx.new_block();
    let else_block = cx.new_block();
    let cont_block = cx.new_block();
    let result_ty = cx.fresh_infer_ty(span);
    let result_local =
        cx.mir
            .new_local_with_mut(result_ty, None, span, crate::mir::ty::Mutability::Mutable);

    cx.terminate_kind(TerminatorKind::SwitchInt {
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
        cx.terminate_kind(TerminatorKind::Goto(cont_block));
    }

    // Else block
    cx.current_block = else_block;
    if let Some(else_expr) = else_ {
        let else_result = lower_expr_to_operand(cx, else_expr, None);
        if !cx.is_terminated() {
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(else_result, else_expr.span))),
                else_expr.span,
            );
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
        }
    } else {
        cx.push_assign(
            Place::local(result_local, span),
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
            span,
        );
        cx.terminate_kind(TerminatorKind::Goto(cont_block));
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
    let scrut_local = lower_expr_to_operand(cx, scrutinee, None);
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
    let is_enum = matches!(&scrut_ty.kind, TyKind::Adt(def_id, _) if cx.hir.and_then(|h| h.find_owner(*def_id)).is_some_and(|o| {
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

    // Stage 14.87 (Bug A fix): Track literal values "claimed" by guarded arms.
    // When building switch targets for unguarded arms, skip any literal value
    // that was previously claimed by a guarded arm. Those values need to be
    // evaluated in the otherwise block (where the guarded arm runs first).
    //
    // Without this, `match n { 0 if n == 0 => 100, 0 => 200 }` would route
    // n=0 directly to the second arm via the switch target, bypassing the
    // guarded first arm. Per Rust semantics, arms are tried in source order,
    // so the guarded arm should run first.
    //
    // Per §1.0 原則 5 "报错 > 静默": previously the guarded arm was silently
    // skipped when an overlapping unguarded arm existed, causing wrong output.
    //
    // Note: we use a Vec + linear scan (not HashSet) because ConstVal contains
    // f64 which doesn't implement Eq/Hash. In practice only Int/Bool values
    // are stored (no Float patterns), but using Vec avoids the trait bound.
    let mut guarded_lit_values: Vec<ConstVal> = Vec::new();

    for arm in arms {
        let arm_block = cx.new_block();
        arm_blocks.push(arm_block);

        // Stage 14.86: Match arms with guards (`pat if cond => body`) must
        // NOT be added as direct switch targets — the guard condition must
        // be evaluated before the arm body runs. If we added them as switch
        // targets, the arm would match without checking the guard.
        //
        // Skip the literal/enum target push for guarded arms — they'll be
        // handled in the otherwise block (where we can evaluate both the
        // pattern match AND the guard).
        //
        // Per §1.0 原則 5 "报错 > 静默": previously guarded arms silently
        // matched without checking the guard, causing wrong runtime behavior
        // (e.g., `x if x < 10 => 1` matched even when x >= 10).
        let has_guard = arm.guard.is_some();

        // Check if this arm's pattern is a literal
        if let HirPatKind::Lit(expr) = &arm.pat.kind {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                let val = ConstVal::Int(*n);
                if has_guard {
                    // Record this literal as claimed by a guarded arm
                    guarded_lit_values.push(val);
                } else if !guarded_lit_values.contains(&val) {
                    // Not claimed — add as switch target
                    targets.push((val, arm_block));
                    continue;
                }
                // If we reach here (guarded or claimed), continue to next arm
                // (don't fall through to other pattern handlers below —
                // guarded arms are fully handled in otherwise block)
                if has_guard {
                    continue;
                }
            }
            if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                let val = ConstVal::Bool(*b);
                if has_guard {
                    guarded_lit_values.push(val);
                } else if !guarded_lit_values.contains(&val) {
                    targets.push((val, arm_block));
                    continue;
                }
                if has_guard {
                    continue;
                }
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
        //
        // Stage 14.86: Skip Or-pattern targets for guarded arms (same as
        // single-literal arms — guards must be evaluated in otherwise block).
        //
        // Stage 14.87 (Bug A fix): For unguarded Or-patterns, skip any
        // sub-pattern literal that was claimed by a previous guarded arm.
        // If all sub-pattern literals are claimed (or none are left), don't
        // add any targets — let otherwise handle the whole arm.
        if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
            if has_guard {
                // Record all literal sub-pattern values as claimed
                for sub_pat in sub_pats {
                    if let HirPatKind::Lit(expr) = &sub_pat.kind {
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            guarded_lit_values.push(ConstVal::Int(*n));
                        }
                        if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            guarded_lit_values.push(ConstVal::Bool(*b));
                        }
                    }
                }
            } else {
                // Unguarded Or-pattern: only add sub-pattern literals that
                // weren't claimed by a previous guarded arm
                let mut all_lit = true;
                let mut added_any = false;
                for sub_pat in sub_pats {
                    if let HirPatKind::Lit(expr) = &sub_pat.kind {
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            let val = ConstVal::Int(*n);
                            if !guarded_lit_values.contains(&val) {
                                targets.push((val, arm_block));
                                added_any = true;
                            }
                            continue;
                        }
                        if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            let val = ConstVal::Bool(*b);
                            if !guarded_lit_values.contains(&val) {
                                targets.push((val, arm_block));
                                added_any = true;
                            }
                            continue;
                        }
                    }
                    // Non-literal sub-pattern in Or — not supported yet.
                    all_lit = false;
                }
                if all_lit && added_any {
                    continue;
                }
                // If all_lit but !added_any (all claimed by guards) or
                // !all_lit (non-literal sub-pattern), fall through to
                // otherwise handling.
            }
        }

        // Stage 3.40 (L-ENUM-MATCH): Handle enum variant patterns.
        // `Color::Red` → HirPatKind::Path(path) where path resolves to enum.
        // `Opt::Some(x)` → HirPatKind::TupleStruct(path, sub_pats).
        // Resolve the variant index and use it as the switch target.
        //
        // Stage 14.86: Skip enum variant targets for guarded arms.
        //
        // Stage 14.87 (Bug A fix): For guarded enum variant arms, record the
        // variant index as claimed. For unguarded enum variant arms, skip
        // adding the target if the variant was claimed by a previous guarded
        // arm — those values need to be evaluated in otherwise.
        let enum_variant_idx: Option<u32> = if is_enum {
            match &arm.pat.kind {
                HirPatKind::Path(path) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::TupleStruct(path, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                HirPatKind::Struct(path, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
                            .map(|(idx, _)| idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        if let Some(idx) = enum_variant_idx {
            let val = ConstVal::Uint(idx as u128);
            // Stage 14.89 (Bug 4 fix): Check if this enum variant arm has
            // inner sub-patterns that could differentiate it from other arms
            // with the same variant. If so, don't add as a switch target —
            // handle in otherwise where inner sub-patterns can be checked.
            // This prevents duplicate switch cases AND ensures the correct
            // arm body runs (was: first arm with matching variant always won).
            let has_inner_subpatterns = match &arm.pat.kind {
                HirPatKind::TupleStruct(_, sub_pats) => sub_pats
                    .iter()
                    .any(|sp| !matches!(&sp.kind, HirPatKind::Wild)),
                HirPatKind::Struct(_, fields, _) => fields
                    .iter()
                    .any(|f| !matches!(&f.pat.kind, HirPatKind::Wild)),
                HirPatKind::Path(_) => false, // Unit variant — no payload
                _ => false,
            };
            if has_guard {
                // Record this enum variant as claimed by a guarded arm
                guarded_lit_values.push(val);
            } else if !guarded_lit_values.contains(&val) && !has_inner_subpatterns {
                // Not claimed AND no inner subpatterns — safe to add as switch target
                let already_in_targets = targets.iter().any(|(t_val, _)| *t_val == val);
                if !already_in_targets {
                    targets.push((val, arm_block));
                    continue;
                }
            } else if !guarded_lit_values.contains(&val) && has_inner_subpatterns {
                // Stage 14.89 (Bug 4 fix): Has inner subpatterns — record as
                // claimed so subsequent arms with same variant also go to otherwise.
                guarded_lit_values.push(val);
            }
            // If claimed by a guard OR has inner subpatterns OR already in
            // targets, fall through to otherwise handling
        }

        // Non-literal patterns (Wild, Ident, etc.) → go to otherwise
    }

    // Stage 14.47: If there are no switch targets (no literal/enum patterns),
    // don't emit a SwitchInt — just goto the otherwise block directly.
    // This handles `match t { (a, b) => ... }` where the pattern is a tuple
    // destructure (not a literal/enum). Was: emitted SwitchInt on a tuple
    // scrutinee → "expected integer or bool for switch, found Tuple" typeck error.
    if targets.is_empty() {
        cx.terminate_kind(TerminatorKind::Goto(otherwise_block));
    } else {
        // Terminate current block with SwitchInt
        cx.terminate_kind(TerminatorKind::SwitchInt {
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
        let arm_result = lower_expr_to_operand(cx, &arm.body, None);
        // Stage 14.34: If the arm body diverged (e.g. `return` inside match arm),
        // the current block is already terminated. Skip the assignment + Goto.
        if !cx.is_terminated() {
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
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
        let has_guard = arm.guard.is_some();
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
        // Stage 14.86: arms WITHOUT guards that are literal/Or-all-lit/enum-variant
        // were already handled as switch cases — skip them in otherwise.
        // Arms WITH guards (regardless of pattern kind) must be handled here
        // so we can evaluate the guard before running the arm body.
        //
        // Stage 14.87 (Bug A fix): An unguarded arm whose literal value was
        // "claimed" by a previous guarded arm was NOT added as a switch target.
        // It needs to be evaluated in the otherwise block (where the guarded
        // arm runs first; if guard fails, this unguarded arm should match).
        // So we DON'T skip unguarded literal/Or-all-lit arms if their values
        // were claimed by a guarded arm.
        let was_claimed = if is_literal {
            // Check if this arm's literal value is in guarded_lit_values
            if let HirPatKind::Lit(expr) = &arm.pat.kind {
                if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                    guarded_lit_values.contains(&ConstVal::Int(*n))
                } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                    guarded_lit_values.contains(&ConstVal::Bool(*b))
                } else {
                    false
                }
            } else {
                false
            }
        } else if is_or_all_lit {
            // Check if any sub-pattern literal was claimed
            if let HirPatKind::Or(sub_pats) = &arm.pat.kind {
                sub_pats.iter().any(|sp| {
                    if let HirPatKind::Lit(expr) = &sp.kind {
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            return guarded_lit_values.contains(&ConstVal::Int(*n));
                        }
                        if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            return guarded_lit_values.contains(&ConstVal::Bool(*b));
                        }
                    }
                    false
                })
            } else {
                false
            }
        } else if is_enum_variant {
            // Stage 14.87 (Bug A fix): Check if this enum variant was claimed
            // by a previous guarded arm
            let variant_idx = match &arm.pat.kind {
                HirPatKind::Path(path)
                | HirPatKind::TupleStruct(path, _)
                | HirPatKind::Struct(path, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
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
                guarded_lit_values.contains(&ConstVal::Uint(idx as u128))
            } else {
                false
            }
        } else {
            false
        };
        // Skip ONLY if: no guard AND (literal/Or-all-lit/enum-variant) AND
        // not was_claimed (i.e., this arm was added as a switch target).
        //
        // Stage 14.89 (Bug 4 fix): Also check if this enum variant was NOT
        // added as a switch target (because it was a duplicate). If it wasn't
        // added, don't skip — handle it in otherwise (check inner sub-patterns).
        let was_added_as_target = if is_enum_variant {
            // Check if this variant was added to targets
            let variant_idx = match &arm.pat.kind {
                HirPatKind::Path(path)
                | HirPatKind::TupleStruct(path, _)
                | HirPatKind::Struct(path, _, _) => {
                    if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                        if path.segments.len() >= 2 {
                            super::resolve_enum_variant(
                                cx,
                                def_id,
                                super::method_resolution::variant_name_from_path(path),
                            )
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
                let val = ConstVal::Uint(idx as u128);
                targets.iter().any(|(t_val, _)| *t_val == val)
            } else {
                false
            }
        } else {
            // For literal/Or patterns, they were added as targets if not claimed
            !was_claimed
        };
        // Stage 14.89 (Bug 4 fix): If enum variant was NOT added as target
        // (duplicate), treat as claimed so it gets handled in the
        // has_guard || was_claimed block (which checks the pattern).
        let was_claimed = was_claimed || (!was_added_as_target && is_enum_variant);
        if !has_guard && (is_literal || is_or_all_lit || is_enum_variant) && was_added_as_target {
            continue;
        }

        // Stage 14.86 + 14.87: For guarded arms (and unguarded arms whose
        // literal values were claimed by a previous guarded arm), we need
        // to:
        // 1. Check the pattern match (since they weren't added as switch
        //    targets, we need to re-check in otherwise)
        // 2. Bind pattern variables (for Ident patterns) so the guard can
        //    reference them (guarded arms only — claimed-but-unguarded arms
        //    use literal/Or/enum patterns, no Ident bindings)
        // 3. Evaluate the guard (guarded arms only)
        // 4. If both pass, run the arm body; otherwise, fall through to next arm
        //
        // Stage 14.87 (Bug A fix): previously only `has_guard` arms were
        // handled here. Now also handle `was_claimed` arms (unguarded arms
        // whose literal value was claimed by a previous guarded arm — these
        // weren't added as switch targets, so they need pattern re-check).
        if has_guard || was_claimed {
            let next_block = cx.new_block();
            let match_block = cx.new_block();

            cx.current_block = fallthrough_block;

            // Step 1: Bind pattern variables BEFORE evaluating pattern check
            // or guard. For Ident patterns, this creates the binding local
            // and assigns the scrutinee value. The guard can then reference
            // the binding.
            collect_pat_bindings_for_mir(cx, &arm.pat);
            lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            // Stage 14.77: For Ident patterns, assign scrutinee value to binding
            if let HirPatKind::Ident(_mode, _ident, _) = &arm.pat.kind {
                if let Some(binding_local) = cx.local_map.get(&arm.pat.hir_id).copied() {
                    let scrut_ty = cx.mir.local(scrut_local).ty.clone();
                    if matches!(&scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                        cx.push_assign(
                            Place::local(binding_local, arm.pat.span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(scrut_local, arm.pat.span)),
                                    ProjectionElem::Deref,
                                ),
                                span: arm.pat.span,
                            })),
                            arm.pat.span,
                        );
                    } else {
                        cx.push_assign(
                            Place::local(binding_local, arm.pat.span),
                            Rvalue::Use(Operand::Copy(Place::local(scrut_local, arm.pat.span))),
                            arm.pat.span,
                        );
                    }
                }
            }

            // Step 2: Build the pattern-match condition.
            // For literal/Or/enum patterns, generate the equality check.
            // For Tuple/TupleStruct/Struct patterns with literal sub-patterns,
            // also generate conditional checks (Stage 14.90 Bug X1 fix).
            // For Ident/Wild/other, skip (always matches).
            let needs_pattern_check = match &arm.pat.kind {
                HirPatKind::Lit(_) => true,
                HirPatKind::Or(_) => true,
                HirPatKind::Path(_)
                | HirPatKind::TupleStruct(_, _)
                | HirPatKind::Struct(_, _, _)
                    if is_enum =>
                {
                    true
                }
                // Stage 14.90 (Bug X1 fix): Tuple/TupleStruct/Struct patterns
                // with literal sub-patterns need conditional checks in
                // guarded arms. Without this, `match (1, 5) { (0, _) if true => 100, (1, _) if true => 200 }`
                // would match the first arm without checking the tuple fields.
                HirPatKind::Tuple(sub_pats)
                    if sub_pats
                        .iter()
                        .any(|sp| matches!(&sp.kind, HirPatKind::Lit(_))) =>
                {
                    true
                }
                HirPatKind::TupleStruct(_, sub_pats)
                    if !is_enum
                        && sub_pats
                            .iter()
                            .any(|sp| matches!(&sp.kind, HirPatKind::Lit(_))) =>
                {
                    true
                }
                HirPatKind::Struct(_, fields, _)
                    if fields
                        .iter()
                        .any(|f| matches!(&f.pat.kind, HirPatKind::Lit(_))) =>
                {
                    true
                }
                _ => false,
            };

            let after_pattern_check_block = if needs_pattern_check {
                // Build pattern-match check, branching to after_pattern_check_block
                // on success or next_block on failure.
                let after_pattern_check_block = cx.new_block();
                build_pattern_equality_check(
                    cx,
                    scrut_local,
                    &arm.pat,
                    is_enum,
                    after_pattern_check_block,
                    next_block,
                );
                after_pattern_check_block
            } else {
                // No pattern check needed — current block IS the after-pattern-check block
                cx.current_block
            };

            // Step 3: If arm has a guard, evaluate the guard from after_pattern_check_block.
            // If no guard (was_claimed but unguarded), skip directly to arm body.
            let body_block = if has_guard {
                cx.current_block = after_pattern_check_block;
                let guard_expr = arm.guard.as_ref().unwrap();
                let guard_local = lower_expr_to_operand(cx, guard_expr, None);
                cx.terminate_kind(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::local(guard_local, guard_expr.span)),
                    targets: vec![(ConstVal::Bool(true), match_block)],
                    otherwise: next_block,
                });
                match_block
            } else {
                // No guard — after_pattern_check_block IS where we run the arm body
                after_pattern_check_block
            };

            // body_block: pattern + guard both passed — execute arm body
            cx.current_block = body_block;
            // Note: pattern bindings already done above (before guard eval)
            let arm_result = lower_expr_to_operand(cx, &arm.body, None);
            if !cx.is_terminated() {
                cx.push_assign(
                    Place::local(result_local, span),
                    Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                    arm.span,
                );
                cx.terminate_kind(TerminatorKind::Goto(cont_block));
            }
            fallthrough_block = next_block;
            continue;
        }

        // Stage 14.67: For tuple patterns with literal sub-patterns,
        // generate a conditional check.
        let has_tuple_lit = matches!(&arm.pat.kind, HirPatKind::Tuple(_));
        // Stage 14.89 (Bug 2 fix): Also handle TupleStruct patterns with
        // literal sub-patterns (e.g., `Pair(0, _) => ...`).
        let has_tuple_struct_lit = matches!(
            &arm.pat.kind,
            HirPatKind::TupleStruct(_, sub_pats)
            if sub_pats.iter().any(|sp| matches!(&sp.kind, HirPatKind::Lit(_)))
        );
        // Stage 14.89 (Bug 3 fix): Also handle Struct patterns with literal
        // sub-patterns (e.g., `Config { mode: 0, .. } => ...`).
        let has_struct_lit = matches!(
            &arm.pat.kind,
            HirPatKind::Struct(_, fields, _)
            if fields.iter().any(|f| matches!(&f.pat.kind, HirPatKind::Lit(_)))
        );
        if has_tuple_lit || has_tuple_struct_lit || has_struct_lit {
            // Generate condition: AND of all literal sub-field checks
            let next_block = cx.new_block();
            let match_block = cx.new_block();

            // Build the condition by checking each literal sub-pattern
            cx.current_block = fallthrough_block;
            match &arm.pat.kind {
                HirPatKind::Tuple(sub_pats) => {
                    build_tuple_pattern_condition(
                        cx,
                        scrut_local,
                        sub_pats,
                        match_block,
                        next_block,
                    );
                }
                HirPatKind::TupleStruct(_, sub_pats) => {
                    // Stage 14.89: For tuple struct patterns, extract fields
                    // by positional index (same as Tuple, but the scrutinee
                    // is a struct, not a tuple).
                    build_tuple_pattern_condition(
                        cx,
                        scrut_local,
                        sub_pats,
                        match_block,
                        next_block,
                    );
                }
                HirPatKind::Struct(path, fields, _) => {
                    // Stage 14.89: For struct patterns, look up field indices
                    // by name from HIR, then check each literal sub-pattern.
                    if let crate::hir::Res::Def(struct_def_id, _) = path.res {
                        let field_indices: std::collections::HashMap<
                            crate::lexer::token::Symbol,
                            usize,
                        > = {
                            let mut map = std::collections::HashMap::new();
                            if let Some(hir) = cx.hir {
                                if let Some(crate::hir::OwnerNode::Item(
                                    crate::hir::HirItem::Struct(s),
                                )) = hir.find_owner(struct_def_id)
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
                        // Build a Vec of (field_idx, sub_pat) for literal sub-patterns
                        let mut lit_checks: Vec<(usize, &HirPat)> = Vec::new();
                        for field_pat in fields {
                            if let Some(field_idx) =
                                field_indices.get(&field_pat.ident.name).copied()
                            {
                                if matches!(&field_pat.pat.kind, HirPatKind::Lit(_)) {
                                    lit_checks.push((field_idx, &field_pat.pat));
                                }
                            }
                        }
                        // For struct patterns, build the condition inline
                        // (build_tuple_pattern_condition expects positional indices)
                        let mut current = cx.current_block;
                        let span = arm.pat.span;
                        for (field_idx, lit_pat) in &lit_checks {
                            if let HirPatKind::Lit(expr) = &lit_pat.kind {
                                let lit_val = if let HirExprKind::Lit(HirLitKind::Int(n, _)) =
                                    &expr.kind
                                {
                                    Some(*n)
                                } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                                    Some(if *b { 1 } else { 0 })
                                } else {
                                    None
                                };
                                if let Some(n) = lit_val {
                                    // Extract field from scrutinee
                                    let field_ty =
                                        Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                                    let field_local =
                                        cx.mir.new_local(field_ty.clone(), None, span);
                                    cx.mir.block_mut(current).statements.push(Statement {
                                        kind: StatementKind::StorageLive(field_local),
                                        span,
                                    });
                                    cx.current_block = current;
                                    cx.push_assign(
                                        Place::local(field_local, span),
                                        Rvalue::Use(Operand::Copy(Place {
                                            kind: PlaceKind::Projection(
                                                Box::new(Place::local(scrut_local, span)),
                                                ProjectionElem::Field(
                                                    FieldId(*field_idx as u32),
                                                    field_ty.clone(),
                                                ),
                                            ),
                                            span,
                                        })),
                                        span,
                                    );
                                    current = cx.current_block;
                                    // Compare field == n
                                    let cmp_result =
                                        cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
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
                                                ty: field_ty,
                                                val: crate::mir::ty::ConstVal::Int(n),
                                            }),
                                        ),
                                        span,
                                    );
                                    current = cx.current_block;
                                    let continue_block = cx.new_block();
                                    cx.mir.block_mut(current).terminator = Terminator::new(
                                        TerminatorKind::SwitchInt {
                                            discr: Operand::Copy(Place::local(cmp_result, span)),
                                            targets: vec![(ConstVal::Bool(true), continue_block)],
                                            otherwise: next_block,
                                        },
                                        Span::DUMMY,
                                    );
                                    current = continue_block;
                                }
                            }
                        }
                        // All literal checks passed — goto match_block
                        cx.mir.block_mut(current).terminator =
                            Terminator::new(TerminatorKind::Goto(match_block), Span::DUMMY);
                        cx.current_block = current;
                    }
                }
                _ => {}
            }
            // match_block: pattern matched — execute arm body
            cx.current_block = match_block;
            collect_pat_bindings_for_mir(cx, &arm.pat);
            lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);
            let arm_result = lower_expr_to_operand(cx, &arm.body, None);
            cx.push_assign(
                Place::local(result_local, span),
                Rvalue::Use(Operand::Copy(Place::local(arm_result, arm.body.span))),
                arm.span,
            );
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
            fallthrough_block = next_block;
            continue;
        }

        // Non-tuple, non-literal pattern (Wild, Ident, etc.) — catch-all
        cx.current_block = fallthrough_block;
        collect_pat_bindings_for_mir(cx, &arm.pat);
        lower_enum_variant_pattern_bindings(cx, scrut_local, &arm.pat);

        // Stage 14.77: For Ident patterns (bindings like `n => { ... }`),
        // assign the scrutinee value to the binding local.
        //
        // collect_pat_bindings_for_mir creates the local but doesn't assign it.
        // Without this, `n` would be uninitialized — reading garbage values.
        //
        // Per §1.0 原则 5 "报错 > 静默": bindings must be initialized to the
        // scrutinee value, not left as uninitialized stack garbage.
        if let HirPatKind::Ident(_mode, _ident, _) = &arm.pat.kind {
            // Look up the local created by collect_pat_bindings_for_mir
            if let Some(binding_local) = cx.local_map.get(&arm.pat.hir_id).copied() {
                // Check if scrut_local is a Ref (e.g., &self match)
                let scrut_ty = cx.mir.local(scrut_local).ty.clone();
                if matches!(&scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                    // Scrutinee is a reference — deref before assigning
                    let deref_ty = match &scrut_ty.kind {
                        crate::mir::ty::TyKind::Ref(_, _, inner) => (**inner).clone(),
                        _ => scrut_ty.clone(),
                    };
                    cx.push_assign(
                        Place::local(binding_local, arm.pat.span),
                        Rvalue::Use(Operand::Copy(Place {
                            kind: PlaceKind::Projection(
                                Box::new(Place::local(scrut_local, arm.pat.span)),
                                ProjectionElem::Deref,
                            ),
                            span: arm.pat.span,
                        })),
                        arm.pat.span,
                    );
                    let _ = deref_ty; // type hint (not used directly)
                } else {
                    // Scrutinee is by value — copy directly
                    cx.push_assign(
                        Place::local(binding_local, arm.pat.span),
                        Rvalue::Use(Operand::Copy(Place::local(scrut_local, arm.pat.span))),
                        arm.pat.span,
                    );
                }
            }
        }

        let arm_result = lower_expr_to_operand(cx, &arm.body, None);
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
            cx.terminate_kind(TerminatorKind::Goto(cont_block));
        }
        break;
    }
    // Stage 14.71: Only terminate fallthrough_block if no catch-all was found.
    // If a catch-all was found, the loop already terminated the last block.
    // If no catch-all, fallthrough_block needs a terminator.
    // Check if fallthrough_block is the same as cx.current_block — if so,
    // it hasn't been terminated yet (no catch-all found).
    if !cx.is_terminated() {
        cx.terminate_kind(TerminatorKind::Goto(cont_block));
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
                        ty: field_ty,
                        val: crate::mir::ty::ConstVal::Int(n),
                    }),
                ),
                span,
            );
            current = cx.current_block;

            // Switch on cmp_result: if true, continue to next check (or match);
            // if false, goto next_block (pattern didn't match)
            let continue_block = cx.new_block();
            cx.mir.block_mut(current).terminator = Terminator::new(
                TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::local(cmp_result, span)),
                    targets: vec![(ConstVal::Bool(true), continue_block)],
                    otherwise: next_block,
                },
                Span::DUMMY,
            );
            current = continue_block;
        }
        // Stage 14.87 (Bug B fix): Handle enum variant sub-patterns.
        // For `(Opt::None, 0)` etc., extract the field's enum discriminant
        // and compare to the variant index. Was: silently skipped (treated
        // as wildcard), causing wrong match results.
        else if let HirPatKind::Path(path)
        | HirPatKind::TupleStruct(path, _)
        | HirPatKind::Struct(path, _, _) = &sub_pat.kind
        {
            if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    if let Some((variant_idx, _)) = super::resolve_enum_variant(
                        cx,
                        def_id,
                        super::method_resolution::variant_name_from_path(path),
                    ) {
                        // Build the enum's Adt type for the field local
                        let enum_ty = Ty::new(
                            TyKind::Adt(def_id, Vec::<crate::mir::ty::Ty>::new().into()),
                            span,
                        );
                        let field_local = cx.mir.new_local(enum_ty.clone(), None, span);
                        cx.mir.block_mut(current).statements.push(Statement {
                            kind: StatementKind::StorageLive(field_local),
                            span,
                        });
                        cx.current_block = current;
                        cx.push_assign(
                            Place::local(field_local, span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(scrut_local, span)),
                                    ProjectionElem::Field(FieldId(i as u32), enum_ty.clone()),
                                ),
                                span,
                            })),
                            span,
                        );
                        current = cx.current_block;

                        // Extract discriminant: discr = field_local.0
                        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
                        cx.mir.block_mut(current).statements.push(Statement {
                            kind: StatementKind::StorageLive(discr_local),
                            span,
                        });
                        cx.current_block = current;
                        cx.push_assign(
                            Place::local(discr_local, span),
                            Rvalue::Use(Operand::Copy(Place {
                                kind: PlaceKind::Projection(
                                    Box::new(Place::local(field_local, span)),
                                    ProjectionElem::Field(FieldId(0), discr_ty.clone()),
                                ),
                                span,
                            })),
                            span,
                        );
                        current = cx.current_block;

                        // Compare: discr_local == variant_idx
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
                                Operand::Copy(Place::local(discr_local, span)),
                                Operand::Constant(crate::mir::ty::Const {
                                    ty: discr_ty,
                                    val: crate::mir::ty::ConstVal::Int(variant_idx as u128),
                                }),
                            ),
                            span,
                        );
                        current = cx.current_block;

                        // Switch on cmp_result
                        let continue_block = cx.new_block();
                        cx.mir.block_mut(current).terminator = Terminator::new(
                            TerminatorKind::SwitchInt {
                                discr: Operand::Copy(Place::local(cmp_result, span)),
                                targets: vec![(ConstVal::Bool(true), continue_block)],
                                otherwise: next_block,
                            },
                            Span::DUMMY,
                        );
                        current = continue_block;
                    }
                }
            }
        }
        // Wildcard or Ident — skip (always matches)
    }

    // All checks passed — goto match_block
    cx.mir.block_mut(current).terminator =
        Terminator::new(TerminatorKind::Goto(match_block), Span::DUMMY);
    cx.current_block = current;
}

/// Stage 14.86: Build a pattern-equality check for guarded match arms.
///
/// For arms with guards that have literal/Or/enum patterns, we need to
/// re-check the pattern in the otherwise block (since guarded arms were
/// not added as switch targets).
///
/// Generates: if pattern_matches(scrut, pat) { goto match_block } else { goto next_block }
///
/// Pattern kinds handled:
/// - `HirPatKind::Lit(lit_expr)` — check `scrut == lit`
/// - `HirPatKind::Or(sub_pats)` — check `scrut == lit1 || scrut == lit2 || ...`
///   (only for all-literal sub-patterns; non-literal Or sub-patterns in
///   guarded arms are not supported — fall through to next_block)
/// - `HirPatKind::Path/TupleStruct/Struct` (enum variant) — check
///   `scrut_discr == variant_idx` (extract discriminant from scrut first)
///
/// For non-literal/Or/enum patterns (Ident, Wild, etc.), this function is
/// not called (callers check `needs_pattern_check` first).
///
/// Per §1.0 原則 5 "报错 > 静默": this generates the pattern check that
/// was previously missing for guarded arms, causing them to match without
/// verifying the pattern.
fn build_pattern_equality_check(
    cx: &mut MirLowerCtxt,
    scrut_local: LocalId,
    pat: &HirPat,
    is_enum: bool,
    match_block: BasicBlockId,
    next_block: BasicBlockId,
) {
    use crate::mir::place::{BinOp, Operand, Place, PlaceKind, ProjectionElem, Rvalue};

    let span = pat.span;

    // Helper: build `scrut == lit_val` and terminate current block with
    // a SwitchInt that goes to match_block on true, next_block on false.
    let build_eq_check = |cx: &mut MirLowerCtxt, lit_val: i128, is_bool: bool| {
        // Load scrutinee value
        let scrut_place = Place::local(scrut_local, span);
        // Build `scrut == lit_val`
        let lit_const = crate::mir::ty::Const {
            ty: Ty::new(
                if is_bool {
                    TyKind::Bool
                } else {
                    TyKind::Int(crate::ast::IntTy::I32)
                },
                span,
            ),
            val: if is_bool {
                crate::mir::ty::ConstVal::Bool(lit_val != 0)
            } else {
                crate::mir::ty::ConstVal::Int(lit_val as u128)
            },
        };
        let cmp_result = cx.eval_rvalue_to_temp(
            Rvalue::BinaryOp(
                BinOp::Eq,
                Operand::Copy(scrut_place),
                Operand::Constant(lit_const),
            ),
            Ty::new(TyKind::Bool, span),
            span,
        );
        cx.terminate_kind(TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::local(cmp_result, span)),
            targets: vec![(ConstVal::Bool(true), match_block)],
            otherwise: next_block,
        });
    };

    match &pat.kind {
        HirPatKind::Lit(expr) => {
            if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                build_eq_check(cx, *n as i128, false);
            } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                build_eq_check(cx, *b as i128, true);
            } else {
                // Unknown literal — fall through to next
                cx.terminate_kind(TerminatorKind::Goto(next_block));
            }
        }
        HirPatKind::Or(sub_pats) => {
            // For Or-patterns, build: if scrut == lit1 { match } else if scrut == lit2 { match } else { next }
            // Each sub-pattern is a separate check. On success, go to match_block.
            // On failure, go to the next sub-pattern's check block.
            // After the last sub-pattern fails, go to next_block.
            let mut current_block = cx.current_block;
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if let HirPatKind::Lit(expr) = &sub_pat.kind {
                    let (lit_val, is_bool) =
                        if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                            (*n as i128, false)
                        } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                            (*b as i128, true)
                        } else {
                            // Unknown literal type — fall through to next
                            cx.current_block = current_block;
                            cx.terminate_kind(TerminatorKind::Goto(next_block));
                            return;
                        };
                    // The "failure" target is the next sub-pattern's check,
                    // or next_block if this is the last sub-pattern.
                    let failure_block = if i + 1 < sub_pats.len() {
                        cx.new_block()
                    } else {
                        next_block
                    };
                    cx.current_block = current_block;
                    // Build `scrut == lit_val` inline (can't use build_eq_check
                    // because it captures match_block/next_block from closure)
                    let scrut_place = Place::local(scrut_local, span);
                    let lit_const = crate::mir::ty::Const {
                        ty: Ty::new(
                            if is_bool {
                                TyKind::Bool
                            } else {
                                TyKind::Int(crate::ast::IntTy::I32)
                            },
                            span,
                        ),
                        val: if is_bool {
                            crate::mir::ty::ConstVal::Bool(lit_val != 0)
                        } else {
                            crate::mir::ty::ConstVal::Int(lit_val as u128)
                        },
                    };
                    let cmp_result = cx.eval_rvalue_to_temp(
                        Rvalue::BinaryOp(
                            BinOp::Eq,
                            Operand::Copy(scrut_place),
                            Operand::Constant(lit_const),
                        ),
                        Ty::new(TyKind::Bool, span),
                        span,
                    );
                    cx.terminate_kind(TerminatorKind::SwitchInt {
                        discr: Operand::Copy(Place::local(cmp_result, span)),
                        targets: vec![(ConstVal::Bool(true), match_block)],
                        otherwise: failure_block,
                    });
                    current_block = failure_block;
                } else {
                    // Non-literal sub-pattern — not supported, fall through to next
                    cx.current_block = current_block;
                    cx.terminate_kind(TerminatorKind::Goto(next_block));
                    return;
                }
            }
        }
        HirPatKind::Path(path)
        | HirPatKind::TupleStruct(path, _)
        | HirPatKind::Struct(path, _, _)
            if is_enum =>
        {
            // For enum variant, check discriminant (field 0 of scrut struct)
            if let Res::Def(def_id, crate::resolve::DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    if let Some((variant_idx, _)) = super::resolve_enum_variant(
                        cx,
                        def_id,
                        super::method_resolution::variant_name_from_path(path),
                    ) {
                        // Extract discriminant: discr = scrut.0
                        let discr_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                        let discr_local = cx.mir.new_local(discr_ty.clone(), None, span);
                        let scrut_ty = cx.mir.local(scrut_local).ty.clone();
                        let scrut_place =
                            if matches!(&scrut_ty.kind, crate::mir::ty::TyKind::Ref(_, _, _)) {
                                Place {
                                    kind: PlaceKind::Projection(
                                        Box::new(Place::local(scrut_local, span)),
                                        ProjectionElem::Deref,
                                    ),
                                    span,
                                }
                            } else {
                                Place::local(scrut_local, span)
                            };
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
                        // Check discr == variant_idx
                        let lit_const = crate::mir::ty::Const {
                            ty: Ty::new(discr_ty.kind.clone(), span),
                            val: crate::mir::ty::ConstVal::Int(variant_idx as u128),
                        };
                        let cmp_result = cx.eval_rvalue_to_temp(
                            Rvalue::BinaryOp(
                                BinOp::Eq,
                                Operand::Copy(Place::local(discr_local, span)),
                                Operand::Constant(lit_const),
                            ),
                            Ty::new(TyKind::Bool, span),
                            span,
                        );
                        cx.terminate_kind(TerminatorKind::SwitchInt {
                            discr: Operand::Copy(Place::local(cmp_result, span)),
                            targets: vec![(ConstVal::Bool(true), match_block)],
                            otherwise: next_block,
                        });
                        return;
                    }
                }
            }
            // Couldn't resolve variant — fall through
            cx.terminate_kind(TerminatorKind::Goto(next_block));
        }
        // Stage 14.90 (Bug X1 fix): Handle Tuple/TupleStruct patterns with
        // literal sub-patterns. These need conditional field checks.
        // For Tuple and TupleStruct (positional), use build_tuple_pattern_condition.
        HirPatKind::Tuple(sub_pats) => {
            build_tuple_pattern_condition(cx, scrut_local, sub_pats, match_block, next_block);
        }
        HirPatKind::TupleStruct(_, sub_pats) if !is_enum => {
            // Plain tuple struct — same as Tuple (positional fields)
            build_tuple_pattern_condition(cx, scrut_local, sub_pats, match_block, next_block);
        }
        // Stage 14.90 (Bug X1 fix): Handle Struct patterns with literal
        // sub-patterns. Look up field indices by name, check each literal.
        HirPatKind::Struct(path, fields, _) => {
            if let crate::hir::Res::Def(struct_def_id, _) = path.res {
                let field_indices: std::collections::HashMap<crate::lexer::token::Symbol, usize> = {
                    let mut map = std::collections::HashMap::new();
                    if let Some(hir) = cx.hir {
                        if let Some(crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s))) =
                            hir.find_owner(struct_def_id)
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
                let mut current = cx.current_block;
                let mut found_lit = false;
                for field_pat in fields {
                    if let Some(field_idx) = field_indices.get(&field_pat.ident.name).copied() {
                        if let HirPatKind::Lit(expr) = &field_pat.pat.kind {
                            found_lit = true;
                            let lit_val =
                                if let HirExprKind::Lit(HirLitKind::Int(n, _)) = &expr.kind {
                                    Some(*n)
                                } else if let HirExprKind::Lit(HirLitKind::Bool(b)) = &expr.kind {
                                    Some(if *b { 1 } else { 0 })
                                } else {
                                    None
                                };
                            if let Some(n) = lit_val {
                                let field_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), span);
                                let field_local = cx.mir.new_local(field_ty.clone(), None, span);
                                cx.mir.block_mut(current).statements.push(Statement {
                                    kind: StatementKind::StorageLive(field_local),
                                    span,
                                });
                                cx.current_block = current;
                                cx.push_assign(
                                    Place::local(field_local, span),
                                    Rvalue::Use(Operand::Copy(Place {
                                        kind: PlaceKind::Projection(
                                            Box::new(Place::local(scrut_local, span)),
                                            ProjectionElem::Field(
                                                FieldId(field_idx as u32),
                                                field_ty.clone(),
                                            ),
                                        ),
                                        span,
                                    })),
                                    span,
                                );
                                current = cx.current_block;
                                let cmp_result =
                                    cx.mir.new_local(Ty::new(TyKind::Bool, span), None, span);
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
                                            ty: field_ty,
                                            val: crate::mir::ty::ConstVal::Int(n),
                                        }),
                                    ),
                                    span,
                                );
                                current = cx.current_block;
                                let continue_block = cx.new_block();
                                cx.mir.block_mut(current).terminator = Terminator::new(
                                    TerminatorKind::SwitchInt {
                                        discr: Operand::Copy(Place::local(cmp_result, span)),
                                        targets: vec![(ConstVal::Bool(true), continue_block)],
                                        otherwise: next_block,
                                    },
                                    Span::DUMMY,
                                );
                                current = continue_block;
                            }
                        }
                    }
                }
                if found_lit {
                    cx.mir.block_mut(current).terminator =
                        Terminator::new(TerminatorKind::Goto(match_block), Span::DUMMY);
                    cx.current_block = current;
                } else {
                    // No literal sub-patterns — always match
                    cx.terminate_kind(TerminatorKind::Goto(match_block));
                }
            } else {
                // Can't resolve struct — fall through
                cx.terminate_kind(TerminatorKind::Goto(next_block));
            }
        }
        _ => {
            // Shouldn't happen (callers check needs_pattern_check first)
            cx.terminate_kind(TerminatorKind::Goto(match_block));
        }
    }
}
