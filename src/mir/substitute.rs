//! Stage 16.53 (Task 11 Phase 2): Type substitution — replace `TyKind::Param`
//! with concrete types from a `SubstsRef` slice.
//!
//! This module provides the `substitute` function, which is the core
//! algorithm for monomorphization. Given a generic type (e.g., the field
//! type `T` in `struct Box<T> { val: T }`) and a substitution slice
//! (e.g., `[i32]`), `substitute` produces the concrete type (`i32`).
//!
//! ## Algorithm
//!
//! `substitute` walks the `Ty` tree recursively:
//! - `Param(idx)` → `substs[idx]` (the replacement)
//! - `Adt(def_id, inner_substs)` → recursively substitute each inner subst
//! - `Ref(_, _, inner)` → recursively substitute inner
//! - `Tuple(tys)` → recursively substitute each element
//! - `Array(inner, _)` → recursively substitute inner
//! - `Slice(inner)` → recursively substitute inner
//! - `RawPtr(_, inner)` → recursively substitute inner
//! - `FnDef(_, inner_substs)` → recursively substitute each inner subst
//! - `Closure(_, inner_substs)` → recursively substitute each inner subst
//! - `FnPtr(sig)` → recursively substitute inputs + output
//! - All other kinds (Bool, Int, Str, Error, etc.) → clone unchanged
//!
//! ## Bounds Safety
//!
//! If `Param(idx)` has `idx >= substs.len()`, the original `Param` is
//! returned unchanged (no panic). This handles the case where a generic
//! type is used without full substitution (e.g., partially substituted
//! during intermediate typeck steps). The caller is responsible for
//! ensuring substs are complete before relying on the result.
//!
//! ## Performance
//!
//! `substitute` is a pure function — no HIR access, no side effects.
//! It allocates a new `Ty` for each substituted node. For deeply nested
//! types, this is O(n) in the type tree size. Future optimization: use
//! interning to share unchanged subtrees (Stage 15.26 TypeInterner can
//! dedup identical `Ty` values).
//!
//! Per §23: `substitute` follows `<verb>` pattern (pure function).
//! Per §16: this module reads `Ty` only (no HIR, no resolver).
//! Per §1.0 原則 6 "通用 > 特例": one substitute function for all type
//! kinds, dispatched via match.

use crate::mir::ty::{Const, Sig, SubstsRef, Ty, TyKind};
use crate::session::Span;

/// Substitute type parameters in a `Ty` with concrete types from `substs`.
///
/// Given `ty = Adt(Box_def_id, [Param(0)])` and `substs = [i32]`, produces
/// `Adt(Box_def_id, [i32])`.
///
/// Given `ty = Param(ParamTy { index: 0, .. })` and `substs = [i32]`,
/// produces `i32`.
///
/// Per §23: `substitute` follows `<verb>` pattern (pure function).
/// Per §1.0 原則 3 "显式 > 隐式": the substs slice is explicit.
/// Per §1.0 原則 6 "通用 > 特例": one function for all type kinds.
pub fn substitute(ty: &Ty, substs: &[Ty]) -> Ty {
    // Note: Ty is interned (Stage 15.28) — it has no span field. We use
    // Span::DUMMY for new Ty construction (the span is not preserved
    // through interning; this is acceptable since substitute is a type
    // transformation, not a source-position-preserving operation).
    let _ = Span::DUMMY;
    match &ty.kind {
        // === Leaf types: no substs to replace ===
        TyKind::Bool
        | TyKind::Char
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Str
        | TyKind::Never
        | TyKind::Foreign
        | TyKind::Error => ty.clone(),

        // === Inference variables: not substitutable ===
        // (Infer vars are resolved by typeck, not by substitution)
        TyKind::Infer(_) => ty.clone(),

        // === Param: the core replacement ===
        TyKind::Param(param_ty) => {
            // Bounds-safe: if index is out of range, return the original
            // Param unchanged. This handles partial substitution during
            // intermediate typeck steps.
            substs
                .get(param_ty.index as usize)
                .cloned()
                .unwrap_or_else(|| ty.clone())
        }

        // === Recursive types: substitute inner ===
        TyKind::Ref(region, mutability, inner) => Ty::new(
            TyKind::Ref(*region, *mutability, Box::new(substitute(inner, substs))),
            Span::DUMMY,
        ),
        TyKind::RawPtr(mutability, inner) => Ty::new(
            TyKind::RawPtr(*mutability, Box::new(substitute(inner, substs))),
            Span::DUMMY,
        ),
        TyKind::Array(inner, len) => Ty::new(
            TyKind::Array(
                Box::new(substitute(inner, substs)),
                Box::new(substitute_const(len, substs)),
            ),
            Span::DUMMY,
        ),
        TyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(substitute(inner, substs))),
            Span::DUMMY,
        ),
        TyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(tys.iter().map(|t| substitute(t, substs)).collect()),
            Span::DUMMY,
        ),

        // === Generic-capable types: substitute inner substs ===
        TyKind::Adt(def_id, inner_substs) => {
            let new_substs: Vec<Ty> = inner_substs.iter().map(|t| substitute(t, substs)).collect();
            Ty::new(TyKind::Adt(*def_id, new_substs.into()), Span::DUMMY)
        }
        TyKind::FnDef(def_id, inner_substs) => {
            let new_substs: Vec<Ty> = inner_substs.iter().map(|t| substitute(t, substs)).collect();
            Ty::new(TyKind::FnDef(*def_id, new_substs.into()), Span::DUMMY)
        }
        TyKind::Closure(def_id, inner_substs) => {
            let new_substs: Vec<Ty> = inner_substs.iter().map(|t| substitute(t, substs)).collect();
            Ty::new(TyKind::Closure(*def_id, new_substs.into()), Span::DUMMY)
        }

        // === FnPtr: substitute inputs + output ===
        TyKind::FnPtr(sig) => Ty::new(TyKind::FnPtr(substitute_sig(sig, substs)), Span::DUMMY),
    }
}

