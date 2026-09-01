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

/// Stage 18.351: Helper — does a Ty (recursively) contain any Param?
///
/// Used by `infer_projection` Field arm to decide whether to apply
/// `substitute(field_ty, base_substs)` for unsubstituted generic placeholders.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit predicate makes the
/// "needs substitution" check visible at every callsite.
/// Per §16: pure MIR data predicate, no HIR access.
fn type_contains_param(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Param(_) => true,
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            type_contains_param(inner)
        }
        TyKind::Array(elem, _) => type_contains_param(elem),
        TyKind::Tuple(tys) => tys.iter().any(type_contains_param),
        TyKind::Adt(_, substs) => substs.iter().any(type_contains_param),
        TyKind::Closure(_, substs) => substs.iter().any(type_contains_param),
        TyKind::FnDef(_, substs) => substs.iter().any(type_contains_param),
        _ => false,
    }
}

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
            Rvalue::Load(_, _) | Rvalue::GetElementPtr { .. } | Rvalue::SizeOf(_) => {
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
                    // Stage 18.428 (§20 iterative audit): Push error for
                    // Deref on concrete non-pointer types (Int, Bool, Str,
                    // Tuple, Array, Adt, Float, Char). Was: silently returned
                    // TyKind::Error without pushing error → `*42`, `*true`,
                    // `*"hello"` etc. silently compiled.
                    //
                    // Per §20 (iterative audit): same class as Stage 18.412/
                    // 18.416/18.420/18.422/18.425/18.426 — silent acceptance
                    // of invalid operations.
                    // Per §1.0 原則 4 (报错 > 静默): Deref on non-pointer
                    // types must be reported.
                    // Per §1.0 原則 9 (正确 > 妥协): defer for Infer/Error/
                    // Param (pattern bindings + closure captures produce
                    // Deref projections on Infer types — don't push
                    // false-positive errors on unresolved types).
                    let is_deferred = matches!(
                        &base_ty.kind,
                        TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) | TyKind::Closure(_, _)
                    );
                    if !is_deferred {
                        self.errors.push(TypeError::new(
                            format!(
                                "cannot dereference type `{}` — only references and pointers can be dereferenced",
                                crate::mir::ty::type_to_string(base_ty)
                            ),
                            place_span,
                        ));
                    }
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
                // Stage 18.351 (P2 soundness fix): Apply generic substs from
                // the base struct's resolved type to the field_ty.
                //
                // Why this is needed in typeck (not just writeback):
                // typeck runs BEFORE writeback (driver order: typeck →
                // writeback_type_propagation). At typeck time, the
                // `field_ty` stored in `ProjectionElem::Field(_, field_ty)`
                // may still contain unsubstituted `Param(N)` placeholders
                // (because MIR lower's `resolve_field_type` couldn't
                // resolve substs at lower time). Without applying substs
                // here, typeck sees `*mut Param(0)` instead of `*mut i64`,
                // producing false "expected *mut i64, found *mut <type param>"
                // errors.
                //
                // Per §1.0 原則 3 (显式 > 隐式): explicit subst in typeck.
                // Per §1.0 原則 6 (通解 > 特解): one subst path for all
                // generic struct field accesses in typeck (mirrors
                // writeback Rule 3 + codegen detect_place_type fix from
                // Stage 18.347).
                // Per §20 (iterative audit): same class as Stage 18.347
                // (Param leak in field projection) — typeck path was missed.
                if type_contains_param(field_ty) {
                    if let TyKind::Adt(_, substs) = &base_ty.kind {
                        if !substs.is_empty() {
                            return crate::mir::substitute::substitute(field_ty, substs);
                        }
                    }
                    // Also handle Ref-to-Adt base (e.g., `&self.field`).
                    if let TyKind::Ref(_, _, inner) = &base_ty.kind {
                        if let TyKind::Adt(_, substs) = &inner.kind {
                            if !substs.is_empty() {
                                return crate::mir::substitute::substitute(field_ty, substs);
                            }
                        }
                    }
                }
                field_ty.clone()
            }
            ProjectionElem::Index(idx_local) => {
                // Stage 18.424 (§20 iterative audit): Align typeck with
                // Stage 18.422 MIR lower fix — `&str` indexing is now a
                // typeck error (was: returned u8 here, contradicting the
                // MIR lower side which now rejects). Also push errors for
                // non-indexable concrete types (Int, Bool, Float, Adt,
                // Tuple) — was: silently returned None → no error.
                //
                // Per §20 (iterative audit): same class as Stage 18.422 —
                // silent acceptance of invalid operations / design divergence.
                // Per §1.0 原則 4 (报错 > 静默): non-indexable types must error.
                // Per §1.0 原則 9 (正确 > 妥协): defer for Infer/Error/Param
                // (don't push false-positive errors on unresolved types).
                let inner_ty = match &base_ty.kind {
                    TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                    // Stage 18.424: REMOVED `TyKind::Str => Some(u8)` — &str
                    // indexing is now a typeck error (consistency with Stage
                    // 18.422 MIR lower fix). Users must use `.as_bytes()[i]`.
                    TyKind::Ref(_, _, inner) => {
                        // For &Array, &Slice: index returns element type.
                        // For &Str: now an error (Stage 18.424).
                        // For &Infer or &Error: defer (don't push error).
                        match &inner.kind {
                            TyKind::Array(inner, _) | TyKind::Slice(inner) => {
                                Some((**inner).clone())
                            }
                            // Stage 18.424: REMOVED `TyKind::Str => Some(u8)`.
                            TyKind::Str => {
                                // &str indexing is now an error — push it.
                                self.errors.push(TypeError::new(
                                    "cannot index into type `&str` — use `.as_bytes()[i]` for byte access or `.chars().nth(i)` for char access".to_string(),
                                    place_span,
                                ));
                                None
                            }
                            TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None, // defer
                            _ => {
                                // Non-indexable concrete type behind &Ref
                                // (e.g., &i32, &bool, &struct). Push error.
                                self.errors.push(TypeError::new(
                                    format!(
                                        "cannot index into type `&{}`",
                                        crate::mir::ty::type_to_string(inner)
                                    ),
                                    place_span,
                                ));
                                None
                            }
                        }
                    }
                    // Stage 18.62: Infer/Error/Param are acceptable fallbacks.
                    TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
                    _ => {
                        // Stage 18.424: Push error for non-indexable concrete
                        // types (Int, Bool, Float, Adt, Tuple, etc.).
                        // Was: silently returned None → no error → `n[0]`
                        // on integer silently compiled.
                        self.errors.push(TypeError::new(
                            format!(
                                "cannot index into type `{}`",
                                crate::mir::ty::type_to_string(base_ty)
                            ),
                            place_span,
                        ));
                        None
                    }
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
                // Stage 18.424 (§20 iterative audit): Same alignment as Index arm
                // — removed `TyKind::Str => Some(u8)` and push errors for
                // non-indexable concrete types. Per §1.0 原則 4 (报错 > 静默).
                let inner_ty = match &base_ty.kind {
                    TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                    // Stage 18.424: REMOVED `TyKind::Str => Some(u8)`.
                    TyKind::Ref(_, _, inner) => match &inner.kind {
                        TyKind::Array(inner, _) | TyKind::Slice(inner) => Some((**inner).clone()),
                        // Stage 18.424: REMOVED `TyKind::Str => Some(u8)`.
                        TyKind::Str => {
                            self.errors.push(TypeError::new(
                                "cannot index into type `&str` — use `.as_bytes()[i]`".to_string(),
                                place_span,
                            ));
                            None
                        }
                        TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None, // defer
                        _ => {
                            self.errors.push(TypeError::new(
                                format!(
                                    "cannot index into type `&{}`",
                                    crate::mir::ty::type_to_string(inner)
                                ),
                                place_span,
                            ));
                            None
                        }
                    },
                    // Stage 18.76: Defer for unresolved types.
                    TyKind::Infer(_) | TyKind::Error | TyKind::Param(_) => None,
                    _ => {
                        // Stage 18.424: Push error for non-indexable concrete types.
                        self.errors.push(TypeError::new(
                            format!(
                                "cannot index into type `{}`",
                                crate::mir::ty::type_to_string(base_ty)
                            ),
                            place_span,
                        ));
                        None
                    }
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
                    // Stage 18.416 (§20 iterative audit): Add type check for
                    // BitAnd/BitOr/BitXor — same class as Stage 18.412 Shl/Shr
                    // fix. Without this check, `"hello" & "world"` and
                    // `[1,2,3] | [4,5,6]` silently pass typeck (unify succeeds
                    // because both operands are the same type), then codegen's
                    // `_ => "add i32"` fallback emits wrong LLVM IR for the
                    // non-integer operands.
                    //
                    // Was: only `unify(a, b)` — returned a_ty without checking
                    // that a_ty is Bool or Int/Uint. For `"hello" & "world"`,
                    // unify(&str, &str) succeeds → no error → silent acceptance.
                    //
                    // Fix: Check `is_notable_ty(&a_ty)` BEFORE unify. If a is
                    // not Bool/Int/Uint, report error and skip unify (avoids
                    // double-reporting for `"hello" & 1` where both the
                    // notability check and unify would fail).
                    //
                    // Per §20 (iterative audit): same class as Stage 18.412
                    // (Shl/Shr lhs check). Finding one BinaryOp type-check bug
                    // means auditing ALL BinaryOp arms.
                    // Per §1.0 原則 4 (报错 > 静默): bitwise op on non-Bool/
                    // non-Int types must be reported at typeck.
                    // Per §1.0 原則 6 (通解 > 特解): one is_notable_ty check
                    // covers all non-Bool/non-Int types (Float, Str, Array,
                    // Tuple, Adt, Unit, etc.).
                    BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                        if !is_notable_ty(&a_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "bitwise op requires Bool or integer type, found {}",
                                    self.format_ty(&a_ty)
                                ),
                                stmt_span,
                            ));
                        } else if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
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
                    // Stage 18.412 (v0.5+ Phase 2 L3 step 2 root-cause fix):
                    // Add LHS type check — shift lhs must be Int/Uint (not
                    // Bool, Float, Str, Unit, Adt, etc.). This catches
                    // `&str << 2` and `() << 2` at typeck, eliminating the
                    // need for writeback_binaryop_results (Pass 2) which
                    // was a workaround that overwrote dest_local.ty to
                    // i32 (from b_ty) so codegen would catch the mismatch.
                    //
                    // Was: only checked `is_shift_count_ty(&b_ty)` — the
                    // Shl arm returned `a_ty` (e.g. `&str`) without error.
                    // Pass 2 then masked this by overwriting dest to i32.
                    //
                    // Per §1.0 原則 4 (报错 > 静默): LHS type error must be
                    // reported at typeck, not masked by writeback.
                    // Per §1.6 终极检验: root-cause fix at typeck, not a
                    // writeback patch.
                    // Per §12 (最优 > 最小): one LHS check covers all
                    // non-integer LHS types (Bool, Float, Str, Unit, Adt).
                    // Per §1.0 原則 6 (通解 > 特解): one LHS check replaces
                    // the per-call-site writeback overwrite.
                    BinOp::Shl | BinOp::Shr => {
                        if !is_shift_count_ty(&a_ty) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "shift lhs must be an integer type, found {}",
                                    self.format_ty(&a_ty)
                                ),
                                stmt_span,
                            ));
                        }
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
                    //
                    // Stage 18.236 (Pointer Arithmetic): Allow `ptr + int` and
                    // `ptr - int` when one operand is RawPtr and the other is
                    // integer. Result type = the RawPtr type. This reuses the
                    // existing GetElementPtr MIR lowering (§1.0 原則 6 通解).
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                        // Stage 18.236: Check for pointer arithmetic (ptr + int).
                        let a_is_ptr = matches!(&a_ty.kind, crate::mir::ty::TyKind::RawPtr(_, _));
                        let b_is_ptr = matches!(&b_ty.kind, crate::mir::ty::TyKind::RawPtr(_, _));
                        let a_is_int = matches!(
                            &a_ty.kind,
                            crate::mir::ty::TyKind::Int(_)
                                | crate::mir::ty::TyKind::Uint(_)
                                | crate::mir::ty::TyKind::Infer(_)
                                | crate::mir::ty::TyKind::Error
                        );
                        let b_is_int = matches!(
                            &b_ty.kind,
                            crate::mir::ty::TyKind::Int(_)
                                | crate::mir::ty::TyKind::Uint(_)
                                | crate::mir::ty::TyKind::Infer(_)
                                | crate::mir::ty::TyKind::Error
                        );
                        // ptr + int or int + ptr → result = ptr type (Add only)
                        // ptr - int → result = ptr type (Sub only)
                        // ptr + ptr, ptr - ptr, ptr * int etc. → error
                        if (a_is_ptr && b_is_int) || (a_is_int && b_is_ptr) {
                            if matches!(op, BinOp::Add | BinOp::Sub) {
                                // Result is the pointer type.
                                if a_is_ptr {
                                    a_ty
                                } else {
                                    b_ty
                                }
                            } else {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "cannot apply {} to pointer (only + and - are supported)",
                                        format!("{:?}", op).to_lowercase()
                                    ),
                                    stmt_span,
                                ));
                                a_ty
                            }
                        } else {
                            // Standard arithmetic: both must be Int/Uint/Float.
                            if !is_arithmetic_ty(&a_ty) {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "cannot apply arithmetic to {} (expected integer or float)",
                                        self.format_ty(&a_ty)
                                    ),
                                    stmt_span,
                                ));
                            }
                            if !is_arithmetic_ty(&b_ty) {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "cannot apply arithmetic to {} (expected integer or float)",
                                        self.format_ty(&b_ty)
                                    ),
                                    stmt_span,
                                ));
                            }
                            if let Err(mut e) = self.unify.unify(&a_ty, &b_ty, stmt_span) {
                                if stmt_span != Span::DUMMY {
                                    e.span = stmt_span;
                                }
                                self.errors.push(*e);
                            }
                            a_ty
                        }
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
            Rvalue::Load(_, _) | Rvalue::GetElementPtr { .. } | Rvalue::SizeOf(_) => {
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
            Rvalue::Cast(kind, operand, target_ty) => {
                // Stage 18.426 (§20 iterative audit): Validate cast
                // validity — reject invalid casts like `"hello" as i32`
                // (Str→Int), `true as &str` (Bool→Str), `(1,2) as i32`
                // (Tuple→Int), `42 as Foo` (Int→Adt), etc.
                //
                // Was: `Rvalue::Cast(_, _, target_ty) => target_ty.clone()`
                // — returned target_ty without checking source type →
                // silently accepted invalid casts, codegen fell through to
                // `_ => "bitcast"` fallback producing wrong/invalid LLVM IR.
                //
                // Per §20 (iterative audit): same class as Stage 18.412/
                // 18.416/18.420/18.422/18.425 — silent acceptance of invalid
                // operations.
                // Per §1.0 原則 4 (报错 > 静默): invalid casts must be rejected.
                // Per §1.0 原則 6 (通解 > 特解): one check covers all invalid
                // cast pairs.
                // Per §1.6 终极检验: root-cause fix at typeck, not codegen.
                let src_ty = self.infer_operand(mir, operand);
                let src_ty = self.unify.resolve(&src_ty);
                let dst_ty = self.unify.resolve(target_ty);
                if !Self::is_valid_cast(&src_ty, &dst_ty, *kind) {
                    self.errors.push(TypeError::new(
                        format!(
                            "invalid cast: `{}` as `{}` — non-numeric/non-pointer types cannot be cast",
                            self.format_ty(&src_ty),
                            self.format_ty(&dst_ty)
                        ),
                        stmt_span,
                    ));
                }
                target_ty.clone()
            }
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
                    // Stage 18.255 (TD-TUPLE-CTOR-TYPECK Phase 1): swap unify
                    // arg order — declared type is "expected", actual value is
                    // "found". Previously `unify(&op_ty, elem_ty)` produced
                    // "expected <actual>, found <declared>" which is reversed.
                    // Per §2 原则 3 (显式 > 隐式): error messages must match
                    // user's mental model (declared type is expected).
                    for op in operands {
                        let op_ty = self.infer_operand(mir, op);
                        if let Err(mut e) = self.unify.unify(elem_ty, &op_ty, stmt_span) {
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
                // Stage 18.255 (TD-TUPLE-CTOR-TYPECK Phase 1): swap unify
                // arg order — declared field type is "expected", actual
                // operand type is "found". Previously `unify(&op_ty, field_ty)`
                // produced "expected <actual>, found <declared>" which is
                // reversed. Per §2 原则 3 (显式 > 隐式) + §2 原则 9
                // (正确 > 妥协): error messages must reflect Rust semantics.
                AggregateKind::Adt(def_id, _variant, _substs, field_tys) => {
                    for (i, op) in operands.iter().enumerate() {
                        let op_ty = self.infer_operand(mir, op);
                        if let Some(field_ty) = field_tys.get(i) {
                            if let Err(mut e) = self.unify.unify(field_ty, &op_ty, stmt_span) {
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

    /// Stage 18.426 (§20 iterative audit): Check if a cast from `src_ty` to
    /// `dst_ty` with `kind` is valid.
    ///
    /// Valid casts (matching Rust semantics + Landin codegen support):
    /// - Numeric: Int↔Int, Int↔Uint, Uint↔Uint, Float↔Float, Int↔Float,
    ///   Uint↔Float
    /// - Bool→Int/Uint (Rust allows `true as i32`)
    /// - Int↔Ptr (inttoptr / ptrtoint)
    /// - Ptr↔Ptr (bitcast)
    /// - Unsize: &[T; N] → &[T], &str → &[u8] (CastKind::Unsize)
    ///
    /// Invalid casts (rejected):
    /// - Str→anything, anything→Str (use .as_bytes() / .to_string())
    /// - Tuple→anything, anything→Tuple
    /// - Adt→anything, anything→Adt (except via Unsize/Deref)
    /// - Array→anything (except via Unsize to slice)
    /// - FnDef→anything (except FnDef→FnPtr via Unsize/reify)
    ///
    /// Per §1.0 原則 4 (报错 > 静默): invalid casts must be rejected.
    /// Per §1.0 原則 6 (通解 > 特解): one validity check covers all pairs.
    /// Per §1.0 原則 9 (正确 > 妥协): defer for Infer/Error (don't push
    /// false-positive errors on unresolved types).
    fn is_valid_cast(src_ty: &Ty, dst_ty: &Ty, kind: CastKind) -> bool {
        // Defer for unresolved types — don't push false-positive errors.
        // Stage 18.426: if src is Infer (e.g., `42` without suffix), allow
        // cast to any castable dst type (numeric, pointer). Reject only
        // when dst is a clearly non-castable concrete type (Adt, Array,
        // Tuple, Str, etc.).
        let src_defers = matches!(
            &src_ty.kind,
            TyKind::Infer(_) | TyKind::Error | TyKind::Param(_)
        );
        let dst_defers = matches!(
            &dst_ty.kind,
            TyKind::Infer(_) | TyKind::Error | TyKind::Param(_)
        );
        if src_defers && dst_defers {
            return true;
        }
        // Unsize casts are always valid (checked at codegen level).
        if matches!(kind, CastKind::Unsize) {
            return true;
        }
        // Stage 18.426: If src is Infer, allow cast to any castable dst
        // (numeric, pointer, char, bool). Reject only for non-castable
        // concrete dst (Str, Tuple, Adt, Array).
        if src_defers {
            let dst_is_castable = matches!(
                &dst_ty.kind,
                TyKind::Int(_)
                    | TyKind::Uint(_)
                    | TyKind::Float(_)
                    | TyKind::Bool
                    | TyKind::Char
                    | TyKind::RawPtr(_, _)
                    | TyKind::Ref(_, _, _)
            );
            return dst_is_castable;
        }
        // Numeric casts (matching Rust Reference §5.2.7):
        // - Int/Uint <-> Int/Uint: OK
        // - Float <-> Float: OK
        // - Int/Uint -> Float: OK
        // - Float -> Int/Uint: OK
        // - Char -> Int/Uint: OK
        // - Int/Uint -> Char: OK
        // - Bool -> Int/Uint: OK (Rust allows `true as i32`)
        // - Int/Uint -> Bool: OK (Rust allows `1 as bool`)
        // - Bool -> Bool/Float/Char: NOT OK (Rust rejects)
        // - Float -> Bool/Char: NOT OK (Rust rejects)
        let src_is_int_uint = matches!(&src_ty.kind, TyKind::Int(_) | TyKind::Uint(_));
        let dst_is_int_uint = matches!(&dst_ty.kind, TyKind::Int(_) | TyKind::Uint(_));
        let src_is_float = matches!(&src_ty.kind, TyKind::Float(_));
        let dst_is_float = matches!(&dst_ty.kind, TyKind::Float(_));
        let src_is_char = matches!(&src_ty.kind, TyKind::Char);
        let dst_is_char = matches!(&dst_ty.kind, TyKind::Char);
        let src_is_bool = matches!(&src_ty.kind, TyKind::Bool);
        let dst_is_bool = matches!(&dst_ty.kind, TyKind::Bool);
        if src_is_int_uint && dst_is_int_uint {
            return true;
        }
        if src_is_float && dst_is_float {
            return true;
        }
        if src_is_int_uint && dst_is_float {
            return true;
        }
        if src_is_float && dst_is_int_uint {
            return true;
        }
        if src_is_char && dst_is_int_uint {
            return true;
        }
        if src_is_int_uint && dst_is_char {
            return true;
        }
        if src_is_bool && dst_is_int_uint {
            return true;
        }
        if src_is_int_uint && dst_is_bool {
            return true;
        }
        // Int <-> Ptr casts.
        // Stage 18.426: Int -> &str is INVALID (cannot cast int to string ref).
        // Other Int -> Ptr casts are valid (inttoptr / ptrtoint).
        // Stage 18.426: Ptr -> Int is allowed (including &str -> usize) because
        // the format! variadic intrinsic casts all args to usize for printf.
        // This is a pragmatic allowance — the root-cause fix is to make the
        // format intrinsic handle &str args via .len() instead of cast.
        // Per §1.0 原則 9 (正确 > 妥协): allow Ptr→Int for format! intrinsic.
        // Per §5.2: documented as known limitation (format intrinsic design).
        let dst_is_ptr = matches!(&dst_ty.kind, TyKind::RawPtr(_, _) | TyKind::Ref(_, _, _));
        let dst_is_str_ref = matches!(
            &dst_ty.kind,
            TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str)
        );
        let src_is_ptr = matches!(&src_ty.kind, TyKind::RawPtr(_, _) | TyKind::Ref(_, _, _));
        let src_is_str_ref = matches!(
            &src_ty.kind,
            TyKind::Ref(_, _, inner) if matches!(inner.kind, TyKind::Str)
        );
        // Int -> Ptr (but NOT Int -> &str)
        if src_is_int_uint && dst_is_ptr && !dst_is_str_ref {
            return true;
        }
        // Ptr -> Int (allow all, including &str -> usize for format! intrinsic)
        if src_is_ptr && dst_is_int_uint {
            return true;
        }
        // Ptr <-> Ptr casts (but NOT involving &str).
        // Stage 18.426: &str <-> &T is invalid (str is unsized, cannot cast).
        if src_is_ptr && dst_is_ptr && !src_is_str_ref && !dst_is_str_ref {
            return true;
        }
        // FnDef -> FnPtr (reify pointer).
        if matches!(&src_ty.kind, TyKind::FnDef(_, _)) && matches!(&dst_ty.kind, TyKind::FnPtr(_)) {
            return true;
        }
        // All other casts are invalid (Str, Tuple, Adt, Array,
        // Bool->Bool/Float/Char, Float->Bool/Char, etc.).
        false
    }
}
