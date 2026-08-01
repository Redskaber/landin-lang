//! MIR type system: `Ty` + `TyKind` + inference variables.
//!
//! Per 03-type-system.md, MIR types are the "resolved" types — after
//! type inference (Stage 2.2), all inference variables are unified
//! to concrete types. During MIR construction (Stage 2.1), inference
//! variables may be present as placeholders.

use crate::ast::{FloatTy, IntTy, UintTy};
use crate::hir::DefId;
use std::rc::Rc;

#[cfg(test)]
use crate::session::Span;

/// A MIR type.
///
/// Stage 15.5 (v0.2): Removed `span: Span` field from `Ty`.
/// Stage 15.23 (v0.2): Added `kind()` method as preparation for Rc<TyKind> interning.
/// Stage 15.25 (v0.2): Added `Eq, Hash` derives to Ty (transitively from TyKind).
/// Stage 15.28 (v0.2): Ty::new and Ty::from_kind now go through a thread-local
///   TypeInterner for automatic dedup. Equal TyKind values return the same Ty
///   (by value), reducing memory usage. The interner is opt-in — callers that
///   don't want dedup can use `Ty::from_kind_raw` (no interning).
///
/// Per `docs/lang-design/19-ty-interning.md`: the thread-local interner is the
/// v0.2 approach. v0.3 will replace it with arena interning (`&'tcx TyKind`).
///
/// Per §1.0 原則 6 "通用 > 特例": one thread-local interner handles all TyKind variants.
/// Per §15 "最优 > 最小": this is the root-cause fix for type duplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ty {
    pub kind: TyKind,
}

// Stage 15.28: Thread-local TypeInterner for automatic Ty dedup.
//
// This is a global (per-thread) interner that deduplicates all Ty values
// created via `Ty::new` and `Ty::from_kind`. The interner is:
// - Thread-safe (each thread has its own interner)
// - Bounded (grows with unique types, not total type constructions)
// - Transparent (callers don't need to pass an interner around)
//
// Per §1.0 原則 3 "显式 > 隐式": the interner is explicit in the thread_local! macro.
thread_local! {
    static TYPE_INTERNER: std::cell::RefCell<crate::mir::ty_interner::TypeInterner> =
        std::cell::RefCell::new(crate::mir::ty_interner::TypeInterner::new());
}

impl Ty {
    /// Create a new Ty. Span is no longer stored on Ty (Stage 15.5).
    /// The `_span` parameter is kept for API compatibility — callers
    /// should migrate to `Ty::from_kind()` which doesn't take span.
    ///
    /// Stage 15.28: Now goes through the thread-local TypeInterner for dedup.
    pub fn new(kind: TyKind, _span: crate::session::Span) -> Self {
        Self::from_kind(kind)
    }

    /// Stage 15.5: Construct a Ty without span (preferred new API).
    ///
    /// Stage 15.28: Now goes through the thread-local TypeInterner for dedup.
    /// Equal TyKind values return the same Ty (by value), reducing memory.
    pub fn from_kind(kind: TyKind) -> Self {
        TYPE_INTERNER.with(|interner| interner.borrow_mut().intern(kind))
    }

    /// Stage 15.28: Construct a Ty WITHOUT going through the interner.
    ///
    /// This is for cases where the caller knows the TyKind is unique (e.g.,
    /// inference variables that are always different) or where interning
    /// overhead is unnecessary.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the `_raw` suffix makes it explicit
    /// that this bypasses the interner.
    pub fn from_kind_raw(kind: TyKind) -> Self {
        Self { kind }
    }

    /// Stage 15.23: Accessor method for TyKind.
    pub fn kind(&self) -> &TyKind {
        &self.kind
    }

    /// Stage 15.28: Get the number of unique types in the thread-local interner.
    /// Useful for debugging and statistics.
    pub fn interner_len() -> usize {
        TYPE_INTERNER.with(|interner| interner.borrow().len())
    }

    /// Stage 15.28: Clear the thread-local interner.
    /// Called between compilations to avoid cross-compilation pollution.
    pub fn clear_interner() {
        TYPE_INTERNER
            .with(|interner| *interner.borrow_mut() = crate::mir::ty_interner::TypeInterner::new());
    }
}

/// All MIR type kinds.
/// Stage 15.25 (v0.2): Added `Eq, Hash` derives — enabled by ConstVal::Float
/// now storing u64 (bits) instead of f64. Required for future Ty interning
/// (HashMap<TyKind, ...> dedup in TypeInterner).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TyKind {
    Bool,
    Char,
    Int(IntTy),
    Uint(UintTy),
    Float(FloatTy),
    Str,
    Never,
    /// `&'r mut? T`
    Ref(Region, Mutability, Box<Ty>),
    /// `*mut T` / `*const T`
    RawPtr(Mutability, Box<Ty>),
    /// `[T; N]`
    Array(Box<Ty>, Box<Const>),
    /// `[T]`
    Slice(Box<Ty>),
    /// `(T1, T2, ...)`
    Tuple(Vec<Ty>),
    /// Function definition type: `fn(T1, T2) -> T3`
    FnDef(DefId, SubstsRef),
    /// Function pointer type: `fn(T1, T2) -> T3`
    FnPtr(Sig),
    /// Closure type
    Closure(DefId, SubstsRef),
    /// Algebraic data type (struct/enum): `Foo<T1, T2>`
    Adt(DefId, SubstsRef),
    /// Foreign type (`extern { type Foo; }`)
    Foreign,
    /// Type parameter: `T` in a generic context
    Param(ParamTy),
    /// Inference variable (placeholder during typeck)
    Infer(InferVar),
    /// Type error (after failed unification)
    Error,
}

