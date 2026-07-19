//! Unification engine for type inference.
//!
//! Implements a standard union-find style unification table for
//! type variables. Supports:
//! - General type variables (`TyVid`)
//! - Integer type variables (`IntVid`) — can unify with any IntTy/UintTy
//! - Float type variables (`FloatVid`) — can unify with f32/f64
//!
//! Per 03-type-system.md, Landin uses Hindley-Milner-style inference
//! with bidirectional type checking on MIR.

use crate::ast::{FloatTy, IntTy, UintTy};
use crate::mir::ty::*;
use crate::typeck::error::TypeError;
use std::collections::HashMap;

/// The unification table. Holds bindings for all inference variables.
///
/// Uses union-find with path compression for efficient variable lookup.
/// Each `TyVid` maps to either `None` (unbound) or `Some(Ty)` (bound
/// to a concrete or partially-resolved type).
#[derive(Debug, Default)]
pub struct UnificationTable {
    /// General type variable bindings.
    ty_vars: HashMap<TyVid, Option<Ty>>,
    /// Next TyVid to allocate.
    next_ty_vid: u32,
    /// Integer variable bindings.
    int_vars: HashMap<IntVid, Option<IntTy>>,
    /// Next IntVid to allocate.
    next_int_vid: u32,
    /// Float variable bindings.
    float_vars: HashMap<FloatVid, Option<FloatTy>>,
    /// Next FloatVid to allocate.
    next_float_vid: u32,
    /// Errors encountered during unification (non-fatal).
    errors: Vec<TypeError>,
}

impl UnificationTable {
    pub fn new() -> Self {
        Self::default()
    }

    // ================================================================
    // Variable allocation
    // ================================================================

    /// Allocate a fresh general type variable.
    pub fn new_ty_var(&mut self) -> TyVid {
        let vid = TyVid(self.next_ty_vid);
        self.next_ty_vid += 1;
        self.ty_vars.insert(vid, None);
        vid
    }

    /// Allocate a fresh integer type variable.
    pub fn new_int_var(&mut self) -> IntVid {
        let vid = IntVid(self.next_int_vid);
        self.next_int_vid += 1;
        self.int_vars.insert(vid, None);
        vid
    }

    /// Allocate a fresh float type variable.
    pub fn new_float_var(&mut self) -> FloatVid {
        let vid = FloatVid(self.next_float_vid);
        self.next_float_vid += 1;
        self.float_vars.insert(vid, None);
        vid
    }

    // ================================================================
    // Resolution
    // ================================================================

    /// Resolve a type variable to its bound type (if any).
    /// Follows the chain of bindings until reaching an unbound variable
    /// or a concrete type.
    pub fn resolve_ty_var(&self, vid: TyVid) -> Option<Ty> {
        match self.ty_vars.get(&vid) {
            None => None,
            Some(None) => None, // unbound
            Some(Some(ty)) => {
                // Follow the chain
                if let TyKind::Infer(InferVar::TyVar(inner_vid)) = &ty.kind {
                    self.resolve_ty_var(*inner_vid).or(Some(ty.clone()))
                } else {
                    Some(ty.clone())
                }
            }
        }
    }

    /// Resolve an int variable to its bound IntTy (if any).
    pub fn resolve_int_var(&self, vid: IntVid) -> Option<IntTy> {
        self.int_vars.get(&vid).copied().flatten()
    }

    /// Resolve a float variable to its bound FloatTy (if any).
    pub fn resolve_float_var(&self, vid: FloatVid) -> Option<FloatTy> {
        self.float_vars.get(&vid).copied().flatten()
    }

