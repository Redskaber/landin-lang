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

/// Stage 15.64: Conservative Copy-ness check for MIR lowering.
///
/// Returns `true` for types that are ALWAYS Copy (primitives, references,
/// function types, `Never`, `Infer`, `Error`, `Foreign`). Returns `false`
/// for types that MAY or MAY NOT be Copy (`Adt`, `Str`, `Slice`, `Closure`,
/// `Param`).
///
/// This is the **conservative** check — it's used during MIR lowering
/// (where `TraitResolver` is not available) to decide between
/// `Operand::Copy` and `Operand::Move`. For `Adt` types, we conservatively
/// use `Move` (treating the type as non-Copy). This is sound: a false
/// negative (using Move for a Copy type) just means an unnecessary move
/// (the source local is marked as moved and can't be used again); a false
/// positive (using Copy for a non-Copy type) would be unsound (double-drop).
///
/// For the **precise** Copy check (using `TraitResolver` to query
/// `impl Copy`), use `borrowck::copy_semantics::ty_is_copy_with_resolver`.
/// The precise check runs during borrow checking (after MIR lowering).
///
/// ## Recursion
///
/// `Tuple` and `Array` are recursively checked — a tuple is Copy iff all
/// elements are Copy (conservatively). This matches Rust's `#[derive(Copy)]`
/// semantics.
///
/// Per §23 rule 5 (DRY): single source of truth for conservative Copy
/// detection in MIR lowering. Replaces inline checks in
/// `mir::lower::control_flow` (let bindings) and `mir::lower::expr_operand`
/// (struct literals, closure captures).
/// Per §1.0 原則 5 "报错 > 静默": conservative (false negative) is preferred
/// over unsound (false positive).
/// Per §16: this function reads `Ty` only (no HIR, no resolver).
pub fn is_mir_ty_copy_conservative(ty: &Ty) -> bool {
    use crate::mir::ty::TyKind::*;
    match &ty.kind {
        Bool | Char | Int(_) | Uint(_) | Float(_) => true,
        Ref(_, _, _) => true,
        RawPtr(_, _) => true,
        FnDef(_, _) | FnPtr(_) => true,
        Never => true,
        Tuple(tys) => tys.iter().all(is_mir_ty_copy_conservative),
        Array(inner, _) => is_mir_ty_copy_conservative(inner),
        // Infer, Error, Foreign, and Param: assume Copy to avoid spurious
        // errors during type inference (the concrete type isn't known yet).
        //
        // Stage 16.53 (Task 11 Phase 2): `Param` is now added to the "assume
        // Copy" list. A `Param(X)` represents a generic type parameter whose
        // concrete type is only known after monomorphization (Phase 4).
        // During MIR lowering + typeck + borrowck, we don't know whether `X`
        // is Copy, so we conservatively assume Copy to avoid spurious "use of
        // moved value" errors (e.g., `self.x.f()` where `f` takes `&self` and
        // `self.x: X`). The actual Copy-ness will be checked after
        // monomorphization when the concrete type is substituted in.
        //
        // Per §1.0 原則 5 "报错 > 静默": conservative (assume Copy) is preferred
        // over unsound (assume non-Copy) during inference, because false
        // positives (spurious move errors) are worse than false negatives
        // (missing move errors that will be caught post-mono).
        Infer(_) | Error | Foreign | Param(_) => true,
        // Adt, Str, Slice, Closure: conservatively non-Copy.
        // Use `ty_is_copy_with_resolver` for precise Adt Copy detection.
        Adt(_, _) | Str | Slice(_) | Closure(_, _) => false,
    }
}