/// Mutability of a reference or pointer.
/// Stage 15.25 (v0.2): Added `Eq, Hash` derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutability {
    Mutable,
    Immutable,
}

/// A region (lifetime) in MIR. During MIR construction (Stage 2.1),
/// all regions are `Region::Var` (inference variables). Stage 2.3
/// borrow check will infer concrete regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// `'static`
    Static,
    /// Inference variable (resolved by Stage 2.3)
    Var(RegionVid),
    /// Erased (for codegen)
    Erased,
}

/// Region variable ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionVid(pub u32);

/// Substitutions: the type arguments to a generic type.
/// E.g., `HashMap<String, i32>` has SubstsRef = [String, i32].
///
/// Stage 15.10 (v0.2): Changed from `Vec<Ty>` to `Rc<[Ty]>` — interned
/// slice. This makes `SubstsRef::clone()` a refcount bump (8 bytes) instead
/// of a heap allocation (24 bytes + N × 40 bytes for the Ty values).
/// For a crate with 50 generic applications, this eliminates 50 heap
/// allocations per compilation.
///
/// Per `docs/lang-design/19-ty-interning.md`: this is the stepping stone
/// toward full Ty interning. The Rc<[Ty]> form allows sharing the same
/// substs slice across multiple types (e.g., `Vec<i32>` used in 100 places
/// shares one `Rc<[i32]>` slice after interning).
///
/// Per §1.0 原则 6 "通用 > 特例": one shared slice type for all generic apps.
/// Per §15 "最优 > 最小": root-cause fix for per-app heap allocation.
///
/// Construction: `Vec<Ty>` → `Rc<[Ty]>` via `.into()` or `Rc::from()`.
/// Consumption: `Rc<[Ty]>` derefs to `[Ty]`, so `.iter()`, `.get()`,
/// `.len()`, `.is_empty()` all work unchanged.
/// Mutation: `Rc<[Ty]>` is immutable — use `Rc::make_mut` (requires
/// `Vec<Ty>` conversion) or rebuild the Vec and convert back.
pub type SubstsRef = Rc<[Ty]>;

/// Function signature.
/// Stage 15.25 (v0.2): Added `Eq, Hash` derives.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sig {
    pub inputs: Vec<Ty>,
    pub output: Box<Ty>,
    pub abi: crate::ast::Abi,
    pub is_unsafe: bool,
}

/// A type parameter: `T` with its index in the generic params.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamTy {
    pub index: u32,
    pub name: crate::lexer::Symbol,
}

/// A compile-time constant value used in types (e.g., array length `[T; N]`).
///
/// Stage 15.11 (v0.2): Changed `ty: Box<Ty>` to `ty: Ty` — eliminates a
/// heap allocation per Const. Ty is already a small struct (one TyKind
/// field), so Box<Ty> was an unnecessary indirection.
/// Per §15 "最优 > 最小": root-cause fix for per-Const heap allocation.
///
/// Stage 15.25 (v0.2): Added `Eq, Hash` derives — enabled by ConstVal::Float
/// now storing u64 (bits) instead of f64.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Const {
    pub ty: Ty,
    pub val: ConstVal,
}

/// Compile-time constant value.
///
/// Stage 15.25 (v0.2): Changed `Float(f64)` to `Float(u64)` — stores the
/// bit representation of f64 instead of the f64 value itself. This enables
/// `Eq + Hash` derives on `ConstVal` (and transitively on `Const`, `TyKind`,
/// and `Ty`), which is required for future Ty interning (HashMap dedup).
///
/// Per §1.0 原則 3 "显式 > 隐式": the bit representation is explicit.
/// Per §15 "最优 > 最小": this is the root-cause fix that unblocks Eq+Hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstVal {
    Int(u128),
    Uint(u128),
    /// Stage 15.25: f64 stored as bits (u64) for Eq+Hash support.
    /// Use `f64::from_bits(val)` to recover the f64 value.
    Float(u64),
    Bool(bool),
    Char(char),
    Str(crate::lexer::Symbol),
    /// Unevaluated (needs const evaluation, Stage 3+)
    Unevaluated,
}

/// Inference variable for type inference (Stage 2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InferVar {
    /// General type variable: `?T`
    TyVar(TyVid),
    /// Integer variable: `?i` (could be i32, u64, etc.)
    IntVar(IntVid),
    /// Float variable: `?f` (could be f32 or f64)
    FloatVar(FloatVid),
}

/// Type variable ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TyVid(pub u32);

/// Integer variable ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntVid(pub u32);

/// Float variable ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FloatVid(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ty_primitive_construction() {
        let ty = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
        assert!(matches!(ty.kind, TyKind::Int(IntTy::I32)));
    }

    #[test]
    fn ty_ref_construction() {
        let inner = Ty::new(TyKind::Bool, Span::DUMMY);
        let ty = Ty::new(
            TyKind::Ref(Region::Static, Mutability::Immutable, Box::new(inner)),
            Span::DUMMY,
        );
        match ty.kind {
            TyKind::Ref(Region::Static, Mutability::Immutable, inner) => {
                assert!(matches!(inner.kind, TyKind::Bool));
            }
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn ty_tuple_construction() {
        let ty = Ty::new(
            TyKind::Tuple(vec![
                Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Bool, Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        match ty.kind {
            TyKind::Tuple(tys) => assert_eq!(tys.len(), 2),
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn ty_infer_var() {
        let ty = Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY);
        assert!(matches!(ty.kind, TyKind::Infer(InferVar::TyVar(TyVid(0)))));
    }

    #[test]
    fn region_var() {
        let r = Region::Var(RegionVid(5));
        assert_eq!(r, Region::Var(RegionVid(5)));
        assert_ne!(r, Region::Static);
    }
}