/// Substitute type parameters in a `Const` (used for array lengths).
///
/// Array lengths are compile-time constants. If the length is a const
/// expression that involves a generic param (e.g., `[T; N]` where N is
/// a const generic), the const's type may need substitution. The const
/// value itself is not substituted (it's a value, not a type).
///
/// Per §23: `substitute_const` follows `<verb>_<noun>` pattern.
fn substitute_const(c: &Const, substs: &[Ty]) -> Const {
    Const {
        ty: substitute(&c.ty, substs),
        val: c.val.clone(),
    }
}

/// Substitute type parameters in a function signature.
///
/// Per §23: `substitute_sig` follows `<verb>_<noun>` pattern.
fn substitute_sig(sig: &Sig, substs: &[Ty]) -> Sig {
    Sig {
        inputs: sig.inputs.iter().map(|t| substitute(t, substs)).collect(),
        output: Box::new(substitute(&sig.output, substs)),
        abi: sig.abi,
        is_unsafe: sig.is_unsafe,
    }
}

/// Substitute type parameters in a `SubstsRef` slice.
///
/// This is useful when you have nested substs (e.g., `Vec<Vec<i32>>`
/// has outer substs `[Vec<i32>]` and the inner `Vec<i32>` has substs
/// `[i32]`). Applying the outer substs to the inner substs produces
/// the fully substituted slice.
///
/// Per §23: `substitute_substs` follows `<verb>_<noun>` pattern.
pub fn substitute_substs(inner_substs: &SubstsRef, outer_substs: &[Ty]) -> Vec<Ty> {
    inner_substs
        .iter()
        .map(|t| substitute(t, outer_substs))
        .collect()
}