/// Stage 15.80: Format a `TyKind` as a human-readable type string for
/// user-facing diagnostics.
///
/// This replaces the previous `{:?}` (Debug) formatting that leaked
/// internal enum variant names (e.g., `Int(I32)`, `Infer(IntVar(IntVid(0)))`)
/// into user-facing error messages. The Debug format is useful for
/// compiler developers but confusing for users.
///
/// Examples:
///   - `Int(I32)` → `"i32"`
///   - `Uint(U8)` → `"u8"`
///   - `Float(F64)` → `"f64"`
///   - `Bool` → `"bool"`
///   - `Str` → `"str"`
///   - `Never` → `"!"`
///   - `Ref(_, Immutable, T)` → `"&T"`
///   - `Ref(_, Mutable, T)` → `"&mut T"`
///   - `RawPtr(Immutable, T)` → `"*const T"`
///   - `RawPtr(Mutable, T)` → `"*mut T"`
///   - `Array(T, n)` → `"[T; n]"`
///   - `Slice(T)` → `"[T]"`
///   - `Tuple([A, B])` → `"(A, B)"`
///   - `Tuple([])` → `"()"`
///   - `Tuple([A])` → `"(A,)"`
///   - `FnDef(_, _)` → `"fn"` (def_id is not user-meaningful)
///   - `FnPtr(sig)` → `"fn(...) -> ..."` (sig formatted)
///   - `Closure(_, _)` → `"{closure}"`
///   - `Adt(_, _)` → `"<adt>"` (def_id is not user-meaningful without resolver)
///   - `Foreign` → `"<foreign type>"`
///   - `Param(p)` → `"<type param>"`
///   - `Infer(TyVar(_))` → `"_"`
///   - `Infer(IntVar(_))` → `"{integer}"`
///   - `Infer(FloatVar(_))` → `"{float}"`
///   - `Error` → `"<type error>"`
///
/// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
/// Per §23 (API Naming): `type_to_string` follows `<noun>_<verb>_<noun>`
/// pattern (matches Rust convention `ty::type_to_string`).
///
/// Note: this function does NOT resolve `Adt` DefIds to type names —
/// that requires resolver access (the `DefId` → name mapping lives in
/// `driver::fn_name_by_def_id`). For richer type display in diagnostics,
/// a future stage can add a `type_to_string_with_resolver` variant.
/// For now, the simple `<adt>` placeholder is sufficient to remove the
/// Debug format leak.
pub fn type_to_string(ty: &Ty) -> String {
    type_kind_to_string(&ty.kind)
}

/// Format a `TyKind` as a human-readable type string.
///
/// Stage 15.80: extracted from `type_to_string` so callers that have
/// a `TyKind` directly (e.g., `expected.kind`, `found.kind`) don't need
/// to wrap it in a `Ty` just to format it.
pub fn type_kind_to_string(kind: &TyKind) -> String {
    use std::fmt::Write;
    match kind {
        TyKind::Bool => "bool".to_string(),
        TyKind::Char => "char".to_string(),
        TyKind::Int(i) => int_ty_to_string(*i).to_string(),
        TyKind::Uint(u) => uint_ty_to_string(*u).to_string(),
        TyKind::Float(f) => float_ty_to_string(*f).to_string(),
        TyKind::Str => "str".to_string(),
        TyKind::Never => "!".to_string(),
        TyKind::Ref(_, mutability, inner) => {
            let prefix = match mutability {
                Mutability::Mutable => "&mut ",
                Mutability::Immutable => "&",
            };
            format!("{prefix}{}", type_to_string(inner))
        }
        TyKind::RawPtr(mutability, inner) => {
            let prefix = match mutability {
                Mutability::Mutable => "*mut ",
                Mutability::Immutable => "*const ",
            };
            format!("{prefix}{}", type_to_string(inner))
        }
        TyKind::Array(inner, count) => {
            // Format the array length if it's a concrete value.
            let len_str = match &count.val {
                ConstVal::Uint(n) => n.to_string(),
                ConstVal::Int(n) => n.to_string(),
                _ => "_".to_string(),
            };
            format!("[{}; {}]", type_to_string(inner), len_str)
        }
        TyKind::Slice(inner) => format!("[{}]", type_to_string(inner)),
        TyKind::Tuple(tys) => {
            if tys.is_empty() {
                "()".to_string()
            } else if tys.len() == 1 {
                format!("({},)", type_to_string(&tys[0]))
            } else {
                let mut s = String::from("(");
                for (i, t) in tys.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    let _ = write!(s, "{}", type_to_string(t));
                }
                s.push(')');
                s
            }
        }
        TyKind::FnDef(_, _) => "fn".to_string(),
        TyKind::FnPtr(sig) => fn_sig_to_string(sig),
        TyKind::Closure(_, _) => "{closure}".to_string(),
        TyKind::Adt(_, _) => "<adt>".to_string(),
        TyKind::Foreign => "<foreign type>".to_string(),
        TyKind::Param(_) => "<type param>".to_string(),
        TyKind::Infer(infer_var) => infer_var_to_string(infer_var).to_string(),
        TyKind::Error => "<type error>".to_string(),
    }
}