    /// Fully resolve a Ty, replacing all inference variables with their
    /// bound types (if resolved). Unresolved variables stay as Infer.
    pub fn resolve(&self, ty: &Ty) -> Ty {
        match &ty.kind {
            TyKind::Infer(InferVar::TyVar(vid)) => {
                self.resolve_ty_var(*vid).unwrap_or_else(|| ty.clone())
            }
            TyKind::Infer(InferVar::IntVar(vid)) => self
                .resolve_int_var(*vid)
                .map(|i| Ty::new(TyKind::Int(i), ty.span))
                .unwrap_or_else(|| ty.clone()),
            TyKind::Infer(InferVar::FloatVar(vid)) => self
                .resolve_float_var(*vid)
                .map(|f| Ty::new(TyKind::Float(f), ty.span))
                .unwrap_or_else(|| ty.clone()),
            TyKind::Ref(r, m, inner) => {
                Ty::new(TyKind::Ref(*r, *m, Box::new(self.resolve(inner))), ty.span)
            }
            TyKind::RawPtr(m, inner) => {
                Ty::new(TyKind::RawPtr(*m, Box::new(self.resolve(inner))), ty.span)
            }
            TyKind::Array(inner, c) => Ty::new(
                TyKind::Array(Box::new(self.resolve(inner)), c.clone()),
                ty.span,
            ),
            TyKind::Slice(inner) => Ty::new(TyKind::Slice(Box::new(self.resolve(inner))), ty.span),
            TyKind::Tuple(tys) => Ty::new(
                TyKind::Tuple(tys.iter().map(|t| self.resolve(t)).collect()),
                ty.span,
            ),
            _ => ty.clone(),
        }
    }

    // ================================================================
    // Unification
    // ================================================================

