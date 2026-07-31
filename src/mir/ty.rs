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
/// Span belongs on `LocalDecl` and `Statement`, not on the type itself.
/// This is the foundational change for Ty interning — without Span, Ty
/// values can be cached and compared by kind alone.
///
/// Per `docs/lang-design/19-ty-interning.md`: this enables O(1) Ty::clone()
/// in v0.3 (via Rc/arena interning) and correct caching of method return types.
///
/// Per §1.0 原則 3 "显式 > 隐式": Span is now explicit on LocalDecl/Statement,
/// not implicitly duplicated on every Ty.
#[derive(Debug, Clone, PartialEq)]
pub struct Ty {
    pub kind: TyKind,
}

impl Ty {
    /// Create a new Ty. Span is no longer stored on Ty (Stage 15.5).
    /// The `_span` parameter is kept for API compatibility — callers
    /// should migrate to `Ty::from_kind()` which doesn't take span.
    pub fn new(kind: TyKind, _span: crate::session::Span) -> Self {
        Self { kind }
    }

    /// Stage 15.5: Construct a Ty without span (preferred new API).
    pub fn from_kind(kind: TyKind) -> Self {
        Self { kind }
    }
}

/// All MIR type kinds.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct Sig {
    pub inputs: Vec<Ty>,
    pub output: Box<Ty>,
    pub abi: crate::ast::Abi,
    pub is_unsafe: bool,
}

/// A type parameter: `T` with its index in the generic params.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct Const {
    pub ty: Ty,
    pub val: ConstVal,
}

/// Compile-time constant value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstVal {
    Int(u128),
    Uint(u128),
    Float(f64),
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
