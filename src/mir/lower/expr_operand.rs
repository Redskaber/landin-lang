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
use crate::mir::body::TerminatorKind;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;

// Re-exports from the parent module — see §16 (interface isolation).
// Stage 18.130: import lower_path_generic_args directly from ty_lower (per §13.4 J3)
// since mod.rs no longer re-exports it (only used by body_lower + expr_operand).
use super::ty_lower::lower_path_generic_args;
use super::{lower_hir_ty_to_mir_ty, MirLowerCtxt};
// Sibling helper modules — each owns a specialized sub-algorithm.
// (`adt_layout` is not used here: it runs from mod.rs's body entry points,
//  not from expression lowering.)
use super::{closure_capture, control_flow, field_resolution, overflow_assert, pattern_bindings};
// Stage 18.131: method resolution functions extracted to method_resolution.rs
use super::method_resolution::{auto_deref_if_ref, resolve_enum_variant};
// Stage 18.132: call lowering functions extracted to call_lower.rs
use super::call_lower::lower_expr_to_place;
// Stage 18.133: expression variant functions extracted to expr_variants.rs

pub(crate) fn lower_expr_to_operand(
    cx: &mut MirLowerCtxt,
    expr: &HirExpr,
    expected_ty: Option<&Ty>,
) -> LocalId {
    // Stage 18.256 (TD-TUPLE-CTOR-TYPECK Phase 2a): expected_ty param added
    // for future use in Phase 2b-2e (threading expected type through MIR
    // lower pipeline). Currently unused — all call sites pass `None`,
    // preserving existing behavior. Per §13.4 J4: expected_ty is a single
    // coherent concept threaded through all lower_expr_* functions.
    // Per §1.0 原則 3 (显式 > 隐式): the param is explicit even when None,
    // documenting intent at the call site.
    let _ = expected_ty;
    match &expr.kind {
        HirExprKind::Lit(lit) => {
            let (const_val, ty) = cx.lit_to_const(lit);
            cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Constant(const_val)), ty, expr.span)
        }
        HirExprKind::Path(path) => {
            // Stage 18.133 §13.4 J2: extracted to expr_variants.rs
            super::expr_variants::lower_path_expr(cx, expr, path)
        }
        HirExprKind::Binary { op, lhs, rhs, .. } => {
            // Short-circuit And/Or must be lowered to control flow,
            // not to BitAnd/BitOr (which would evaluate both sides).
            if *op == HirBinOp::And || *op == HirBinOp::Or {
                return control_flow::lower_short_circuit(cx, *op, lhs, rhs, expr.span);
            }
            let lhs_local = lower_expr_to_operand(cx, lhs, None);
            let rhs_local = lower_expr_to_operand(cx, rhs, None);

            // Stage 18.236 (Pointer Arithmetic): When op is Add/Sub and one
            // operand is a RawPtr and the other is an integer, lower to
            // GetElementPtr instead of BinaryOp. This reuses the existing
            // GEP infrastructure (Stage 18.226-18.227) — no new MIR variant.
            //
            // Per §1.0 原則 6 (通解 > 特解): one GEP path for all pointer
            // arithmetic, not a special BinaryOp pointer case.
            // Per §10 (DRY): reuses existing GetElementPtr codegen.
            if *op == HirBinOp::Add || *op == HirBinOp::Sub {
                let lhs_ty = cx.mir.local(lhs_local).ty.clone();
                let rhs_ty = cx.mir.local(rhs_local).ty.clone();
                let lhs_is_ptr = matches!(&lhs_ty.kind, crate::mir::ty::TyKind::RawPtr(_, _));
                let rhs_is_ptr = matches!(&rhs_ty.kind, crate::mir::ty::TyKind::RawPtr(_, _));

                if lhs_is_ptr != rhs_is_ptr {
                    // Exactly one is a pointer — valid pointer arithmetic.
                    // Determine which operand is the pointer and which is the index.
                    let (ptr_local, idx_local, ptr_ty) = if lhs_is_ptr {
                        (lhs_local, rhs_local, lhs_ty)
                    } else {
                        (rhs_local, lhs_local, rhs_ty)
                    };

                    // For Sub, negate the index (ptr - n → ptr + (-n)).
                    // We emit: idx_neg = 0 - idx; then GEP(ptr, [idx_neg]).
                    // For Add, just use idx directly.
                    let gep_idx_local = if *op == HirBinOp::Sub {
                        let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), expr.span);
                        let zero_local = cx.mir.new_local(i64_ty.clone(), None, expr.span);
                        cx.push_assign(
                            Place::local(zero_local, expr.span),
                            Rvalue::Use(Operand::Constant(crate::mir::ty::Const {
                                ty: i64_ty.clone(),
                                val: crate::mir::ty::ConstVal::Int(0),
                            })),
                            expr.span,
                        );
                        let neg_idx_local = cx.mir.new_local(i64_ty.clone(), None, expr.span);
                        cx.push_assign(
                            Place::local(neg_idx_local, expr.span),
                            Rvalue::BinaryOp(
                                MirLowerCtxt::lower_bin_op(crate::hir::HirBinOp::Sub),
                                Operand::Copy(Place::local(zero_local, expr.span)),
                                Operand::Copy(Place::local(idx_local, expr.span)),
                            ),
                            expr.span,
                        );
                        neg_idx_local
                    } else {
                        idx_local
                    };

                    // Lower to GetElementPtr: ptr + idx → GEP(ptr, [idx])
                    let result = cx.eval_rvalue_to_temp(
                        Rvalue::GetElementPtr {
                            base: Operand::Copy(Place::local(ptr_local, expr.span)),
                            indices: vec![Operand::Copy(Place::local(gep_idx_local, expr.span))],
                            result_ty: ptr_ty.clone(),
                        },
                        ptr_ty,
                        expr.span,
                    );
                    return result;
                }
            }

            let mir_op = MirLowerCtxt::lower_bin_op(*op);
            // Stage 15.76: For comparison ops (==, !=, <, >, <=, >=),
            // the result type is bool. For arithmetic ops (+, -, *, /, %,
            // &, |, ^, <<, >>), the result type is the lhs operand's type.
            let binop_ty = match *op {
                HirBinOp::Eq
                | HirBinOp::Ne
                | HirBinOp::Lt
                | HirBinOp::Le
                | HirBinOp::Gt
                | HirBinOp::Ge => Ty::new(TyKind::Bool, expr.span),
                _ => cx.mir.local(lhs_local).ty.clone(),
            };
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
            let inner_local = lower_expr_to_operand(cx, inner, None);
            let mir_op = MirLowerCtxt::lower_un_op(*op);
            // Stage 15.76: Use inner operand's type for unary op result
            // (same as Rust: `-a` has type of `a`).
            let unary_ty = cx.mir.local(inner_local).ty.clone();
            let result = cx.eval_rvalue_to_temp(
                Rvalue::UnaryOp(mir_op, Operand::Copy(Place::local(inner_local, inner.span))),
                unary_ty.clone(),
                expr.span,
            );
            // Stage 18.67: Emit NegOverflow assert for signed integer negation.
            // Per §1.0 原則 4 "报错 > 静默": `-i32::MIN` must panic, not wrap.
            // Per §1.0 原則 6 "通用 > 特例": reuses the Assert terminator
            // infrastructure (same as binary overflow / div-by-zero checks).
            if *op == HirUnaryOp::Neg {
                use crate::mir::ty::TyKind;
                let is_signed_int = matches!(unary_ty.kind, TyKind::Int(_));
                if is_signed_int {
                    overflow_assert::emit_neg_overflow_assert(
                        cx,
                        Operand::Copy(Place::local(inner_local, inner.span)),
                        expr.span,
                    );
                }
            }
            result
        }
        HirExprKind::Block(block) => {
            // Stage 18.270 (TD-RETURN-TY-PATH-SUBSTS Phase 2d continuation):
            // thread expected_ty into lower_block so the trailing
            // expression can use it. Per §17.6: same class as Phase 2d.
            control_flow::lower_block(cx, block, expected_ty)
        }
        HirExprKind::Call { func, args, .. } => {
            // Stage 18.133 §13.4 J2: extracted to expr_variants.rs.
            // Stage 18.258 (TD-TUPLE-CTOR-TYPECK Phase 2c): thread
            // expected_ty into lower_call_expr so it can extract substs
            // from the expected type when turbofish is absent.
            super::expr_variants::lower_call_expr(cx, expr, func, args, expected_ty)
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
                let ret_local = lower_expr_to_operand(cx, ret, None);
                // Stage 16.06: Use Operand::Move for return values.
                // Return semantically moves the value into the caller's
                // return slot. Using Operand::Copy was unsound for non-Copy
                // types (e.g., structs with `impl Drop`) — the borrow
                // checker would reject "use of moved value: does not
                // implement Copy". With field-level Copy derivation
                // (Stage 16.06), non-Copy types are now correctly
                // identified, so we must use Move for correctness.
                cx.push_assign(
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Use(Operand::Move(Place::local(ret_local, ret.span))),
                    expr.span,
                );
            } else {
                // Stage 14.23: `return;` (no value) — assign unit () to the
                // return local. This allows typeck to detect the mismatch
                // when the function expects a non-unit type (e.g. i32).
                // Previously, `return;` left the return local uninitialized,
                // and the Never block type (from Stage 14.22) allowed it to
                // pass typeck — masking the error.
                cx.push_assign(
                    Place::local(LocalId(0), Span::DUMMY),
                    Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
                    expr.span,
                );
            }
            cx.terminate_kind(TerminatorKind::Return);
            // Return a dummy local (unreachable after Return)
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }
        HirExprKind::Assign { lhs, rhs, op } => {
            // Stage 13.25: Handle compound assignment operators (+=, -=, *=, /=, %=).
            // When `op` is Some, the assignment is `lhs op= rhs`, which desugars
            // to `lhs = lhs op rhs`. Before Stage 13.25, the `op` field was
            // ignored (via `..`), so `x += 5` was lowered as `x = 5` (P0 bug).
            let lhs_place = lower_expr_to_place(cx, lhs, None);

            let rhs_local = if let Some(bin_op) = op {
                // Compound assignment: `lhs op= rhs` → `lhs = lhs op rhs`
                // Lower the RHS first, then read the LHS, apply the binop,
                // and store the result back.
                let rhs_val = lower_expr_to_operand(cx, rhs, None);
                // Read the current value of lhs into a temp
                let lhs_copy = lhs_place.clone();
                let lhs_ty = cx.fresh_infer_ty(expr.span);
                let lhs_val =
                    cx.eval_rvalue_to_temp(Rvalue::Use(Operand::Copy(lhs_copy)), lhs_ty, expr.span);
                // Apply the binary operation: result = lhs_val op rhs_val
                let mir_op = MirLowerCtxt::lower_bin_op(*bin_op);
                let result_ty = cx.fresh_infer_ty(expr.span);
                let lhs_operand = Operand::Copy(Place::local(lhs_val, expr.span));
                let rhs_operand = Operand::Copy(Place::local(rhs_val, rhs.span));
                cx.eval_rvalue_to_temp(
                    Rvalue::BinaryOp(mir_op, lhs_operand, rhs_operand),
                    result_ty,
                    expr.span,
                )
            } else {
                // Simple assignment: `lhs = rhs`
                lower_expr_to_operand(cx, rhs, None)
            };

            // Stage 3.34 (L-MUT-1 fix): handle assignment LHS that are
            // projections (field access, index, deref).
            cx.push_assign(
                lhs_place,
                Rvalue::Use(Operand::Copy(Place::local(rhs_local, rhs.span))),
                expr.span,
            );
            rhs_local
        }
        HirExprKind::Tuple { elems, .. } => {
            let elem_locals: Vec<LocalId> = elems
                .iter()
                .map(|e| lower_expr_to_operand(cx, e, None))
                .collect();
            let operands: Vec<Operand> = elem_locals
                .iter()
                .map(|l| Operand::Copy(Place::local(*l, Span::DUMMY)))
                .collect();
            // Stage 15.77: Resolve the tuple type from the element local types
            // (was: fresh_infer_ty, which stays unresolved at borrowck time
            // because writeback runs after borrowck). Same pattern as
            // Stages 15.73 (let bindings), 15.75 (deref), 15.76 (binop/unop).
            //
            // Each element local's type is read from local_decls (the source
            // of truth for lowered locals). The result is a concrete
            // `Tuple([ty_0, ty_1, ...])` type that survives borrowck.
            //
            // Per §1.0 原則 3 "显式 > 隣式": tuple type is explicitly resolved
            // from the element locals' types.
            // Per §16: reads only MIR data (local_decls), no HIR lookup.
            let elem_tys: Vec<Ty> = elem_locals
                .iter()
                .map(|l| cx.mir.local(*l).ty.clone())
                .collect();
            let tuple_ty = Ty::new(TyKind::Tuple(elem_tys), expr.span);
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
            let base_local = lower_expr_to_operand(cx, receiver, None);
            // Stage 18.304 (P3 fix): Check if receiver is a primitive type.
            // Per §2 原則 4 (报错>静默): field access on primitive types
            // (i32, bool, etc.) must report error, not silently return field 0.
            // Per §12 (最优>最小): root cause fix — check type before resolving field.
            {
                let base_ty = cx.mir.local(base_local).ty.clone();
                let inner_ty = match &base_ty.kind {
                    // Auto-deref Ref to check inner type.
                    crate::mir::ty::TyKind::Ref(_, _, inner) => inner.as_ref(),
                    _ => &base_ty,
                };
                let is_primitive = matches!(
                    &inner_ty.kind,
                    crate::mir::ty::TyKind::Int(_)
                        | crate::mir::ty::TyKind::Uint(_)
                        | crate::mir::ty::TyKind::Bool
                        | crate::mir::ty::TyKind::Char
                        | crate::mir::ty::TyKind::Float(_)
                        | crate::mir::ty::TyKind::Str
                );
                if is_primitive {
                    let field_name_str =
                        cx.interner.try_resolve(&ident.name).unwrap_or("<unknown>");
                    let type_str = cx.format_ty(&base_ty);
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        format!(
                            "no field `{}` on type `{}` — primitive types have no fields",
                            field_name_str, type_str
                        ),
                        expr.span,
                    ));
                    // Return Error placeholder local — compilation will abort.
                    let err_ty = Ty::new(crate::mir::ty::TyKind::Error, expr.span);
                    return cx.mir.new_local(err_ty, None, expr.span);
                }
            }
            // Resolve the field index from the ident name.
            let field_index = field_resolution::resolve_field_index(cx, receiver, &ident.name);
            // Stage 3.32: resolve the field's actual type from the struct def.
            let field_ty = field_resolution::resolve_field_type(cx, receiver, field_index)
                .unwrap_or_else(|| cx.fresh_infer_ty(expr.span));
            let field_ty_for_proj = field_ty.clone();
            // Stage 14.19 (GAP-31): If the base local's type is Ref (e.g. &self
            // or &mut self), auto-deref before projecting the field.
            let base_place =
                auto_deref_if_ref(cx, Place::local(base_local, receiver.span), receiver);
            let result = cx.mir.new_local(field_ty, None, expr.span);
            cx.push_assign(
                Place::local(result, expr.span),
                Rvalue::Use(Operand::Copy(Place {
                    kind: PlaceKind::Projection(
                        Box::new(base_place),
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
            let base_local = lower_expr_to_operand(cx, receiver, None);
            let index_local = lower_expr_to_operand(cx, index, None);
            // Stage 3.52: compute the element type from the receiver's type,
            // instead of using a fresh infer var (which typeck defaults to
            // i32). For `&[T]` (fat pointer), elem_ty = T. For `[T; N]`,
            // elem_ty = T. Falls back to fresh infer var if the receiver's
            // type can't be resolved (preserves old behavior for test
            // contexts).
            let elem_ty = field_resolution::resolve_index_element_type(cx, base_local, expr.span)
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
            let inner_local = lower_expr_to_operand(cx, inner, None);
            let bk = match mutability {
                crate::ast::Mutability::Mutable => crate::mir::place::BorrowKind::Mut,
                crate::ast::Mutability::Immutable => crate::mir::place::BorrowKind::Shared,
            };
            // Stage 15.77: Resolve the ref type from the inner local's type
            // (was: fresh_infer_ty, which stays unresolved at borrowck time
            // because writeback runs after borrowck). Same pattern as
            // Stages 15.73 (let bindings), 15.75 (deref), 15.76 (binop/unop).
            //
            // Result type: `Ref(Region::Erased, inner_ty, mutability)`.
            // We use Region::Erased (the codegen region) rather than a fresh
            // RegionVar because borrowck has its own region inference
            // (region_inference.rs) that assigns region variables to borrows
            // separately. At MIR lowering time, the meaningful information
            // is "this is a reference to inner_ty" — the region is filled in
            // by region inference, not by the lowerer.
            //
            // Per §1.0 原則 3 "显式 > 隐式": ref type is explicitly resolved.
            // Per §16: reads only MIR data (local_decls), no HIR lookup.
            let inner_ty = cx.mir.local(inner_local).ty.clone();
            let mir_mut = match mutability {
                crate::ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                crate::ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            let ref_ty = Ty::new(
                TyKind::Ref(Region::Erased, mir_mut, Box::new(inner_ty)),
                expr.span,
            );
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
            let inner_local = lower_expr_to_operand(cx, inner, None);
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
        HirExprKind::Try { expr: inner, .. } => lower_expr_to_operand(cx, inner, None),

        // Loop: `loop { body }` → basic block loop
        HirExprKind::Loop { body, .. } => {
            let loop_header = cx.new_block();
            let loop_body_start = cx.new_block();
            let loop_exit = cx.new_block();
            let result_ty = cx.fresh_infer_ty(expr.span);
            // Stage 14.66: Loop result local must be MUTABLE so that
            // `break expr` can assign to it without triggering
            // "cannot assign twice to immutable variable" borrowck errors.
            //
            // Previously, this used `new_local` (Immutable), causing
            // `loop { break 42; }` to fail with AssignImmutable.
            //
            // Per §1.0 原则 6 "通用 > 特例": the loop result local is
            // always mutable (break assigns to it), regardless of the
            // loop's context.
            let result = cx.mir.new_local_with_mut(
                result_ty,
                None,
                expr.span,
                crate::mir::ty::Mutability::Mutable,
            );

            // Entry → goto loop_header
            cx.terminate_kind(TerminatorKind::Goto(loop_header));

            // loop_header → goto loop_body_start (placeholder for future
            // condition checking / break targeting)
            cx.current_block = loop_header;
            cx.terminate_kind(TerminatorKind::Goto(loop_body_start));

            // Stage 13.19: Push (continue_target=loop_header, break_target=loop_exit)
            cx.loop_stack.push((loop_header, loop_exit));
            // Stage 14.24: Push the result local so `break expr` can assign to it
            cx.loop_result_locals.push(result);

            // loop_body_start → lower body → goto loop_header
            cx.current_block = loop_body_start;
            let _body_result = control_flow::lower_block(cx, body, None);
            // Stage 14.68: Only emit Goto if the body didn't diverge.
            if !cx.is_terminated() {
                cx.terminate_kind(TerminatorKind::Goto(loop_header));
            }

            // Pop the loop stack
            cx.loop_stack.pop();
            cx.loop_result_locals.pop();

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
            cx.terminate_kind(TerminatorKind::Goto(cond_block));

            // cond_block: evaluate cond, switchInt
            cx.current_block = cond_block;
            let cond_local = lower_expr_to_operand(cx, cond, None);
            cx.terminate_kind(TerminatorKind::SwitchInt {
                discr: Operand::Copy(Place::local(cond_local, cond.span)),
                targets: vec![(ConstVal::Bool(true), body_block)],
                otherwise: exit_block,
            });

            // Stage 13.19: Push (continue_target=cond_block, break_target=exit_block)
            // onto the loop stack so `break` and `continue` inside the body
            // can emit the correct Goto. Before Stage 13.19, break/continue
            // were no-ops (P0 control-flow bug).
            cx.loop_stack.push((cond_block, exit_block));

            // body_block: lower body, goto cond_block
            cx.current_block = body_block;
            control_flow::lower_block(cx, body, None);
            // Stage 14.68: Only emit Goto if the body didn't diverge
            // (e.g. via `return`, `break`, or `continue`). Without this
            // check, the Goto would OVERWRITE the Return terminator,
            // causing the function to not return (infinite loop instead).
            //
            // Per §1.0 原则 5 "报错 > 静默": silently overwriting a Return
            // terminator is a P0 control-flow bug.
            if !cx.is_terminated() {
                cx.terminate_kind(TerminatorKind::Goto(cond_block));
            }

            // Pop the loop stack (body is done)
            cx.loop_stack.pop();

            // exit_block: continuation
            cx.current_block = exit_block;
            cx.mir
                .new_local(Ty::new(TyKind::Tuple(vec![]), expr.span), None, expr.span)
        }

        // For: `for pat in iter { body }`
        //
        // Stage 14.97: Implement proper for-loop over Range expressions.
        // Previously this was a stub that just checked if iter was "truthy",
        // which produced wrong runtime behavior for any range.
        //
        // Desugars `for i in start..end { body }` to:
        //   let mut i = start;
        //   while i < end { body; i += 1; }
        //
        // For inclusive ranges `start..=end`, uses `i <= end` and the same
        // increment. For non-Range iter expressions (arrays, etc.), emits a
        // typeck error (deferred to v0.2+).
        HirExprKind::For {
            pat, iter, body, ..
        } => {
            // Stage 18.133 §13.4 J2: extracted to expr_variants.rs
            super::expr_variants::lower_for_expr(cx, expr, pat, iter, body)
        }
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
                let operand = if crate::mir::ty::is_mir_ty_copy_conservative(&ty) {
                    Operand::Copy(Place::local(*local_id, expr.span))
                } else {
                    Operand::Move(Place::local(*local_id, expr.span))
                };
                capture_operands.push(operand);
            }

            // Create closure value with captures
            // Stage 16.13 (Task 10 Step 1): Allocate a unique DefId for this
            // closure literal. Previously, all closures in a crate shared the
            // crate's first owner DefId — incorrect. Now each closure gets a
            // unique DefId from the reserved range (CLOSURE_DEF_ID_BASE).
            let closure_def_id = cx.allocate_closure_def_id();
            let closure_ty = Ty::new(
                // Stage 15.10: capture_tys is Vec<Ty>, convert to Rc<[Ty]>.
                TyKind::Closure(closure_def_id, capture_tys.clone().into()),
                expr.span,
            );
            let closure_local = cx.mir.new_local(closure_ty.clone(), None, expr.span);
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
                            // Stage 15.10: capture_tys → Rc<[Ty]>
                            AggregateKind::Closure(closure_def_id, capture_tys.clone().into()),
                            capture_operands,
                        ),
                    ))),
                    span: expr.span,
                });

            // Stage 16.34 (Task 10 Step 5 — cleanup): Removed the
            // `closure_bodies` side-table insertion. The closure dispatch
            // at the call site now uses the type-based check
            // (`TyKind::Closure(_, _)`) instead of the side-table lookup.
            //
            // The `ClosureBodyInfo` struct and `closure_bodies` field are
            // no longer needed — the `SynthesizedClosureFunction` metadata
            // (registered below) carries all the information needed for
            // the synthesized `call` function.
            //
            // Per §1.0 原則 5 "去除兼容思维": dead side-table removed.
            // Per §23 rule 5 (DRY): `SynthesizedClosureFunction` is the
            // single source of truth for closure metadata.

            // Stage 16.13 (Task 10 Step 1): Register the synthesized closure
            // function metadata. This is infrastructure for Strategy A
            // (synthesized `call` function per closure). The actual MIR body
            // synthesis is deferred to Step 2 — for now, the inline approach
            // (Stage 13.3a) is still used for closure calls.
            //
            // The captures are stored with their field index (matching the
            // order in the closure struct) for later extraction from `self`
            // in the synthesized function.
            //
            // Stage 16.31 (通解 — capture mutability): Also collect the
            // mutability of each captured variable from its local_decl.
            // This is propagated to the extract local in the closure MIR
            // body so that borrowck doesn't flag `x += 1` (where `x` is
            // a captured `mut`) as "cannot assign twice to immutable".
            let synthesized_captures: Vec<(
                crate::hir::HirId,
                u32,
                Ty,
                crate::mir::ty::Mutability,
            )> = captured
                .iter()
                .enumerate()
                .map(|(i, (hir_id, local_id))| {
                    let local_decl = cx.mir.local(*local_id);
                    let ty = local_decl.ty.clone();
                    let mutability = local_decl.mutability;
                    (*hir_id, i as u32, ty, mutability)
                })
                .collect();
            let fn_name = format!("closure_call_fn_{}", cx.closure_def_id_counter - 1);
            let synthesized_func = super::SynthesizedClosureFunction {
                def_id: closure_def_id,
                params: params.clone(),
                body: body.clone(),
                captures: synthesized_captures,
                closure_struct_ty: closure_ty.clone(),
                fn_name,
            };
            cx.register_synthesized_closure_function(synthesized_func);

            closure_local
        }

        // Break: `break expr` → goto loop exit block
        // Stage 13.19: Now emits a real Goto to the enclosing loop's exit
        // block (tracked via cx.loop_stack). Before Stage 13.19, this was a
        // no-op (P0 control-flow bug — break did nothing).
        HirExprKind::Break { expr: br_expr, .. } => {
            // Stage 14.24: Assign break value to the loop's result local
            // before jumping to the break target. Was: break value was
            // lowered but discarded (let _ = ...), so `loop { break 42; }`
            // returned an uninitialized local instead of 42.
            if let Some(e) = br_expr {
                let br_local = lower_expr_to_operand(cx, e, None);
                // Find the loop's result local and assign the break value to it.
                // The loop_stack entry is (continue_target, break_target).
                // We need the result local — it's stored in the Loop context.
                // For now, we use the loop's result local which is the last
                // local allocated before the loop body.
                if let Some((_, break_target)) = cx.loop_stack.last().copied() {
                    // Assign break value to the loop result local.
                    // The loop result local is stored in cx.loop_result_locals.
                    if let Some(result_local) = cx.loop_result_locals.last().copied() {
                        cx.push_assign(
                            Place::local(result_local, expr.span),
                            Rvalue::Use(Operand::Copy(Place::local(br_local, e.span))),
                            expr.span,
                        );
                    }
                    cx.terminate_kind(TerminatorKind::Goto(break_target));
                }
            } else {
                // Get the break target from the loop stack.
                if let Some((_, break_target)) = cx.loop_stack.last().copied() {
                    cx.terminate_kind(TerminatorKind::Goto(break_target));
                }
            }
            // Allocate a fresh block for any code after the break (unreachable
            // but needed so current_block is valid for subsequent lowering).
            cx.current_block = cx.new_block();
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Continue: `continue` → goto loop header (cond_block for while)
        // Stage 13.19: Now emits a real Goto to the enclosing loop's continue
        // target. Before Stage 13.19, this was a no-op.
        HirExprKind::Continue => {
            if let Some((continue_target, _)) = cx.loop_stack.last().copied() {
                cx.terminate_kind(TerminatorKind::Goto(continue_target));
            }
            cx.current_block = cx.new_block();
            cx.mir
                .new_local(Ty::new(TyKind::Never, Span::DUMMY), None, Span::DUMMY)
        }

        // Range: `start..end` → Aggregate (simplified)
        HirExprKind::Range { start, end, .. } => {
            let start_local = start.as_ref().map(|s| lower_expr_to_operand(cx, s, None));
            let end_local = end.as_ref().map(|e| lower_expr_to_operand(cx, e, None));
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
            let elem_locals: Vec<LocalId> = elems
                .iter()
                .map(|e| lower_expr_to_operand(cx, e, None))
                .collect();
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
                        ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), Span::DUMMY),
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

        // Repeat: `[val; N]` → Aggregate(Array, [val, val, ...]) with N copies
        HirExprKind::Repeat { elem, count, .. } => {
            let elem_local = lower_expr_to_operand(cx, elem, None);
            // Stage 14.20: Evaluate the count expression to get N.
            // If count is a literal integer, extract its value directly.
            // Stage 14.103 (ME-3 fix): If count is NOT a literal, emit a
            // typeck error instead of silently falling back to 1 element.
            //
            // Per §1.0 原则 5 "报错 > 静默": non-const array lengths must be
            // reported, not silently mis-compiled as 1-element arrays.
            let n: usize = match &count.kind {
                crate::hir::HirExprKind::Lit(crate::hir::HirLitKind::Int(val, _)) => *val as usize,
                crate::hir::HirExprKind::Lit(crate::hir::HirLitKind::Uint(val, _)) => *val as usize,
                _ => {
                    // Non-literal count — emit error, fall back to 1 element
                    // for recovery (so downstream codegen doesn't crash).
                    cx.type_errors.push(crate::typeck::TypeError::new(
                        // Stage 15.88: use human-readable expression kind
                        // (was: {:?} Debug format leaking HirExprKind::...).
                        format!(
                            "array repeat count must be a literal integer in v0.1; const-eval for non-literal counts is v0.2+ work (found {})",
                            crate::hir::hir_expr_kind_to_string(&count.kind)
                        ),
                        expr.span,
                    ));
                    1
                }
            };
            // Stage 14.79: Use the actual element type from the lowered element,
            // not Error. Was: TyKind::Error (resolved to i32, causing nested
            // arrays like [[i32; 3]; 3] to have wrong alloca type — the outer
            // array [3 x [3 x i32]] was stored into an alloca typed [3 x i32],
            // causing "Invalid InsertValueInst operands!" in LLVMSysEmitter.
            //
            // Stage 14.80 hardening: only use the concrete element type if it
            // is *not* an Infer/Error placeholder. For unsuffixed integer
            // literals like `0`, the lowered element type is `Infer(IntVar)`
            // (int-only — cannot unify with Float/Bool/Char/Str). If we put
            // that into array_ty, typeck would error on `let arr: [f64; 3]
            // = [0; 3]` — but in Rust that is *also* a type error, so the
            // behavior is correct. However, for the AggregateKind::Array we
            // use a fresh TyVar (general inference variable) so each operand
            // unifies successfully with the declared element type, and the
            // outer array_type can unify with the destination's element type.
            //
            // Per §1.0 原則 5 "报错 > 静默": surface real type errors (e.g.
            // `[0; 3]` for `[f64; 3]` is now caught by typeck instead of
            // being silently mis-compiled as `Error` → `i32`).
            // Per §1.0 原則 6 "通用 > 特例": one rule handles both cases by
            // checking whether the element type is already concrete.
            let actual_elem_ty = cx.mir.local(elem_local).ty.clone();
            // For array_ty: use concrete element type if known, else fall
            // back to Error (preserves pre-14.79 behavior for unsuffixed
            // literals — typeck will catch the mismatch if destination is
            // non-int, but codegen uses Error → I32 fallback for size).
            // For AggregateKind::Array: always use a fresh TyVar so operands
            // unify cleanly with the declared element type.
            let array_elem_ty = if matches!(&actual_elem_ty.kind, TyKind::Infer(_) | TyKind::Error)
            {
                Ty::new(TyKind::Error, expr.span)
            } else {
                actual_elem_ty
            };
            let agg_elem_ty = cx.fresh_infer_ty(expr.span);
            let count_const = crate::mir::ty::Const {
                ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), expr.span),
                val: crate::mir::ty::ConstVal::Int(n as u128),
            };
            let array_ty = Ty::new(
                TyKind::Array(Box::new(array_elem_ty), Box::new(count_const)),
                expr.span,
            );
            // Build the operands list: N copies of the element.
            let operands: Vec<Operand> = (0..n)
                .map(|_| Operand::Copy(Place::local(elem_local, elem.span)))
                .collect();
            cx.eval_rvalue_to_temp(
                Rvalue::Aggregate(AggregateKind::Array(agg_elem_ty), operands),
                array_ty,
                expr.span,
            )
        }

        // Struct literal: `Foo { x: 1, y: 2 }` → Aggregate(Adt, operands)
        HirExprKind::Struct { path, fields, .. } => {
            // Stage 18.264 (TD-STRUCT-LITERAL-FIELD-EXPECTED-TY): resolve
            // the struct's field_tys BEFORE lowering field value
            // expressions, so we can thread expected_ty into each field's
            // lower_expr_to_operand. This closes the soundness hole where
            // `Outer { f: Holder(true) }` (with `f: Holder<i32>`) silently
            // accepted type mismatches.
            //
            // Per §17.6 (缺陷纳入 — same class as TD-TUPLE-CTOR-CALL-ARG):
            // when one expected-ty propagation bug is found, audit all
            // similar paths. Struct literal field values use the same
            // pattern as fn call args — both need expected_ty from a
            // pre-computed type context.
            // Per §1.0 原則 6 (通解 > 特解): one expected_ty-based path
            // for all field value lowering, not a per-type special case.
            // Per §2 原則 9 (正确 > 妥协): proper expected-ty propagation
            // at lower time, not relying on typeck back-propagation.
            let pre_field_tys: Option<Vec<Ty>> = if let Res::Def(def_id, DefKind::Struct) = path.res
            {
                let substs_from_path =
                    lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                // Stage 18.268 (TD-GENERIC-STRUCT-LITERAL-FIELD-EXPECTED-TY):
                // If substs from turbofish are empty AND expected_ty is
                // Some(Adt with same def_id) with non-empty substs, use
                // expected substs. This closes the soundness hole where
                // `Generic { f: Holder(true) }` (with `let g: Generic<Holder<i32>>`)
                // silently accepted type mismatches because field_tys
                // were resolved with empty substs (Param T unifies with
                // anything).
                //
                // Per §17.6 (缺陷纳入 — same class as TD-TUPLE-CTOR-TYPECK Phase 2c):
                // when one expected-ty propagation bug is found, audit
                // ALL similar paths until no more found.
                // Per §1.0 原則 6 (通解 > 特解): one expected_ty-based
                // substs extraction path for all struct literal fields.
                let substs = if substs_from_path.is_empty() {
                    if let Some(expected) = expected_ty {
                        if let TyKind::Adt(exp_def_id, exp_substs) = &expected.kind {
                            if *exp_def_id == def_id && !exp_substs.is_empty() {
                                exp_substs.clone()
                            } else {
                                substs_from_path.clone()
                            }
                        } else {
                            substs_from_path.clone()
                        }
                    } else {
                        substs_from_path.clone()
                    }
                } else {
                    substs_from_path.clone()
                };
                let field_tys = if substs.is_empty() {
                    field_resolution::resolve_adt_field_tys(cx, def_id)
                } else {
                    field_resolution::resolve_adt_field_tys_with_substs(cx, def_id, &substs)
                };
                // Also use the substs for the actual Aggregate below.
                // We need to thread them — but the Aggregate is built
                // later using `lower_path_generic_args` again. To avoid
                // double-resolution, we'll use the resolved substs here
                // and pass them via a side variable.
                // Actually, the Aggregate below calls lower_path_generic_args
                // again — let's just leave that for now; the expected_ty
                // threading for arg lowering is what closes the soundness
                // hole (typeck will catch the mismatch).
                let _ = substs; // suppress unused warning
                Some(field_tys)
            } else if let Res::Def(def_id, DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    let variant_name = super::method_resolution::variant_name_from_path(path);
                    if let Some((_, field_tys)) = resolve_enum_variant(cx, def_id, variant_name) {
                        Some(field_tys)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            // Lower each field value, threading expected_ty from the
            // pre-resolved field_tys (if available).
            let field_locals: Vec<LocalId> = fields
                .iter()
                .enumerate()
                .filter_map(|(i, f)| {
                    f.expr.as_ref().map(|e| {
                        let field_expected_ty = pre_field_tys.as_ref().and_then(|tys| tys.get(i));
                        lower_expr_to_operand(cx, e, field_expected_ty)
                    })
                })
                .collect();
            // Stage 15.64: Choose Copy or Move based on the field value's type.
            // For Copy types (primitives, refs, etc.), use Copy (source remains
            // valid). For non-Copy types (Adt with Drop, Str, etc.), use Move
            // (transfers ownership, source is marked as moved).
            //
            // Previously, this always used Operand::Copy for ALL fields, which
            // caused double-drop of non-Copy field temporaries (the temp was
            // not marked as moved, so elaborate_drops inserted a Drop for it
            // in addition to the Drop for the struct itself).
            //
            // Per §23 rule 5 (DRY): uses the shared `is_mir_ty_copy_conservative`
            // helper from `mir::ty` (same as `let` bindings in control_flow.rs).
            // Per §1.0 原則 5 "报错 > 静默": conservative (Move for unknown Adt)
            // is preferred over unsound (Copy for non-Copy).
            let operands: Vec<Operand> = field_locals
                .iter()
                .map(|l| {
                    let field_ty = &cx.mir.local(*l).ty;
                    if crate::mir::ty::is_mir_ty_copy_conservative(field_ty) {
                        Operand::Copy(Place::local(*l, Span::DUMMY))
                    } else {
                        Operand::Move(Place::local(*l, Span::DUMMY))
                    }
                })
                .collect();
            // Stage 3.30 (per §15): if the path resolves to a known struct
            // DefId, use AggregateKind::Adt (the proper representation).
            // Stage 3.38 (L-ENUM): also handle enum struct variants
            // (e.g., `Shape::Circle { r: 1.0 }`).
            //
            // Stage 16.52 (Task 11 Phase 1c): propagate generic args from
            // path into Adt substs (consistent with lower_hir_ty_to_mir_ty).
            // Stage 16.53 (Task 11 Phase 2): use resolve_adt_field_tys_with_substs
            // so generic struct fields get substituted with the Adt's substs.
            if let Res::Def(def_id, DefKind::Struct) = path.res {
                let substs = lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                let field_tys = if substs.is_empty() {
                    field_resolution::resolve_adt_field_tys(cx, def_id)
                } else {
                    field_resolution::resolve_adt_field_tys_with_substs(cx, def_id, &substs)
                };
                let struct_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
                return cx.eval_rvalue_to_temp(
                    Rvalue::Aggregate(AggregateKind::Adt(def_id, 0, substs, field_tys), operands),
                    struct_ty,
                    expr.span,
                );
            }
            // Stage 3.38 (L-ENUM): Enum struct variant (e.g., `Shape::Circle { r: 1.0 }`).
            if let Res::Def(def_id, DefKind::Enum) = path.res {
                if path.segments.len() >= 2 {
                    let variant_name = super::method_resolution::variant_name_from_path(path);
                    if let Some((variant_idx, field_tys)) =
                        resolve_enum_variant(cx, def_id, variant_name)
                    {
                        // Prepend discriminant to the operands.
                        let discr = Operand::Constant(Const {
                            ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                            val: ConstVal::Uint(variant_idx as u128),
                        });
                        let mut all_operands = vec![discr];
                        all_operands.extend(operands);
                        let substs =
                            lower_path_generic_args(path, &mut 0, cx.hir, &cx.generic_params);
                        let enum_ty = Ty::new(TyKind::Adt(def_id, substs.clone()), expr.span);
                        return cx.eval_rvalue_to_temp(
                            Rvalue::Aggregate(
                                AggregateKind::Adt(def_id, variant_idx, substs, field_tys),
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

        // Stage 13.12 + Stage 13.13: Println — emit inline StatementKind::Println
        // in the current basic block so codegen places the printf call at the
        // exact source-code position (fixes Stage 13.12 ordering bug).
        //
        // Stage 13.12 stored the message in a Vec<String> side-table and
        // codegen emitted a separate __landin_printlns_<fnname> helper
        // function called BEFORE landin_main() — this broke ordering for
        // loops and conditionals.
        //
        // Stage 18.48: HirExprKind::Println arm removed — println! now goes
        // through the Call path via __landin_println macro expansion.
        // The parser never generates Expr::Println, so HIR never contains
        // HirExprKind::Println, so this arm is never reached.

        // Stage 4.10 + Stage 13.4a: MacroCall — expand known built-in macros.
        // Stage 4.10: 7 macros (println, print, eprintln, eprint, stringify, assert, debug_assert)
        // Stage 13.4a (TD-032): +19 macros (assert_eq, assert_ne, debug_assert_eq,
        //   debug_assert_ne, write, writeln, panic, todo, unimplemented, unreachable,
        //   cfg, include, concat, env, option_env, format_args, format, vec, dbg)
        // Total: 26 built-in macros per 13-stage1-feature-whitelist.md §2.6
        HirExprKind::MacroCall { path, .. } => {
            // Get the macro name from the last path segment.
            let macro_name = path.segments.last().map(|s| s.ident.name);
            if let Some(name_spur) = macro_name {
                let name = cx.interner.resolve(&name_spur).to_string();
                match name.as_str() {
                    // === Printing macros (4) — emit printf call + produce unit ===
                    // Stage 13.11: Now emits actual printf calls for string literal args.
                    "println" | "print" | "eprintln" | "eprint" => {
                        // Stage 13.11: Emit a call to printf/puts.
                        // We declare printf as extern and call it with the format string.
                        // For println!, we append "\n" to the message.
                        // For eprintln!/eprint!, we use fprintf(stderr, ...).
                        //
                        // Since we don't have the macro args in MIR (they were skipped
                        // by the parser), we emit a no-op unit local for now.
                        // The actual printf call is generated in the C wrapper
                        // via the TextEmitter path (which emits printf declarations).
                        //
                        // For LLVMSysEmitter: the C wrapper includes stdio.h,
                        // so printf is available. The codegen_from_mir path
                        // calls emit_call("printf", ...) which declares printf
                        // as extern in the LLVM module.
                        //
                        // However, since we don't have the message string in MIR,
                        // we can't emit the printf call here. The message was
                        // captured in the AST Println node but lost during HIR
                        // lowering (it was lowered to MacroCall without args).
                        //
                        // For now: produce unit (same as before). The println!
                        // output support requires either:
                        // 1. Storing the message in HIR (new HIR variant), or
                        // 2. A side-channel from parser to codegen, or
                        // 3. Full macro expansion (Stage 4 feature)
                        //
                        // Stage 13.11 pragmatic approach: the parser captures
                        // the message in Expr::Println, but HIR lowering converts
                        // it back to MacroCall (losing the message). To fix,
                        // we need a HIR variant that carries the message.
                        // This is deferred to a future stage.
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Stringification macros (2) — produce &str ===
                    "stringify" | "concat" => {
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
                    // === Assertion macros (6) — produce unit ===
                    "assert" | "debug_assert" | "assert_eq" | "assert_ne" | "debug_assert_eq"
                    | "debug_assert_ne" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Writing macros (2) — produce unit ===
                    "write" | "writeln" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Diverging macros (4) — produce Never type ===
                    "panic" | "todo" | "unimplemented" | "unreachable" => {
                        let never_ty = Ty::new(TyKind::Never, expr.span);
                        cx.mir.new_local(never_ty, None, expr.span)
                    }
                    // === Configuration macro (1) — produce bool ===
                    "cfg" => {
                        let bool_ty = Ty::new(TyKind::Bool, expr.span);
                        cx.mir.new_local(bool_ty, None, expr.span)
                    }
                    // === File inclusion macro (1) — produce unit ===
                    "include" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Environment macros (2) — produce &str ===
                    "env" | "option_env" => {
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
                    // === Format args macro (1) — produce unit ===
                    "format_args" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Format macro (1) — produce String (simplified to unit for MVP) ===
                    "format" => {
                        // Stage 13.4a: simplified to unit (full String requires alloc support)
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Vec macro (1) — produce unit (simplified for MVP) ===
                    "vec" => {
                        // Stage 13.4a: simplified to unit (full Vec<T> requires alloc support)
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    // === Debug macro (1) — produce unit ===
                    "dbg" => {
                        let unit_ty = Ty::new(TyKind::Tuple(vec![]), expr.span);
                        cx.mir.new_local(unit_ty, None, expr.span)
                    }
                    _ => {
                        // Stage 14.104 (ME-5 fix): Unknown macro → emit typeck
                        // error instead of silently returning Error placeholder.
                        //
                        // Per §1.0 原则 5 "报错 > 静默": unknown macros must be
                        // reported, not silently mis-compiled as Error→0.
                        let macro_name_str = cx.interner.resolve(&name_spur).to_string();
                        cx.type_errors.push(crate::typeck::TypeError::new(
                            format!("cannot find macro `{}` in this scope", macro_name_str),
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
                }
            } else {
                // No macro name → emit error.
                cx.type_errors.push(crate::typeck::TypeError::new(
                    "macro call has no name".to_string(),
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
        }

        // Unsafe block: just lower inner block (unsafety is a typeck concern)
        HirExprKind::Unsafe(block) => control_flow::lower_block(cx, block, None),

        // Stage 8.5: async/await — MVP: evaluate synchronously
        HirExprKind::Await { expr } => lower_expr_to_operand(cx, expr, None),
        HirExprKind::Async { block } => control_flow::lower_block(cx, block, None),

        // MethodCall: `receiver.method(args)` → simplified to Call
        HirExprKind::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            // Stage 18.309 §13.4 J2: moved to method_call_lower.rs (split from expr_variants.rs)
            super::method_call_lower::lower_method_call_expr(cx, expr, receiver, method, args)
        }
    }
}