/// Format an `IntTy` as a lowercase string.
fn int_ty_to_string(i: IntTy) -> &'static str {
    match i {
        IntTy::I8 => "i8",
        IntTy::I16 => "i16",
        IntTy::I32 => "i32",
        IntTy::I64 => "i64",
        IntTy::I128 => "i128",
        IntTy::Isize => "isize",
    }
}

/// Format a `UintTy` as a lowercase string.
fn uint_ty_to_string(u: UintTy) -> &'static str {
    match u {
        UintTy::U8 => "u8",
        UintTy::U16 => "u16",
        UintTy::U32 => "u32",
        UintTy::U64 => "u64",
        UintTy::U128 => "u128",
        UintTy::Usize => "usize",
    }
}

/// Format a `FloatTy` as a lowercase string.
fn float_ty_to_string(f: FloatTy) -> &'static str {
    match f {
        FloatTy::F32 => "f32",
        FloatTy::F64 => "f64",
    }
}

/// Format an `InferVar` as a human-readable placeholder.
///
/// Matches Rust's convention: integer inference vars display as `{integer}`,
/// float inference vars as `{float}`, and general type vars as `_`.
fn infer_var_to_string(infer: &InferVar) -> &'static str {
    match infer {
        InferVar::TyVar(_) => "_",
        InferVar::IntVar(_) => "{integer}",
        InferVar::FloatVar(_) => "{float}",
    }
}

/// Format an `FnSig` (function pointer signature) as a string.
///
/// Stage 15.80: simple format `fn(args) -> ret`. For unit return, omits
/// the `-> ()`.
fn fn_sig_to_string(sig: &Sig) -> String {
    use std::fmt::Write;
    let mut s = String::from("fn(");
    for (i, input) in sig.inputs.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        let _ = write!(s, "{}", type_to_string(input));
    }
    s.push(')');
    // Omit `-> ()` for unit return (matches Rust convention).
    if !matches!(sig.output.kind, TyKind::Tuple(ref tys) if tys.is_empty()) {
        let _ = write!(s, " -> {}", type_to_string(&sig.output));
    }
    s
}