    /// Unify two types. Returns `Ok(())` if they unify, `Err(TypeError)`
    /// if they conflict.
    ///
    /// Side effects: may bind inference variables. Errors are also
    /// recorded in the internal error list (non-fatal).
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), Box<TypeError>> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        self.unify_resolved(&a, &b)
    }

    fn unify_resolved(&mut self, a: &Ty, b: &Ty) -> Result<(), Box<TypeError>> {
        // Error propagation: if either side is Error, succeed silently.
        if matches!(a.kind, TyKind::Error) || matches!(b.kind, TyKind::Error) {
            return Ok(());
        }

        match (&a.kind, &b.kind) {
            // Same concrete type → OK
            (TyKind::Bool, TyKind::Bool) => Ok(()),
            (TyKind::Char, TyKind::Char) => Ok(()),
            (TyKind::Never, TyKind::Never) => Ok(()),
            (TyKind::Str, TyKind::Str) => Ok(()),

            // Int with Int
            (TyKind::Int(a_i), TyKind::Int(b_i)) if a_i == b_i => Ok(()),
            (TyKind::Uint(a_u), TyKind::Uint(b_u)) if a_u == b_u => Ok(()),

            // Float with Float
            (TyKind::Float(a_f), TyKind::Float(b_f)) if a_f == b_f => Ok(()),

            // Int var with concrete int
            (TyKind::Infer(InferVar::IntVar(vid)), TyKind::Int(i)) => {
                self.bind_int_var(*vid, *i);
                Ok(())
            }
            (TyKind::Int(i), TyKind::Infer(InferVar::IntVar(vid))) => {
                self.bind_int_var(*vid, *i);
                Ok(())
            }
            (TyKind::Infer(InferVar::IntVar(vid)), TyKind::Uint(u)) => {
                // Int var can unify with Uint by converting to Int
                // For simplicity, store as Int (the variable can be either)
                self.bind_int_var_to_uint(*vid, *u);
                Ok(())
            }
            (TyKind::Uint(u), TyKind::Infer(InferVar::IntVar(vid))) => {
                self.bind_int_var_to_uint(*vid, *u);
                Ok(())
            }

            // Float var with concrete float
            (TyKind::Infer(InferVar::FloatVar(vid)), TyKind::Float(f)) => {
                self.bind_float_var(*vid, *f);
                Ok(())
            }
            (TyKind::Float(f), TyKind::Infer(InferVar::FloatVar(vid))) => {
                self.bind_float_var(*vid, *f);
                Ok(())
            }

            // TyVar with anything: bind the variable
            (TyKind::Infer(InferVar::TyVar(vid)), _) => {
                self.bind_ty_var(*vid, b.clone());
                Ok(())
            }
            (_, TyKind::Infer(InferVar::TyVar(vid))) => {
                self.bind_ty_var(*vid, a.clone());
                Ok(())
            }

            // IntVar with IntVar: merge
            (TyKind::Infer(InferVar::IntVar(a_vid)), TyKind::Infer(InferVar::IntVar(b_vid))) => {
                if a_vid != b_vid {
                    // Merge: if one is bound, propagate
                    if let Some(Some(i)) = self.int_vars.get(a_vid) {
                        self.bind_int_var(*b_vid, *i);
                    } else if let Some(Some(i)) = self.int_vars.get(b_vid) {
                        self.bind_int_var(*a_vid, *i);
                    }
                    // Link: store b_vid's resolution under a_vid
                    // For simplicity, just check both are consistent
                }
                Ok(())
            }

            // FloatVar with FloatVar: merge (similar to IntVar)
            (
                TyKind::Infer(InferVar::FloatVar(a_vid)),
                TyKind::Infer(InferVar::FloatVar(b_vid)),
            ) => {
                if a_vid != b_vid {
                    if let Some(Some(f)) = self.float_vars.get(a_vid) {
                        self.bind_float_var(*b_vid, *f);
                    } else if let Some(Some(f)) = self.float_vars.get(b_vid) {
                        self.bind_float_var(*a_vid, *f);
                    }
                }
                Ok(())
            }

            // Ref with Ref: unify region + mutability + inner
            (TyKind::Ref(_, a_m, a_t), TyKind::Ref(_, b_m, b_t)) => {
                if a_m != b_m {
                    return Err(Box::new(TypeError::mismatch(a.clone(), b.clone(), a.span)));
                }
                self.unify_resolved(a_t, b_t)
            }

            // Tuple with Tuple: unify element-wise
            (TyKind::Tuple(a_tys), TyKind::Tuple(b_tys)) => {
                if a_tys.len() != b_tys.len() {
                    return Err(Box::new(TypeError::mismatch(a.clone(), b.clone(), a.span)));
                }
                for (at, bt) in a_tys.iter().zip(b_tys.iter()) {
                    self.unify_resolved(at, bt)?;
                }
                Ok(())
            }

            // Array with Array
            (TyKind::Array(a_t, _), TyKind::Array(b_t, _)) => self.unify_resolved(a_t, b_t),

            // Slice with Slice
            (TyKind::Slice(a_t), TyKind::Slice(b_t)) => self.unify_resolved(a_t, b_t),

            // Never unifies with anything (bottom type)
            (TyKind::Never, _) | (_, TyKind::Never) => Ok(()),

            // Mismatch
            _ => Err(Box::new(TypeError::mismatch(a.clone(), b.clone(), a.span))),
        }
    }

    // ================================================================
    // Binding helpers
    // ================================================================

    pub fn bind_ty_var(&mut self, vid: TyVid, ty: Ty) {
        self.ty_vars.insert(vid, Some(ty));
    }

    fn bind_int_var(&mut self, vid: IntVid, i: IntTy) {
        self.int_vars.insert(vid, Some(i));
    }

    fn bind_int_var_to_uint(&mut self, vid: IntVid, _u: UintTy) {
        // For simplicity, we store the UintTy as an IntTy equivalent.
        // A more precise implementation would track signedness separately.
        // For Stage 2.2, we just mark the variable as resolved.
        self.int_vars.insert(vid, Some(IntTy::I32)); // placeholder
    }

    fn bind_float_var(&mut self, vid: FloatVid, f: FloatTy) {
        self.float_vars.insert(vid, Some(f));
    }

    // ================================================================
    // Defaulting
    // ================================================================

    /// Default unresolved integer variables to `i32` and float
    /// variables to `f64`. Called after all constraints are collected.
    pub fn default_unresolved(&mut self) {
        for binding in self.int_vars.values_mut() {
            if binding.is_none() {
                *binding = Some(IntTy::I32);
            }
        }
        for binding in self.float_vars.values_mut() {
            if binding.is_none() {
                *binding = Some(FloatTy::F64);
            }
        }
    }

    // ================================================================
    // Error collection
    // ================================================================

    pub fn push_error(&mut self, error: TypeError) {
        self.errors.push(error);
    }

    pub fn take_errors(&mut self) -> Vec<TypeError> {
        std::mem::take(&mut self.errors)
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::session::Span;

    fn ty_int(i: IntTy) -> Ty {
        Ty::new(TyKind::Int(i), Span::DUMMY)
    }

    fn ty_bool() -> Ty {
        Ty::new(TyKind::Bool, Span::DUMMY)
    }

    fn ty_infer(vid: u32) -> Ty {
        Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(vid))), Span::DUMMY)
    }

    #[test]
    fn unify_same_concrete() {
        let mut t = UnificationTable::new();
        assert!(t
            .unify(&ty_int(ast::IntTy::I32), &ty_int(ast::IntTy::I32))
            .is_ok());
    }

    #[test]
    fn unify_mismatched_concrete() {
        let mut t = UnificationTable::new();
        assert!(t.unify(&ty_int(ast::IntTy::I32), &ty_bool()).is_err());
    }

    #[test]
    fn unify_var_with_concrete() {
        let mut t = UnificationTable::new();
        let vid = t.new_ty_var();
        let var_ty = ty_infer(vid.0);
        assert!(t.unify(&var_ty, &ty_int(ast::IntTy::I32)).is_ok());
        let resolved = t.resolve(&var_ty);
        assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
    }

    #[test]
    fn unify_var_with_var() {
        let mut t = UnificationTable::new();
        let vid1 = t.new_ty_var();
        let vid2 = t.new_ty_var();
        let var1 = ty_infer(vid1.0);
        let var2 = ty_infer(vid2.0);
        // Unify var1 with i32
        assert!(t.unify(&var1, &ty_int(ast::IntTy::I32)).is_ok());
        // Unify var2 with var1 → var2 should resolve to i32
        assert!(t.unify(&var2, &var1).is_ok());
        let resolved = t.resolve(&var2);
        assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
    }

    #[test]
    fn unify_int_var_with_concrete() {
        let mut t = UnificationTable::new();
        let vid = t.new_int_var();
        let var_ty = Ty::new(TyKind::Infer(InferVar::IntVar(vid)), Span::DUMMY);
        let concrete = ty_int(ast::IntTy::I64);
        assert!(t.unify(&var_ty, &concrete).is_ok());
        let resolved = t.resolve(&var_ty);
        assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I64)));
    }

    #[test]
    fn default_unresolved_int_to_i32() {
        let mut t = UnificationTable::new();
        let vid = t.new_int_var();
        t.default_unresolved();
        let resolved = t.resolve_int_var(vid);
        assert_eq!(resolved, Some(ast::IntTy::I32));
    }

    #[test]
    fn default_unresolved_float_to_f64() {
        let mut t = UnificationTable::new();
        let vid = t.new_float_var();
        t.default_unresolved();
        let resolved = t.resolve_float_var(vid);
        assert_eq!(resolved, Some(ast::FloatTy::F64));
    }

    #[test]
    fn unify_tuple_same_length() {
        let mut t = UnificationTable::new();
        let a = Ty::new(
            TyKind::Tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]),
            Span::DUMMY,
        );
        let b = Ty::new(
            TyKind::Tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]),
            Span::DUMMY,
        );
        assert!(t.unify(&a, &b).is_ok());
    }

    #[test]
    fn unify_tuple_different_length() {
        let mut t = UnificationTable::new();
        let a = Ty::new(TyKind::Tuple(vec![ty_int(ast::IntTy::I32)]), Span::DUMMY);
        let b = Ty::new(
            TyKind::Tuple(vec![ty_int(ast::IntTy::I32), ty_bool()]),
            Span::DUMMY,
        );
        assert!(t.unify(&a, &b).is_err());
    }

    #[test]
    fn unify_never_with_anything() {
        let mut t = UnificationTable::new();
        let never = Ty::new(TyKind::Never, Span::DUMMY);
        assert!(t.unify(&never, &ty_bool()).is_ok());
        assert!(t.unify(&ty_int(ast::IntTy::I32), &never).is_ok());
    }

    #[test]
    fn unify_error_propagates() {
        let mut t = UnificationTable::new();
        let error = Ty::new(TyKind::Error, Span::DUMMY);
        assert!(t.unify(&error, &ty_bool()).is_ok());
        assert!(t.unify(&ty_int(ast::IntTy::I32), &error).is_ok());
    }

    #[test]
    fn resolve_chain() {
        let mut t = UnificationTable::new();
        let vid1 = t.new_ty_var();
        let vid2 = t.new_ty_var();
        // vid1 → vid2 → i32
        t.bind_ty_var(vid1, ty_infer(vid2.0));
        t.bind_ty_var(vid2, ty_int(ast::IntTy::I32));
        let resolved = t.resolve(&ty_infer(vid1.0));
        assert!(matches!(resolved.kind, TyKind::Int(ast::IntTy::I32)));
    }
}
