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

use super::lower_expr_to_operand;
use super::MirLowerCtxt;
// Stage 18.279 (TD-LOC-CONTROL-FLOW): match/pattern lowering extracted to pattern_lower.rs
pub(crate) use super::pattern_lower::lower_match;

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
pub(crate) fn lower_block(
    cx: &mut MirLowerCtxt,
    block: &HirBlock,
    expected_ty: Option<&crate::mir::ty::Ty>,
) -> LocalId {
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
    // Stage 18.270 (TD-RETURN-TY-PATH-SUBSTS Phase 2d continuation):
    // thread expected_ty into the trailing expression so that generic
    // tuple struct ctors in block tail position can extract substs
    // from the expected type. This closes the soundness hole where
    // `fn make() -> Holder<i32> { Holder(true) }` silently accepted
    // type mismatches because the block's trailing Holder(true) was
    // lowered with expected_ty=None (so Holder's substs stayed as
    // Param(T), which unifies with anything).
    //
    // Per §17.6 (缺陷纳入 — same class as Phase 2d):
    // the Phase 2d fix in body_lower.rs threads expected_ty into
    // body.value, but body.value is a Block, and the Block arm in
    // lower_expr_to_operand calls lower_block WITHOUT passing
    // expected_ty. This fix threads expected_ty through lower_block
    // to the trailing expression.
    // Per §1.0 原則 6 (通解 > 特解): one expected_ty-based path
    // for all block trailing expressions.
    if let Some(expr) = &block.expr {
        lower_expr_to_operand(cx, expr, expected_ty)
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
    let then_result = lower_block(cx, then, None);
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