/// Stage 15.84: Format a `RegionVid` as a human-readable region string.
///
/// Matches Rust's convention: region variables display as `'r<N>` (e.g.,
/// `'r0`, `'r5`). This replaces the previous `{:?}` Debug formatting that
/// leaked `RegionVid(5)` into user-facing lifetime error messages.
///
/// Per §1.0 原則 3 "显式 > 隐式": user-facing region names are explicit.
/// Per §23 (API Naming): `region_vid_to_string` follows `<noun>_<verb>_<noun>`
/// pattern (matches `type_to_string`).
pub fn region_vid_to_string(vid: RegionVid) -> String {
    format!("'r{}", vid.0)
}

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

    // === Stage 15.80: type_to_string tests ===

    #[test]
    fn type_to_string_primitives() {
        assert_eq!(type_to_string(&Ty::new(TyKind::Bool, Span::DUMMY)), "bool");
        assert_eq!(type_to_string(&Ty::new(TyKind::Char, Span::DUMMY)), "char");
        assert_eq!(type_to_string(&Ty::new(TyKind::Str, Span::DUMMY)), "str");
        assert_eq!(type_to_string(&Ty::new(TyKind::Never, Span::DUMMY)), "!");
        assert_eq!(
            type_to_string(&Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY)),
            "i32"
        );
        assert_eq!(
            type_to_string(&Ty::new(TyKind::Int(IntTy::Isize), Span::DUMMY)),
            "isize"
        );
        assert_eq!(
            type_to_string(&Ty::new(TyKind::Uint(UintTy::U8), Span::DUMMY)),
            "u8"
        );
        assert_eq!(
            type_to_string(&Ty::new(TyKind::Uint(UintTy::Usize), Span::DUMMY)),
            "usize"
        );
        assert_eq!(
            type_to_string(&Ty::new(TyKind::Float(FloatTy::F32), Span::DUMMY)),
            "f32"
        );
        assert_eq!(
            type_to_string(&Ty::new(TyKind::Float(FloatTy::F64), Span::DUMMY)),
            "f64"
        );
    }

    #[test]
    fn type_to_string_references() {
        let inner = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
        let shared_ref = Ty::new(
            TyKind::Ref(Region::Static, Mutability::Immutable, Box::new(inner)),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&shared_ref), "&i32");

        let inner = Ty::new(TyKind::Bool, Span::DUMMY);
        let mut_ref = Ty::new(
            TyKind::Ref(Region::Erased, Mutability::Mutable, Box::new(inner)),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&mut_ref), "&mut bool");
    }

    #[test]
    fn type_to_string_raw_pointers() {
        let inner = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
        let const_ptr = Ty::new(
            TyKind::RawPtr(Mutability::Immutable, Box::new(inner)),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&const_ptr), "*const i32");

        let inner = Ty::new(TyKind::Bool, Span::DUMMY);
        let mut_ptr = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(inner)),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&mut_ptr), "*mut bool");
    }

    #[test]
    fn type_to_string_arrays() {
        let inner = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
        let count = Const {
            ty: Ty::new(TyKind::Uint(UintTy::Usize), Span::DUMMY),
            val: ConstVal::Uint(10),
        };
        let arr = Ty::new(TyKind::Array(Box::new(inner), Box::new(count)), Span::DUMMY);
        assert_eq!(type_to_string(&arr), "[i32; 10]");
    }

    #[test]
    fn type_to_string_tuples() {
        let unit = Ty::new(TyKind::Tuple(vec![]), Span::DUMMY);
        assert_eq!(type_to_string(&unit), "()");

        let single = Ty::new(
            TyKind::Tuple(vec![Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY)]),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&single), "(i32,)");

        let pair = Ty::new(
            TyKind::Tuple(vec![
                Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY),
                Ty::new(TyKind::Bool, Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&pair), "(i32, bool)");
    }

    #[test]
    fn type_to_string_inference_vars() {
        let ty_var = Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(0))), Span::DUMMY);
        assert_eq!(type_to_string(&ty_var), "_");

        let int_var = Ty::new(TyKind::Infer(InferVar::IntVar(IntVid(0))), Span::DUMMY);
        assert_eq!(type_to_string(&int_var), "{integer}");

        let float_var = Ty::new(TyKind::Infer(InferVar::FloatVar(FloatVid(0))), Span::DUMMY);
        assert_eq!(type_to_string(&float_var), "{float}");
    }

    #[test]
    fn type_to_string_special() {
        let err = Ty::new(TyKind::Error, Span::DUMMY);
        assert_eq!(type_to_string(&err), "<type error>");

        let foreign = Ty::new(TyKind::Foreign, Span::DUMMY);
        assert_eq!(type_to_string(&foreign), "<foreign type>");

        let closure = Ty::new(
            TyKind::Closure(DefId(0), Rc::from([] as [Ty; 0])),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&closure), "{closure}");

        let fn_def = Ty::new(
            TyKind::FnDef(DefId(0), Rc::from([] as [Ty; 0])),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&fn_def), "fn");
    }

    #[test]
    fn type_to_string_nested() {
        // &[i32; 3]
        let inner = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
        let count = Const {
            ty: Ty::new(TyKind::Uint(UintTy::Usize), Span::DUMMY),
            val: ConstVal::Uint(3),
        };
        let arr = Ty::new(TyKind::Array(Box::new(inner), Box::new(count)), Span::DUMMY);
        let ref_arr = Ty::new(
            TyKind::Ref(Region::Erased, Mutability::Immutable, Box::new(arr)),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&ref_arr), "&[i32; 3]");

        // (*mut bool, i32)
        let bool_ptr = Ty::new(
            TyKind::RawPtr(
                Mutability::Mutable,
                Box::new(Ty::new(TyKind::Bool, Span::DUMMY)),
            ),
            Span::DUMMY,
        );
        let tuple = Ty::new(
            TyKind::Tuple(vec![
                bool_ptr,
                Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY),
            ]),
            Span::DUMMY,
        );
        assert_eq!(type_to_string(&tuple), "(*mut bool, i32)");
    }

    // === Stage 15.84: region_vid_to_string tests ===

    #[test]
    fn region_vid_to_string_basic() {
        assert_eq!(region_vid_to_string(RegionVid(0)), "'r0");
        assert_eq!(region_vid_to_string(RegionVid(1)), "'r1");
        assert_eq!(region_vid_to_string(RegionVid(42)), "'r42");
    }
}
