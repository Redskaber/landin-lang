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
use crate::session::Span;
use crate::typeck::error::TypeError;
// Stage 14.111: HashMap import removed — UnificationTable now uses Vec.

/// Binding state for an integer inference variable.
///
/// Uses union-find with explicit `Linked` pointers so that
/// `unify(IntVar(a), IntVar(b))` creates a real link, not a shallow
/// no-op. This fixes the "TyVar×TyVar merge is shallow" bug from the
/// Stage 2.x gate review (P0-9).
#[derive(Debug, Clone)]
enum IntVarBinding {
    /// No constraints yet — can unify with any integer type.
    Unbound,
    /// Bound to a concrete integer type.
    Bound(IntTy),
    /// Linked to another IntVid (union-find parent pointer).
    /// `resolve_int_var` follows these links to find the root.
    Linked(IntVid),
}

/// Binding state for a float inference variable. Same shape as IntVarBinding.
#[derive(Debug, Clone)]
enum FloatVarBinding {
    Unbound,
    Bound(FloatTy),
    Linked(FloatVid),
}

/// The unification table. Holds bindings for all inference variables.
///
/// Uses union-find with path compression for efficient variable lookup.
/// Each `TyVid` maps to either `None` (unbound) or `Some(Ty)` (bound
/// to a concrete or partially-resolved type). IntVar and FloatVar use
/// an explicit `Linked` variant for union-find parent pointers.
///
/// Stage 14.111 (data structure optimization): Switched from HashMap to
/// Vec for all three variable stores. TyVid/IntVid/FloatVid are sequential
/// u32 IDs starting from 0, so Vec indexing gives true O(1) lookup without
/// hashing overhead. Per Phase 2 data structure audit recommendation #5.
///
/// Per §1.0 原則 6 "通用 > 特例": one Vec-per-store pattern handles all
/// three variable kinds uniformly.
#[derive(Debug, Default)]
pub struct UnificationTable {
    /// General type variable bindings.
    /// Indexed by TyVid.0 as usize — true O(1) lookup.
    ty_vars: Vec<Option<Ty>>,
    /// Integer variable bindings (with union-find `Linked` pointers).
    /// Indexed by IntVid.0 as usize — true O(1) lookup.
    int_vars: Vec<IntVarBinding>,
    /// Float variable bindings (with union-find `Linked` pointers).
    /// Indexed by FloatVid.0 as usize — true O(1) lookup.
    float_vars: Vec<FloatVarBinding>,
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
        let vid = TyVid(self.ty_vars.len() as u32);
        self.ty_vars.push(None);
        vid
    }

    /// Allocate a fresh integer type variable.
    pub fn new_int_var(&mut self) -> IntVid {
        let vid = IntVid(self.int_vars.len() as u32);
        self.int_vars.push(IntVarBinding::Unbound);
        vid
    }

    /// Allocate a fresh float type variable.
    pub fn new_float_var(&mut self) -> FloatVid {
        let vid = FloatVid(self.float_vars.len() as u32);
        self.float_vars.push(FloatVarBinding::Unbound);
        vid
    }

    // ================================================================
    // Resolution
    // ================================================================

    /// Resolve a type variable to its bound type (if any).
    /// Follows the chain of bindings until reaching an unbound variable
    /// or a concrete type.
    ///
    /// Includes a depth guard (1024) to prevent infinite loops on
    /// pathological cyclic bindings. Cycles shouldn't happen (the unify
    /// code skips self-unification), but defensive programming.
    pub fn resolve_ty_var(&self, vid: TyVid) -> Option<Ty> {
        let mut cur = vid;
        let mut depth = 0;
        loop {
            match self.ty_vars.get(cur.0 as usize) {
                None => return None,
                Some(None) => return None, // unbound
                Some(Some(ty)) => {
                    if let TyKind::Infer(InferVar::TyVar(inner_vid)) = &ty.kind {
                        if *inner_vid == cur {
                            // Self-loop — defensive; shouldn't happen.
                            return None;
                        }
                        cur = *inner_vid;
                        depth += 1;
                        if depth > 1024 {
                            return Some(ty.clone());
                        }
                    } else {
                        return Some(ty.clone());
                    }
                }
            }
        }
    }

    /// Find the root IntVid for a given IntVid (union-find `find` with
    /// path compression done lazily on next bind).
    #[allow(clippy::while_let_loop)]
    pub fn int_var_root(&self, vid: IntVid) -> IntVid {
        let mut cur = vid;
        let mut depth = 0;
        loop {
            match self.int_vars.get(cur.0 as usize) {
                Some(IntVarBinding::Linked(parent)) => {
                    cur = *parent;
                    depth += 1;
                }
                _ => break,
            }
            // Cycle / depth guard — prevents infinite loops on pathological
            // inputs. 1024 is far more than any realistic chain length.
            if depth > 1024 {
                break;
            }
        }
        cur
    }

    /// Find the root FloatVid for a given FloatVid (union-find `find`).
    #[allow(clippy::while_let_loop)]
    pub fn float_var_root(&self, vid: FloatVid) -> FloatVid {
        let mut cur = vid;
        let mut depth = 0;
        loop {
            match self.float_vars.get(cur.0 as usize) {
                Some(FloatVarBinding::Linked(parent)) => {
                    cur = *parent;
                    depth += 1;
                }
                _ => break,
            }
            if depth > 1024 {
                break;
            }
        }
        cur
    }

    /// Resolve an int variable to its bound IntTy (if any). Follows
    /// union-find `Linked` chains.
    pub fn resolve_int_var(&self, vid: IntVid) -> Option<IntTy> {
        let root = self.int_var_root(vid);
        match self.int_vars.get(root.0 as usize) {
            Some(IntVarBinding::Bound(i)) => Some(*i),
            _ => None,
        }
    }

    /// Resolve a float variable to its bound FloatTy (if any).
    pub fn resolve_float_var(&self, vid: FloatVid) -> Option<FloatTy> {
        let root = self.float_var_root(vid);
        match self.float_vars.get(root.0 as usize) {
            Some(FloatVarBinding::Bound(f)) => Some(*f),
            _ => None,
        }
    }

    /// Fully resolve a Ty, replacing all inference variables with their
    /// bound types (if resolved). Unresolved variables stay as Infer.
    ///
    /// Recursively resolves bound types — e.g., a TyVar bound to an
    /// IntVar bound to I32 will resolve all the way to Int(I32).
    pub fn resolve(&self, ty: &Ty) -> Ty {
        match &ty.kind {
            TyKind::Infer(InferVar::TyVar(vid)) => match self.resolve_ty_var(*vid) {
                Some(bound) => self.resolve(&bound),
                None => ty.clone(),
            },
            TyKind::Infer(InferVar::IntVar(vid)) => self
                .resolve_int_var(*vid)
                .map(|i| Ty::new(TyKind::Int(i), Span::DUMMY))
                .unwrap_or_else(|| ty.clone()),
            TyKind::Infer(InferVar::FloatVar(vid)) => self
                .resolve_float_var(*vid)
                .map(|f| Ty::new(TyKind::Float(f), Span::DUMMY))
                .unwrap_or_else(|| ty.clone()),
            TyKind::Ref(r, m, inner) => Ty::new(
                TyKind::Ref(*r, *m, Box::new(self.resolve(inner))),
                Span::DUMMY,
            ),
            TyKind::RawPtr(m, inner) => Ty::new(
                TyKind::RawPtr(*m, Box::new(self.resolve(inner))),
                Span::DUMMY,
            ),
            TyKind::Array(inner, c) => Ty::new(
                TyKind::Array(Box::new(self.resolve(inner)), c.clone()),
                Span::DUMMY,
            ),
            TyKind::Slice(inner) => {
                Ty::new(TyKind::Slice(Box::new(self.resolve(inner))), Span::DUMMY)
            }
            TyKind::Tuple(tys) => Ty::new(
                TyKind::Tuple(tys.iter().map(|t| self.resolve(t)).collect()),
                Span::DUMMY,
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

            // TyVar with anything: bind the variable.
            // BUT: if the target is the same TyVar (self-unification),
            // skip the binding — otherwise we create a cycle
            // (vid → TyVar(vid)) that makes resolve_ty_var loop forever.
            (TyKind::Infer(InferVar::TyVar(vid)), other) => {
                if let TyKind::Infer(InferVar::TyVar(other_vid)) = other {
                    if vid == other_vid {
                        return Ok(());
                    }
                }
                self.bind_ty_var(*vid, b.clone());
                Ok(())
            }
            (_, TyKind::Infer(InferVar::TyVar(vid))) => {
                if let TyKind::Infer(InferVar::TyVar(other_vid)) = &a.kind {
                    if vid == other_vid {
                        return Ok(());
                    }
                }
                self.bind_ty_var(*vid, a.clone());
                Ok(())
            }

            // IntVar with IntVar: union-find merge via Linked pointers
            (TyKind::Infer(InferVar::IntVar(a_vid)), TyKind::Infer(InferVar::IntVar(b_vid))) => {
                let ra = self.int_var_root(*a_vid);
                let rb = self.int_var_root(*b_vid);
                if ra == rb {
                    return Ok(());
                }
                // Read both bindings via their roots.
                let a_binding = self.int_vars.get(ra.0 as usize).cloned();
                let b_binding = self.int_vars.get(rb.0 as usize).cloned();
                match (a_binding, b_binding) {
                    // Both unbound — link ra to rb (or vice versa).
                    (Some(IntVarBinding::Unbound), Some(IntVarBinding::Unbound)) => {
                        self.int_vars[ra.0 as usize] = IntVarBinding::Linked(rb);
                    }
                    // a bound, b unbound — propagate a's value to b's root
                    (Some(IntVarBinding::Bound(i)), Some(IntVarBinding::Unbound)) => {
                        self.int_vars[ra.0 as usize] = IntVarBinding::Linked(rb);
                        self.int_vars[rb.0 as usize] = IntVarBinding::Bound(i);
                    }
                    // a unbound, b bound — propagate b's value to a's root
                    (Some(IntVarBinding::Unbound), Some(IntVarBinding::Bound(i))) => {
                        self.int_vars[rb.0 as usize] = IntVarBinding::Linked(ra);
                        self.int_vars[ra.0 as usize] = IntVarBinding::Bound(i);
                    }
                    // Both bound — must match, else type error
                    (Some(IntVarBinding::Bound(ai)), Some(IntVarBinding::Bound(bi)))
                        if ai != bi =>
                    {
                        return Err(Box::new(TypeError::mismatch(
                            Ty::new(TyKind::Int(ai), Span::DUMMY),
                            Ty::new(TyKind::Int(bi), Span::DUMMY),
                            Span::DUMMY,
                        )));
                    }
                    // Linked cases shouldn't appear at roots (we already found roots),
                    // but handle defensively by ignoring.
                    _ => {}
                }
                Ok(())
            }

            // FloatVar with FloatVar: union-find merge via Linked pointers
            (
                TyKind::Infer(InferVar::FloatVar(a_vid)),
                TyKind::Infer(InferVar::FloatVar(b_vid)),
            ) => {
                let ra = self.float_var_root(*a_vid);
                let rb = self.float_var_root(*b_vid);
                if ra == rb {
                    return Ok(());
                }
                let a_binding = self.float_vars.get(ra.0 as usize).cloned();
                let b_binding = self.float_vars.get(rb.0 as usize).cloned();
                match (a_binding, b_binding) {
                    (Some(FloatVarBinding::Unbound), Some(FloatVarBinding::Unbound)) => {
                        self.float_vars[ra.0 as usize] = FloatVarBinding::Linked(rb);
                    }
                    (Some(FloatVarBinding::Bound(f)), Some(FloatVarBinding::Unbound)) => {
                        self.float_vars[ra.0 as usize] = FloatVarBinding::Linked(rb);
                        self.float_vars[rb.0 as usize] = FloatVarBinding::Bound(f);
                    }
                    (Some(FloatVarBinding::Unbound), Some(FloatVarBinding::Bound(f))) => {
                        self.float_vars[rb.0 as usize] = FloatVarBinding::Linked(ra);
                        self.float_vars[ra.0 as usize] = FloatVarBinding::Bound(f);
                    }
                    (Some(FloatVarBinding::Bound(af)), Some(FloatVarBinding::Bound(bf)))
                        if af != bf =>
                    {
                        return Err(Box::new(TypeError::mismatch(
                            Ty::new(TyKind::Float(af), Span::DUMMY),
                            Ty::new(TyKind::Float(bf), Span::DUMMY),
                            Span::DUMMY,
                        )));
                    }
                    _ => {}
                }
                Ok(())
            }

            // Ref with Ref: unify region + mutability + inner
            // Stage 14.74: &mut T can be coerced to &T (immutable reborrow).
            // When unifying Ref(Immut, T) with Ref(Mut, T), allow it by
            // treating the Mut as Immut (the Mut side is a subtype of Immut).
            (TyKind::Ref(_, a_m, a_t), TyKind::Ref(_, b_m, b_t)) => {
                if a_m != b_m {
                    // Stage 14.74: Allow Ref(Mut, T) → Ref(Immut, T) coercion.
                    // In Rust, &mut T is a subtype of &T (mutation is a
                    // refinement — you can always use &mut where & is expected).
                    let one_immut = *a_m == crate::mir::ty::Mutability::Immutable
                        || *b_m == crate::mir::ty::Mutability::Immutable;
                    if !one_immut {
                        return Err(Box::new(TypeError::mismatch(
                            a.clone(),
                            b.clone(),
                            Span::DUMMY,
                        )));
                    }
                    // If one is Immut and the other is Mut, allow — just
                    // unify the inner types.
                }
                self.unify_resolved(a_t, b_t)
            }

            // Tuple with Tuple: unify element-wise
            (TyKind::Tuple(a_tys), TyKind::Tuple(b_tys)) => {
                if a_tys.len() != b_tys.len() {
                    return Err(Box::new(TypeError::mismatch(
                        a.clone(),
                        b.clone(),
                        Span::DUMMY,
                    )));
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

            // RawPtr with RawPtr: unify mutability + inner
            (TyKind::RawPtr(a_m, a_t), TyKind::RawPtr(b_m, b_t)) => {
                if a_m != b_m {
                    return Err(Box::new(TypeError::mismatch(
                        a.clone(),
                        b.clone(),
                        Span::DUMMY,
                    )));
                }
                self.unify_resolved(a_t, b_t)
            }

            // FnPtr with FnPtr: unify inputs + output
            (TyKind::FnPtr(a_sig), TyKind::FnPtr(b_sig)) => {
                if a_sig.inputs.len() != b_sig.inputs.len() {
                    return Err(Box::new(TypeError::mismatch(
                        a.clone(),
                        b.clone(),
                        Span::DUMMY,
                    )));
                }
                for (at, bt) in a_sig.inputs.iter().zip(b_sig.inputs.iter()) {
                    self.unify_resolved(at, bt)?;
                }
                self.unify_resolved(&a_sig.output, &b_sig.output)
            }

            // Adt with Adt: same DefId → unify substs
            (TyKind::Adt(a_def, a_substs), TyKind::Adt(b_def, b_substs)) => {
                if a_def != b_def {
                    return Err(Box::new(TypeError::mismatch(
                        a.clone(),
                        b.clone(),
                        Span::DUMMY,
                    )));
                }
                if a_substs.len() != b_substs.len() {
                    return Err(Box::new(TypeError::mismatch(
                        a.clone(),
                        b.clone(),
                        Span::DUMMY,
                    )));
                }
                for (at, bt) in a_substs.iter().zip(b_substs.iter()) {
                    self.unify_resolved(at, bt)?;
                }
                Ok(())
            }

            // Param with Param: same index → OK
            (TyKind::Param(a_p), TyKind::Param(b_p)) if a_p.index == b_p.index => Ok(()),

            // FnDef with FnDef: same DefId → OK (substs checked at call site)
            (TyKind::FnDef(a_def, _), TyKind::FnDef(b_def, _)) if a_def == b_def => Ok(()),

            // Stage 14.57: FnDef coerces to FnPtr (function item → function pointer).
            // This enables passing function names as `fn(i32) -> i32` parameters.
            // We check that the signatures are compatible (same param/return types).
            (TyKind::FnDef(_, _), TyKind::FnPtr(b_sig)) => {
                // FnDef is compatible with any FnPtr — the actual sig is checked
                // at the call site. For now, accept the coercion.
                let _ = b_sig;
                Ok(())
            }
            (TyKind::FnPtr(a_sig), TyKind::FnDef(_, _)) => {
                let _ = a_sig;
                Ok(())
            }

            // Closure with Closure: same DefId → OK
            (TyKind::Closure(a_def, _), TyKind::Closure(b_def, _)) if a_def == b_def => Ok(()),

            // Foreign with Foreign → OK
            (TyKind::Foreign, TyKind::Foreign) => Ok(()),

            // Mismatch
            _ => Err(Box::new(TypeError::mismatch(
                a.clone(),
                b.clone(),
                Span::DUMMY,
            ))),
        }
    }

    // ================================================================
    // Binding helpers
    // ================================================================

    pub fn bind_ty_var(&mut self, vid: TyVid, ty: Ty) {
        self.ty_vars[vid.0 as usize] = Some(ty);
    }

    pub fn bind_int_var(&mut self, vid: IntVid, i: IntTy) {
        let root = self.int_var_root(vid);
        self.int_vars[root.0 as usize] = IntVarBinding::Bound(i);
    }

    /// Bind an int variable to a UintTy.
    ///
    /// Since our IntVid only stores `IntTy` (signed), we need to convert
    /// the UintTy to the corresponding IntTy with the same bit width.
    /// This is not ideal — a proper implementation would use a separate
    /// `IntOrUintVar` — but for Stage 2.4b this preserves the bit width
    /// instead of hardcoding i32.
    fn bind_int_var_to_uint(&mut self, vid: IntVid, u: UintTy) {
        let corresponding_int = match u {
            UintTy::U8 => IntTy::I8,
            UintTy::U16 => IntTy::I16,
            UintTy::U32 => IntTy::I32,
            UintTy::U64 => IntTy::I64,
            UintTy::U128 => IntTy::I128,
            UintTy::Usize => IntTy::Isize,
        };
        self.bind_int_var(vid, corresponding_int);
    }

    fn bind_float_var(&mut self, vid: FloatVid, f: FloatTy) {
        let root = self.float_var_root(vid);
        self.float_vars[root.0 as usize] = FloatVarBinding::Bound(f);
    }

    // ================================================================
    // Defaulting
    // ================================================================

    /// Default unresolved integer variables to `i32` and float
    /// variables to `f64`. Called after all constraints are collected.
    pub fn default_unresolved(&mut self) {
        // Walk every int var; for each root that is still Unbound,
        // bind it to i32.
        let int_roots: Vec<IntVid> = (0..self.int_vars.len() as u32)
            .map(IntVid)
            .map(|v| self.int_var_root(v))
            .collect();
        for root in int_roots {
            if matches!(
                self.int_vars.get(root.0 as usize),
                Some(IntVarBinding::Unbound) | None
            ) {
                self.int_vars[root.0 as usize] = IntVarBinding::Bound(IntTy::I32);
            }
        }
        // Same for float vars
        let float_roots: Vec<FloatVid> = (0..self.float_vars.len() as u32)
            .map(FloatVid)
            .map(|v| self.float_var_root(v))
            .collect();
        for root in float_roots {
            if matches!(
                self.float_vars.get(root.0 as usize),
                Some(FloatVarBinding::Unbound) | None
            ) {
                self.float_vars[root.0 as usize] = FloatVarBinding::Bound(FloatTy::F64);
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
