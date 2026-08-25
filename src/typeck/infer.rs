//! Type checker — inference sub-responsibility.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.128):
//! Split from `checker.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all `infer_*` methods on `TypeChecker`.
//!
//! ## Sub-responsibility
//! Type inference: walk MIR rvalues/operands/places and produce `Ty` values
//! (with side effects: may push type errors or unify inference variables).
//!
//! ## J1-J6 compliance
//! - J1: typeck design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (inference)
//! - J3: no circular deps (methods operate on `&mut self` from checker.rs)
//! - J4: inference sub-responsibility is complete in this file
//! - J5: stays within typeck stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use crate::ast;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::*;
use crate::session::Span;
use crate::typeck::error::TypeError;

use super::checker::TypeChecker;
use super::predicates::{is_arithmetic_ty, is_negatable_ty, is_notable_ty, is_shift_count_ty};

impl TypeChecker {
    pub(super) fn infer_rvalue_type_only(&mut self, mir: &MirBody, rv: &Rvalue) -> Ty {
        use crate::mir::place::Rvalue;
        match rv {
            Rvalue::Use(op) => self.infer_operand_type_only(mir, op),
            Rvalue::BinaryOp(op, a, _) => {
                // Comparison ops return Bool; arithmetic ops return operand type.
                match op {
                    crate::mir::place::BinOp::Eq
                    | crate::mir::place::BinOp::Ne
                    | crate::mir::place::BinOp::Lt
                    | crate::mir::place::BinOp::Le
                    | crate::mir::place::BinOp::Gt
                    | crate::mir::place::BinOp::Ge => Ty::from_kind(TyKind::Bool),
                    _ => self.infer_operand_type_only(mir, a),
                }
            }
            Rvalue::BinaryOp2(_, a, _) => {
                // Range ops — return the first operand's type (best effort).
                self.infer_operand_type_only(mir, a)
            }
            Rvalue::Load(_, _) | Rvalue::GetElementPtr { .. } => {
                // Stage 18.226: MIR intrinsic ops — return Infer for now

                Ty::from_kind(TyKind::Error)
            }

            Rvalue::UnaryOp(_, op) => self.infer_operand_type_only(mir, op),
            Rvalue::Cast(_, _, ty) => ty.clone(),
            Rvalue::Aggregate(kind, operands) => match kind {
                crate::mir::place::AggregateKind::Tuple => {
                    let tys: Vec<Ty> = operands
                        .iter()
                        .map(|op| self.infer_operand_type_only(mir, op))
                        .collect();
                    Ty::from_kind(TyKind::Tuple(tys))
                }
                crate::mir::place::AggregateKind::Array(elem_ty) => {
                    let len = operands.len() as u128;
                    Ty::from_kind(TyKind::Array(
                        Box::new(elem_ty.clone()),
                        Box::new(crate::mir::ty::Const {
                            ty: Ty::from_kind(TyKind::Uint(ast::UintTy::Usize)),
                            val: crate::mir::ty::ConstVal::Uint(len),
                        }),
                    ))
                }
                crate::mir::place::AggregateKind::Adt(def_id, _variant, substs, _field_tys) => {
                    Ty::from_kind(TyKind::Adt(*def_id, substs.clone()))
                }
                crate::mir::place::AggregateKind::Closure(def_id, substs) => {
                    Ty::from_kind(TyKind::Closure(*def_id, substs.clone()))
                }
            },
            Rvalue::Ref(_, _, lv) => {
                let inner_ty = self.infer_place(mir, lv);
                Ty::from_kind(TyKind::Ref(
                    crate::mir::ty::Region::Erased,
                    crate::mir::ty::Mutability::Immutable,
                    Box::new(inner_ty),
                ))
            }
        }
    }

    /// Stage 18.71: Infer an operand's type WITHOUT side effects.
    /// Stage 18.72: This is now `&mut self` because `infer_place` is
    /// `&mut self` (tuple index bounds check pushes errors). The
    /// "no side effects" contract of `post_check_statement` is preserved
    /// because `infer_rvalue_type_only` (the caller) doesn't use
    /// `infer_operand` — it uses this function directly. The tuple index
    /// OOB error from `infer_place` is acceptable here because it's a
    /// real error that should be reported.
    pub(super) fn infer_operand_type_only(
        &mut self,
        mir: &MirBody,
        op: &crate::mir::place::Operand,
    ) -> Ty {
        match op {
            crate::mir::place::Operand::Copy(lv) | crate::mir::place::Operand::Move(lv) => {
                self.infer_place(mir, lv)
            }
            crate::mir::place::Operand::Constant(c) => c.ty.clone(),
        }
    }