// =====================================================================
// Unit Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Abi, FloatTy, IntTy, UintTy};
    use crate::hir::DefId;
    use crate::mir::ty::{InferVar, TyVid};
    use crate::session::Span;

    /// Helper: create a Ty of the given kind with DUMMY span.
    fn ty(kind: TyKind) -> Ty {
        Ty::new(kind, Span::DUMMY)
    }

    /// Helper: create a ParamTy.
    fn param(index: u32) -> Ty {
        ty(TyKind::Param(crate::mir::ty::ParamTy {
            index,
            name: crate::lexer::Symbol::default(),
        }))
    }

    /// Helper: create an i32 Ty.
    fn i32_ty() -> Ty {
        ty(TyKind::Int(IntTy::I32))
    }

    /// Helper: create a bool Ty.
    fn bool_ty() -> Ty {
        ty(TyKind::Bool)
    }

    // =================================================================
    // §1. Leaf types — no substitution needed
    // =================================================================

    #[test]
    fn stage16_53_substitute_leaf_bool() {
        let t = bool_ty();
        let result = substitute(&t, &[i32_ty()]);
        assert_eq!(result.kind, TyKind::Bool);
    }

    #[test]
    fn stage16_53_substitute_leaf_int() {
        let t = i32_ty();
        let result = substitute(&t, &[bool_ty()]);
        assert_eq!(result.kind, TyKind::Int(IntTy::I32));
    }

    #[test]
    fn stage16_53_substitute_leaf_str() {
        let t = ty(TyKind::Str);
        let result = substitute(&t, &[i32_ty()]);
        assert_eq!(result.kind, TyKind::Str);
    }

    #[test]
    fn stage16_53_substitute_leaf_never() {
        let t = ty(TyKind::Never);
        let result = substitute(&t, &[i32_ty()]);
        assert_eq!(result.kind, TyKind::Never);
    }

    #[test]
    fn stage16_53_substitute_leaf_error() {
        let t = ty(TyKind::Error);
        let result = substitute(&t, &[i32_ty()]);
        assert_eq!(result.kind, TyKind::Error);
    }

    // =================================================================
    // §2. Param — the core replacement
    // =================================================================

    #[test]
    fn stage16_53_substitute_param_replaced() {
        let t = param(0);
        let result = substitute(&t, &[i32_ty()]);
        assert_eq!(result.kind, TyKind::Int(IntTy::I32));
    }

    #[test]
    fn stage16_53_substitute_param_second_index() {
        let t = param(1);
        let result = substitute(&t, &[i32_ty(), bool_ty()]);
        assert_eq!(result.kind, TyKind::Bool);
    }

    #[test]
    fn stage16_53_substitute_param_out_of_bounds() {
        let t = param(5);
        let result = substitute(&t, &[i32_ty()]);
        // Out of bounds → return original Param unchanged
        assert!(matches!(result.kind, TyKind::Param(_)));
    }

    #[test]
    fn stage16_53_substitute_param_empty_substs() {
        let t = param(0);
        let result = substitute(&t, &[]);
        // Empty substs → return original Param unchanged
        assert!(matches!(result.kind, TyKind::Param(_)));
    }

    // =================================================================
    // §3. Ref — substitute inner
    // =================================================================

    #[test]
    fn stage16_53_substitute_ref() {
        let inner = param(0);
        let t = ty(TyKind::Ref(
            crate::mir::ty::Region::Static,
            crate::mir::ty::Mutability::Immutable,
            Box::new(inner),
        ));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Ref(_, _, inner) => assert_eq!(inner.kind, TyKind::Int(IntTy::I32)),
            _ => panic!("expected Ref, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §4. RawPtr — substitute inner
    // =================================================================

    #[test]
    fn stage16_53_substitute_raw_ptr() {
        let inner = param(0);
        let t = ty(TyKind::RawPtr(
            crate::mir::ty::Mutability::Mutable,
            Box::new(inner),
        ));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::RawPtr(_, inner) => assert_eq!(inner.kind, TyKind::Int(IntTy::I32)),
            _ => panic!("expected RawPtr, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §5. Array — substitute inner element type
    // =================================================================

    #[test]
    fn stage16_53_substitute_array() {
        let inner = param(0);
        let len = Const {
            ty: i32_ty(),
            val: crate::mir::ty::ConstVal::Uint(10),
        };
        let t = ty(TyKind::Array(Box::new(inner), Box::new(len)));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Array(inner, _) => assert_eq!(inner.kind, TyKind::Int(IntTy::I32)),
            _ => panic!("expected Array, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §6. Slice — substitute inner
    // =================================================================

    #[test]
    fn stage16_53_substitute_slice() {
        let inner = param(0);
        let t = ty(TyKind::Slice(Box::new(inner)));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Slice(inner) => assert_eq!(inner.kind, TyKind::Int(IntTy::I32)),
            _ => panic!("expected Slice, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §7. Tuple — substitute each element
    // =================================================================

    #[test]
    fn stage16_53_substitute_tuple() {
        let t = ty(TyKind::Tuple(vec![param(0), param(1), i32_ty()]));
        let result = substitute(&t, &[bool_ty(), ty(TyKind::Uint(UintTy::U64))]);
        match result.kind {
            TyKind::Tuple(tys) => {
                assert_eq!(tys.len(), 3);
                assert_eq!(tys[0].kind, TyKind::Bool);
                assert_eq!(tys[1].kind, TyKind::Uint(UintTy::U64));
                assert_eq!(tys[2].kind, TyKind::Int(IntTy::I32));
            }
            _ => panic!("expected Tuple, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §8. Adt — substitute inner substs
    // =================================================================

    #[test]
    fn stage16_53_substitute_adt() {
        let inner_substs: Vec<Ty> = vec![param(0)];
        let t = ty(TyKind::Adt(DefId::new(1), inner_substs.into()));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Adt(def_id, substs) => {
                assert_eq!(def_id, DefId::new(1));
                assert_eq!(substs.len(), 1);
                assert_eq!(substs[0].kind, TyKind::Int(IntTy::I32));
            }
            _ => panic!("expected Adt, got {:?}", result.kind),
        }
    }

    #[test]
    fn stage16_53_substitute_adt_multiple_substs() {
        let inner_substs: Vec<Ty> = vec![param(0), param(1)];
        let t = ty(TyKind::Adt(DefId::new(2), inner_substs.into()));
        let result = substitute(&t, &[i32_ty(), bool_ty()]);
        match result.kind {
            TyKind::Adt(_, substs) => {
                assert_eq!(substs.len(), 2);
                assert_eq!(substs[0].kind, TyKind::Int(IntTy::I32));
                assert_eq!(substs[1].kind, TyKind::Bool);
            }
            _ => panic!("expected Adt, got {:?}", result.kind),
        }
    }

    #[test]
    fn stage16_53_substitute_adt_empty_substs() {
        let t = ty(TyKind::Adt(DefId::new(3), Vec::new().into()));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Adt(def_id, substs) => {
                assert_eq!(def_id, DefId::new(3));
                assert!(substs.is_empty());
            }
            _ => panic!("expected Adt, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §9. FnDef — substitute inner substs
    // =================================================================

    #[test]
    fn stage16_53_substitute_fn_def() {
        let inner_substs: Vec<Ty> = vec![param(0)];
        let t = ty(TyKind::FnDef(DefId::new(5), inner_substs.into()));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::FnDef(_, substs) => {
                assert_eq!(substs.len(), 1);
                assert_eq!(substs[0].kind, TyKind::Int(IntTy::I32));
            }
            _ => panic!("expected FnDef, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §10. Closure — substitute inner substs
    // =================================================================

    #[test]
    fn stage16_53_substitute_closure() {
        let inner_substs: Vec<Ty> = vec![param(0)];
        let t = ty(TyKind::Closure(DefId::new(7), inner_substs.into()));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Closure(_, substs) => {
                assert_eq!(substs.len(), 1);
                assert_eq!(substs[0].kind, TyKind::Int(IntTy::I32));
            }
            _ => panic!("expected Closure, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §11. FnPtr — substitute inputs + output
    // =================================================================

    #[test]
    fn stage16_53_substitute_fn_ptr() {
        let sig = Sig {
            inputs: vec![param(0)],
            output: Box::new(param(0)),
            abi: Abi::Landin,
            is_unsafe: false,
        };
        let t = ty(TyKind::FnPtr(sig));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::FnPtr(sig) => {
                assert_eq!(sig.inputs.len(), 1);
                assert_eq!(sig.inputs[0].kind, TyKind::Int(IntTy::I32));
                assert_eq!(sig.output.kind, TyKind::Int(IntTy::I32));
            }
            _ => panic!("expected FnPtr, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §12. Infer — not substituted (resolved by typeck)
    // =================================================================

    #[test]
    fn stage16_53_substitute_infer() {
        let t = ty(TyKind::Infer(InferVar::TyVar(TyVid(42))));
        let result = substitute(&t, &[i32_ty()]);
        match result.kind {
            TyKind::Infer(InferVar::TyVar(TyVid(vid))) => assert_eq!(vid, 42),
            _ => panic!("expected Infer, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §13. Nested types — deep substitution
    // =================================================================

    #[test]
    fn stage16_53_substitute_nested_adt() {
        // Vec<Vec<T>> with substs [i32] → Vec<Vec<i32>>
        let inner_vec = ty(TyKind::Adt(DefId::new(1), vec![param(0)].into()));
        let outer_vec = ty(TyKind::Adt(DefId::new(1), vec![inner_vec].into()));
        let result = substitute(&outer_vec, &[i32_ty()]);
        match result.kind {
            TyKind::Adt(_, substs) => {
                assert_eq!(substs.len(), 1);
                match &substs[0].kind {
                    TyKind::Adt(_, inner_substs) => {
                        assert_eq!(inner_substs.len(), 1);
                        assert_eq!(inner_substs[0].kind, TyKind::Int(IntTy::I32));
                    }
                    _ => panic!("expected inner Adt, got {:?}", substs[0].kind),
                }
            }
            _ => panic!("expected outer Adt, got {:?}", result.kind),
        }
    }

    #[test]
    fn stage16_53_substitute_nested_ref_adt() {
        // &Box<T> with substs [i32] → &Box<i32>
        let box_ty = ty(TyKind::Adt(DefId::new(1), vec![param(0)].into()));
        let ref_ty = ty(TyKind::Ref(
            crate::mir::ty::Region::Static,
            crate::mir::ty::Mutability::Immutable,
            Box::new(box_ty),
        ));
        let result = substitute(&ref_ty, &[i32_ty()]);
        match result.kind {
            TyKind::Ref(_, _, inner) => match &inner.kind {
                TyKind::Adt(_, substs) => {
                    assert_eq!(substs.len(), 1);
                    assert_eq!(substs[0].kind, TyKind::Int(IntTy::I32));
                }
                _ => panic!("expected Adt, got {:?}", inner.kind),
            },
            _ => panic!("expected Ref, got {:?}", result.kind),
        }
    }

    #[test]
    fn stage16_53_substitute_tuple_of_params() {
        // (T, T, U) with substs [i32, bool] → (i32, i32, bool)
        let t = ty(TyKind::Tuple(vec![param(0), param(0), param(1)]));
        let result = substitute(&t, &[i32_ty(), bool_ty()]);
        match result.kind {
            TyKind::Tuple(tys) => {
                assert_eq!(tys.len(), 3);
                assert_eq!(tys[0].kind, TyKind::Int(IntTy::I32));
                assert_eq!(tys[1].kind, TyKind::Int(IntTy::I32));
                assert_eq!(tys[2].kind, TyKind::Bool);
            }
            _ => panic!("expected Tuple, got {:?}", result.kind),
        }
    }

    // =================================================================
    // §14. substitute_substs — substitute a substs slice
    // =================================================================

    #[test]
    fn stage16_53_substitute_substs_basic() {
        let inner: Vec<Ty> = vec![param(0), param(1)];
        let inner_substs: SubstsRef = inner.into();
        let result = substitute_substs(&inner_substs, &[i32_ty(), bool_ty()]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].kind, TyKind::Int(IntTy::I32));
        assert_eq!(result[1].kind, TyKind::Bool);
    }

    #[test]
    fn stage16_53_substitute_substs_empty() {
        let inner: Vec<Ty> = Vec::new();
        let inner_substs: SubstsRef = inner.into();
        let result = substitute_substs(&inner_substs, &[i32_ty()]);
        assert!(result.is_empty());
    }

    #[test]
    fn stage16_53_substitute_substs_no_params() {
        // Substs with no params → unchanged
        let inner: Vec<Ty> = vec![i32_ty(), bool_ty()];
        let inner_substs: SubstsRef = inner.into();
        let result = substitute_substs(&inner_substs, &[ty(TyKind::Float(FloatTy::F64))]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].kind, TyKind::Int(IntTy::I32));
        assert_eq!(result[1].kind, TyKind::Bool);
    }

    // =================================================================
    // §15. Idempotency — substituting with empty substs is a no-op
    // =================================================================

    #[test]
    fn stage16_53_substitute_empty_substs_idempotent() {
        let t = i32_ty();
        let result = substitute(&t, &[]);
        assert_eq!(result.kind, t.kind);
    }

    #[test]
    fn stage16_53_substitute_empty_substs_on_adt() {
        let inner_substs: Vec<Ty> = vec![i32_ty()];
        let t = ty(TyKind::Adt(DefId::new(1), inner_substs.into()));
        let result = substitute(&t, &[]);
        match result.kind {
            TyKind::Adt(_, substs) => {
                assert_eq!(substs.len(), 1);
                assert_eq!(substs[0].kind, TyKind::Int(IntTy::I32));
            }
            _ => panic!("expected Adt, got {:?}", result.kind),
        }
    }
}