    /// Stage 3.60: Writeback field types using FieldTyTable instead of HIR.
    pub(super) fn infer_place(&mut self, mir: &MirBody, lv: &Place) -> Ty {
        match &lv.kind {
            PlaceKind::Local(id) => {
                if (id.0 as usize) < mir.local_decls.len() {
                    mir.local(*id).ty.clone()
                } else {
                    Ty::new(TyKind::Error, lv.span)
                }
            }
            PlaceKind::Static(_) => {
                // Static type would come from the HIR; for now, Error
                Ty::new(TyKind::Error, lv.span)
            }
            PlaceKind::Projection(base, elem) => {
                let base_ty = self.infer_place(mir, base);
                self.infer_projection(mir, &base_ty, elem, lv.span)
            }
        }
    }

    /// Infer the type after applying a projection element.
    ///
    /// Stage 18.65: Three sites below (Deref on non-Ref, Index on non-Array,
    /// ConstantIndex on non-Array) silently return `TyKind::Error` without
    /// pushing a `TypeError`. These are known Stage 0 limitations — the
    /// conformance tests `err-*-deref-non-ref-*` and `err-*-index-non-array-*`
    /// document the current behavior. Adding errors here would change those
    /// tests from `compile_error` to having specific error patterns.
    /// Per §1.0 原則 4 "报错 > 静默": these should push errors in a future stage.
    ///
    /// Stage 18.72 P1-B: Added tuple index bounds check. When base type is
    /// `Tuple(tys)` and `Field(field_id, _)` is applied, verify
    /// `field_id.0 < tys.len()`. If out of bounds, push a TypeError and
    /// return Error type.
    /// Per §1.0 原則 4 "报错 > 静默": tuple index OOB must be reported.
    pub(super) fn infer_projection(
        &mut self,
        mir: &MirBody,
        base_ty: &Ty,
        elem: &ProjectionElem,
        place_span: Span,
    ) -> Ty {
        match elem {
            ProjectionElem::Deref => {
                if let TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) = &base_ty.kind {
                    (**inner).clone()
                } else {
                    // Stage 18.76 P1-A: Deref on non-pointer types is a known
                    // Stage 0 limitation. Pattern bindings on &self, closure
                    // captures, and other internal mechanisms produce Deref
                    // projections on non-Ref types. Pushing an error here would
                    // break valid code that relies on this limitation.
                    //
                    // Per §1.0 原則 9 "正确 > 妥协": defer error reporting for
                    // Deref until typeck properly tracks reference types through
                    // pattern bindings (v0.2 work).
                    //
                    // Return Error type (not base_ty) to avoid confusing
                    // writeback — returning base_ty would make the local's
                    // type be the pointer type, not the pointee type.
                    Ty::from_kind(TyKind::Error)
                }
            }
            ProjectionElem::Field(field_id, field_ty) => {
                // Stage 18.72 P1-B: Tuple index bounds check.
                // Per §1.0 原則 4 "报错 > 静默": out-of-bounds tuple index
                // must be reported, not silently return Error.
                if let TyKind::Tuple(tys) = &base_ty.kind {
                    if (field_id.0 as usize) >= tys.len() {
                        self.errors.push(TypeError::new(
                            format!(
                                "tuple index out of bounds: index {} but tuple has {} element(s)",
                                field_id.0,
                                tys.len()
                            ),
                            place_span,
                        ));
                        return Ty::from_kind(TyKind::Error);
                    }
                }
                // Stage 18.217 (TD-TUPLE-FIELD-CHECK): Validate tuple struct
                // field index for Adt types. For `struct Box<T>(*mut T)` (1 field),
                // `b.1` should be reported as out-of-bounds.
                // Per §1.0 原則 4 (报错>静默): must report, not silently accept.
                // Per §1.0 原則 9 (正确>妥协): only check when Adt has known
                // layout (not Infer/Error/Param — those defer).
                if let TyKind::Adt(def_id, _) = &base_ty.kind {
                    if let Some(crate::mir::body::AdtLayout::Struct { field_tys }) =
                        mir.adt_layouts.get(def_id)
                    {
                        if (field_id.0 as usize) >= field_tys.len() {
                            self.errors.push(TypeError::new(
                                format!(
                                    "field index out of bounds: index {} but struct has {} field(s)",
                                    field_id.0,
                                    field_tys.len()
                                ),
                                place_span,
                            ));
                            return Ty::from_kind(TyKind::Error);
                        }
                    }
                }
                field_ty.clone()
            }
            ProjectionElem::Index(idx_local) => {
                // Stage 18.76 P1-A: Allow Array, Slice, Str, and Ref(_, _, Str)
                // as indexable types. Str indexing returns u8 (byte).
                // Per §1.0 原則 6 "通用 > 特例": one check covers all indexable types.
                // Per §1.0 原則 9 "正确 > 妥协": defer for Infer/Error/Param types
                // (don't push false-positive errors on unresolved types).
                let inner_ty = match &base_ty.kind {
                    TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                    TyKind::Str => Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span)),
                    TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str) => {
                        Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                    }
                    TyKind::Ref(_, _, inner) => {
                        // For &Array, &Slice, or &Str, index returns element type.
                        // For &Infer or &Error, defer (don't push error).
                        match &inner.kind {
                            TyKind::Array(inner, _) | TyKind::Slice(inner) => {
                                Some((**inner).clone())
                            }
                            TyKind::Str => {
                                Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                            }
                            TyKind::Infer(_) | TyKind::Error => None, // defer
                            _ => None,
                        }
                    }
                    // Stage 18.76: Defer for unresolved types — don't push false-positive errors.
                    TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
                    _ => None,
                };
                if let Some(inner) = inner_ty {
                    // Stage 18.73 P1-D: Validate array index type.
                    if let Some(idx_local_decl) = mir.local_decls.get(idx_local.0 as usize) {
                        let idx_ty = self.unify.resolve(&idx_local_decl.ty);
                        match &idx_ty.kind {
                            TyKind::Int(_)
                            | TyKind::Uint(_)
                            | TyKind::Infer(InferVar::IntVar(_))
                            | TyKind::Error => {
                                // OK — integer type, Infer (deferred), or Error.
                            }
                            _ => {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "array index must be an integer type, found {}",
                                        self.format_ty(&idx_ty)
                                    ),
                                    place_span,
                                ));
                            }
                        }
                    }
                    inner
                } else {
                    // Stage 18.76: Defer for Infer/Error/Param types — don't push
                    // false-positive errors on unresolved types. Return Error
                    // type to propagate the unknown state.
                    Ty::from_kind(TyKind::Error)
                }
            }
            ProjectionElem::ConstantIndex { .. } | ProjectionElem::Subslice { .. } => {
                // Stage 18.76 P1-A: Same indexable types as Index.
                // Per §1.0 原則 9 "正确 > 妥协": defer for Infer/Error/Param types.
                let inner_ty = match &base_ty.kind {
                    TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                    TyKind::Str => Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span)),
                    TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str) => {
                        Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                    }
                    TyKind::Ref(_, _, inner) => match &inner.kind {
                        TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                        TyKind::Str => {
                            Some(Ty::new(TyKind::Uint(crate::ast::UintTy::U8), place_span))
                        }
                        TyKind::Infer(_) | TyKind::Error => None, // defer
                        _ => None,
                    },
                    // Stage 18.76: Defer for unresolved types.
                    TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
                    _ => None,
                };
                if let Some(inner) = inner_ty {
                    inner
                } else {
                    // Stage 18.76: Defer for Infer/Error/Param types — don't push
                    // false-positive errors on unresolved types.
                    Ty::from_kind(TyKind::Error)
                }
            }
        }
    }

    /// Infer the type of an rvalue.
    ///
    /// Stage 15.82: `stmt_span` is the span of the enclosing `Statement`
    /// (or the terminator span for rvalues embedded in terminators). It's
    /// used to attach accurate spans to errors produced inside `infer_rvalue`
    /// (e.g., BinaryOp/UnaryOp type mismatches). Previously these errors
    /// used `Span::DUMMY` (file start "1:1"), making them hard to locate.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the span is an explicit parameter,
    /// not hidden state on `self`.
    pub(super) fn infer_rvalue(&mut self, mir: &MirBody, rv: &Rvalue, stmt_span: Span) -> Ty {
        match rv {
            Rvalue::Use(operand) => self.infer_operand(mir, operand),
            Rvalue::BinaryOp(op, a, b) => {
                // G8 fix (Stage 2.4g): resolve operands before type checking,
                // so TyVar bound to concrete types (Bool, Str, Tuple) is
                // correctly rejected by is_arithmetic_ty etc.
                // Stage 18.72: Split into two statements to avoid borrow conflict.
                let a_ty_raw = self.infer_operand(mir, a);
                let a_ty = self.unify.resolve(&a_ty_raw);
                let b_ty_raw = self.infer_operand(mir, b);
                let b_ty = self.unify.resolve(&b_ty_raw);
                // Unify lhs and rhs types (they must match for arithmetic)
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        // Comparison: unify a and b, return bool
                        if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                            // Stage 15.82: use stmt_span for unify errors.
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                        Ty::from_kind(TyKind::Bool)
                    }
                    // Bitwise ops: Bool or integer types only.
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                        if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                            // Stage 15.82: use stmt_span for unify errors.
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                        // Result type matches operand type (Bool or Int).
                        a_ty
                    }
                    // Shifts: lhs can be int, rhs must be int (not bool).
                    BinOp::Shl | BinOp::Shr => {
                        if !is_shift_count_ty(&b_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "shift count must be an integer type, found {}",
                                    self.format_ty(&b_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        a_ty
                    }
                    // Arithmetic: lhs and rhs must be Int/Uint/Float.
                    // G7 fix (Stage 2.4f): reject Bool, Str, Tuple, etc.
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                        if !is_arithmetic_ty(&a_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply arithmetic to {} (expected integer or float)",
                                    self.format_ty(&a_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        if !is_arithmetic_ty(&b_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply arithmetic to {} (expected integer or float)",
                                    self.format_ty(&b_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                            // Stage 15.82: use stmt_span for unify errors.
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                        a_ty
                    }
                }
            }
            Rvalue::BinaryOp2(_, a, _) => {
                // Stage 14.102 (ME-2 fix): BinaryOp2 is used for Range
                // expressions (start..end). The result type is a Range struct,
                // but for v0.1 we don't have Range in the type system.
                // Previously this silently returned Ty::Error.
                //
                // Per §1.0 原则 5 "报错 > 静默": emit a type error instead of
                // silently returning Error. The operand type is still inferred
                // (for use in for-loop desugaring).
                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                let _a_ty = self.infer_operand(mir, a);
                self.errors.push(TypeError::new(
                    "range expressions (start..end) are not supported in type position in v0.1 — use them only in for-loop iterators".to_string(),
                    stmt_span,
                ));
                Ty::from_kind(TyKind::Error)
            }
            Rvalue::Load(_, _) | Rvalue::GetElementPtr { .. } => {
                // Stage 18.226: MIR intrinsic ops — return Infer for now

                Ty::from_kind(TyKind::Error)
            }

            Rvalue::UnaryOp(op, operand) => {
                // Stage 18.72: Split into two statements to avoid borrow conflict.
                let inner_ty_raw = self.infer_operand(mir, operand);
                let inner_ty = self.unify.resolve(&inner_ty_raw);
                match op {
                    UnOp::Not => {
                        // !bool → bool, !int → int
                        // G7 fix: !str, !float, !tuple are errors.
                        // G8 fix (Stage 2.4g): resolve before checking,
                        // so TyVar bound to Tuple/Float is correctly rejected.
                        if !is_notable_ty(&inner_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply `!` to {} (expected bool or integer)",
                                    self.format_ty(&inner_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        inner_ty
                    }
                    UnOp::Neg => {
                        // -int → int, -float → float
                        // G7 fix: -bool, -str, -tuple are errors.
                        // G8 fix (Stage 2.4g): resolve before checking.
                        if !is_negatable_ty(&inner_ty) {
                            self.errors.push(TypeError::new(
                                // Stage 15.80: use human-readable type name.
                                // Stage 15.82: use stmt_span (was: Span::DUMMY).
                                format!(
                                    "cannot apply unary `-` to {} (expected integer or float)",
                                    self.format_ty(&inner_ty)
                                ),
                                stmt_span,
                            ));
                        }
                        inner_ty
                    }
                }
            }
            Rvalue::Ref(_, borrow_kind, lv) => {
                let inner_ty = self.infer_place(mir, lv);
                let mutability = match borrow_kind {
                    BorrowKind::Shared => Mutability::Immutable,
                    BorrowKind::Mut => Mutability::Mutable,
                    BorrowKind::Raw => Mutability::Immutable,
                };
                Ty::from_kind(TyKind::Ref(Region::Erased, mutability, Box::new(inner_ty)))
            }
            Rvalue::Cast(_, _, target_ty) => target_ty.clone(),
            Rvalue::Aggregate(kind, operands) => match kind {
                AggregateKind::Tuple => {
                    let elem_tys: Vec<Ty> = operands
                        .iter()
                        .map(|o| self.infer_operand(mir, o))
                        .collect();
                    Ty::from_kind(TyKind::Tuple(elem_tys))
                }
                AggregateKind::Array(elem_ty) => {
                    // G7 fix (Stage 2.4f): unify each element's type with
                    // the array's declared element type. This catches
                    // `[1, true]` (Int + Bool mismatch).
                    // Stage 15.83: use stmt_span for unify errors (was:
                    // Span::DUMMY from mismatch(), producing "1:1").
                    for op in operands {
                        let op_ty = self.infer_operand(mir, op);
                        if let Err(mut e) = self.unify.unify(&op_ty, elem_ty, stmt_span) {
                            if stmt_span != Span::DUMMY {
                                e.span = stmt_span;
                            }
                            self.errors.push(*e);
                        }
                    }
                    Ty::from_kind(TyKind::Array(
                        Box::new(elem_ty.clone()),
                        Box::new(Const {
                            ty: Ty::from_kind(TyKind::Uint(ast::UintTy::Usize)),
                            val: ConstVal::Uint(operands.len() as u128),
                        }),
                    ))
                }
                // Stage 3.32 (L-DEBT-2 fix): AggregateKind::Adt now carries
                // field_tys (per §16 data sink from Stage 3.30). Use them
                // to unify each operand with its declared field type, and
                // return the Adt type.
                // Stage 15.83: use stmt_span for unify errors (was:
                // Span::DUMMY from mismatch(), producing "1:1").
                AggregateKind::Adt(def_id, _variant, _substs, field_tys) => {
                    for (i, op) in operands.iter().enumerate() {
                        let op_ty = self.infer_operand(mir, op);
                        if let Some(field_ty) = field_tys.get(i) {
                            if let Err(mut e) = self.unify.unify(&op_ty, field_ty, stmt_span) {
                                if stmt_span != Span::DUMMY {
                                    e.span = stmt_span;
                                }
                                self.errors.push(*e);
                            }
                        }
                    }
                    Ty::from_kind(TyKind::Adt(*def_id, _substs.clone()))
                }
                // Stage 16.29 (通解 — fix closure type inference):
                // Previously, AggregateKind::Closure returned a fresh Infer
                // var, which caused the closure literal's type to be lost.
                // This broke nested closures (`|| || x`) — the outer
                // closure's return type stayed Infer, causing "expected
                // function, found _" errors.
                //
                // The fix: return the actual Closure type
                // (`Closure(def_id, substs)`), which is the correct type
                // of a closure literal. This matches how Rust handles
                // closure literals — their type is determined at lowering
                // time, not inferred.
                //
                // Per §1.0 原則 9 "正确 > 妥协": fix the root cause
                // (return correct type), not the symptom ( Infer var).
                AggregateKind::Closure(def_id, substs) => {
                    for op in operands {
                        let _ = self.infer_operand(mir, op);
                    }
                    Ty::from_kind(TyKind::Closure(*def_id, substs.clone()))
                }
            },
        }
    }

    /// Infer the type of an operand.
    ///
    /// Stage 18.72: Changed from `&self` to `&mut self` because
    /// `infer_place` is now `&mut self` (to support tuple index bounds
    /// check in `infer_projection`).
    pub(super) fn infer_operand(&mut self, mir: &MirBody, op: &Operand) -> Ty {
        match op {
            Operand::Copy(lv) | Operand::Move(lv) => self.infer_place(mir, lv),
            Operand::Constant(c) => c.ty.clone(),
        }
    }

    // Stage 15.86: `operand_span` moved to `mir::place::operand_span` (shared
    // helper, DRY per §23 rule 5). Callers now use
    // `crate::mir::place::operand_span(op)` directly. Previously duplicated
    // as a private method here (Stage 15.81) and in `borrowck::mod` (Stage
    // 15.85).
}
