//! HIR node definitions.
//!
//! Per 06-mir.md §3, HIR is the "high-level IR" — the AST plus per-node
//! `HirId`s and resolution placeholders. It is ~70% structurally isomorphic
//! to the AST (crate::ast), with these differences:
//!
//! - Every node carries a `hir_id: HirId` for cross-referencing with
//!   typeck tables, borrow info, etc.
//! - `HirPath` carries a `res: Res` field populated by Stage 1.3 name
//!   resolution (`Res::Unknown` until then).
//! - `HirTy` carries an `inferred: Option<InferTy>` field populated by
//!   Stage 2 type inference (`None` until then).
//! - `Body` is split out from owners so that name resolution and typeck
//!   can iterate owners first, then descend into bodies.
//!
//! Stage 1.1 only DEFINES these structures; Stage 1.2 implements AST→HIR
//! lowering; Stage 1.3 populates `Res`; Stage 2 populates `InferTy`.

use crate::ast::{
    Abi, Attr, BindingMode, GenericArgs, Ident, Lifetime, MacroDelim, Mutability, PathLeading,
    RangeEnd, SelfKind, Visibility,
};
use crate::lexer::Symbol;
use crate::session::Span;

// Re-export the ID types for convenience.
pub use crate::hir::id::{DefId, HirId, ItemLocalId, OwnerId};

// Stage 3.63 (cross-stage naming standardization): `DefKind` is now
// defined here in `hir::kinds` (its architectural home — it's consumed
// by `Res::Def(DefId, DefKind)` which is a HIR type). The former
// definition in `resolve::module_tree` has been removed; `resolve::*`
// now imports `DefKind` from here. This aligns the dependency direction:
// `resolve` depends on `hir`, not vice versa.

/// The kind of a definition. Used for namespace disambiguation during
/// path resolution (e.g., `Foo` could be a struct type or a struct
/// constructor function — the DefKind tells us which).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Fn,
    Const,
    Static,
    Struct,
    Enum,
    Trait,
    Impl,
    TypeAlias,
    Mod,
    Use,
    ExternFn,
    ExternStatic,
    ExternType,
}

impl DefKind {
    /// Returns `true` if this definition lives in the value namespace
    /// (fn, const, static, extern fn, extern static).
    pub fn is_value(self) -> bool {
        matches!(
            self,
            DefKind::Fn
                | DefKind::Const
                | DefKind::Static
                | DefKind::ExternFn
                | DefKind::ExternStatic
        )
    }

    /// Returns `true` if this definition lives in the type namespace
    /// (struct, enum, trait, type alias, mod, extern type).
    pub fn is_type(self) -> bool {
        matches!(
            self,
            DefKind::Struct
                | DefKind::Enum
                | DefKind::Trait
                | DefKind::TypeAlias
                | DefKind::Mod
                | DefKind::ExternType
        )
    }
}

// =====================================================================
// HIR Crate — top-level container
// =====================================================================

/// The HIR crate: top-level container for all lowered items and bodies.
///
/// Per 06-mir.md §3, the HIR crate is a flat map of `DefId -> OwnerNode`
/// plus a separate map of `BodyId -> Body`. This separation allows
/// name resolution (Stage 1.3) and type inference (Stage 2) to iterate
/// owners first, then lazily descend into bodies.
#[derive(Debug, Clone, Default)]
pub struct HirCrate {
    /// All owner nodes, keyed by DefId. Insertion order is preserved.
    pub owners: Vec<(DefId, OwnerNode)>,
    /// All bodies, keyed by BodyId. A body is the expression tree of
    /// a fn/const/static.
    pub bodies: Vec<(BodyId, Body)>,
    /// Stage 14.110 (perf): Cached index for O(1) owner lookup by DefId.
    /// Maps DefId.0 → index into `owners` Vec. Built lazily on first lookup.
    /// Per Phase 2 data structure audit: eliminates O(n²) linear scans.
    owner_index: std::cell::OnceCell<std::collections::HashMap<u32, usize>>,
    /// Stage 14.110 (perf): Cached index for O(1) body lookup by BodyId.
    body_index: std::cell::OnceCell<std::collections::HashMap<u32, usize>>,
}

impl HirCrate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up an owner node by DefId.
    /// Stage 14.110: O(1) via cached index (was O(n) linear scan).
    pub fn owner(&self, def_id: DefId) -> Option<&OwnerNode> {
        let index = self.owner_index.get_or_init(|| {
            self.owners
                .iter()
                .enumerate()
                .map(|(i, (d, _))| (d.0, i))
                .collect()
        });
        index.get(&def_id.0).map(|&i| &self.owners[i].1)
    }

    /// Look up a body by BodyId.
    /// Stage 14.110: O(1) via cached index (was O(n) linear scan).
    pub fn body(&self, body_id: BodyId) -> Option<&Body> {
        let index = self.body_index.get_or_init(|| {
            self.bodies
                .iter()
                .enumerate()
                .map(|(i, (b, _))| (b.owner.0 .0, i))
                .collect()
        });
        index.get(&body_id.owner.0 .0).map(|&i| &self.bodies[i].1)
    }

    /// Total number of owner nodes.
    pub fn owner_count(&self) -> usize {
        self.owners.len()
    }

    /// Total number of bodies.
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }
}

// =====================================================================
// Owner nodes (top-level items)
// =====================================================================

/// A top-level owner node: an item, foreign item, trait item, or impl item.
#[derive(Debug, Clone)]
pub enum OwnerNode {
    Item(HirItem),
    ForeignItem(HirForeignItem),
    TraitItem(HirTraitItem),
    ImplItem(HirImplItem),
}

/// HIR form of a top-level item (fn, const, static, struct, enum, trait,
/// impl, type alias, extern block, mod, use).
#[derive(Debug, Clone)]
pub enum HirItem {
    Fn(HirFn),
    Const(HirConst),
    Static(HirStatic),
    Struct(HirStruct),
    Enum(HirEnum),
    Trait(HirTrait),
    Impl(HirImpl),
    TypeAlias(HirTypeAlias),
    ExternBlock(HirExternBlock),
    Mod(HirMod),
    Use(HirUse),
}

/// Items inside `extern "C" { ... }` blocks.
#[derive(Debug, Clone)]
pub enum HirForeignItem {
    Fn(HirFn),
    Static(HirStatic),
    TypeAlias(HirTypeAlias),
}

/// Items inside `trait Foo { ... }` blocks.
#[derive(Debug, Clone)]
pub enum HirTraitItem {
    Fn(HirFn),
    Type(HirAssocType),
    Const(HirAssocConst),
}

/// Items inside `impl Foo for Bar { ... }` blocks.
#[derive(Debug, Clone)]
pub enum HirImplItem {
    Fn(HirFn),
    Const(HirConst),
    Type(HirAssocType),
}

// =====================================================================
// Item kinds
// =====================================================================

/// A function declaration. The `body` is `Some(BodyId)` if a body is present
/// (i.e., not a trait method signature without default).
#[derive(Debug, Clone)]
pub struct HirFn {
    pub hir_id: HirId,
    pub ident: Ident,
    pub generics: HirGenerics,
    pub sig: HirFnSig,
    pub body: Option<BodyId>,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirFnSig {
    pub inputs: Vec<HirParam>,
    pub output: HirFnRetTy,
    pub abi: Abi,
    pub is_unsafe: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub hir_id: HirId,
    pub pat: HirPat,
    pub ty: Option<HirTy>, // None for `self` shorthand
    pub self_kind: Option<SelfKind>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirFnRetTy {
    Default(Span),
    Ty(HirTy),
}

#[derive(Debug, Clone)]
pub struct HirConst {
    pub hir_id: HirId,
    pub ident: Ident,
    pub ty: HirTy,
    pub body: BodyId,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirStatic {
    pub hir_id: HirId,
    pub ident: Ident,
    pub ty: HirTy,
    pub mutability: Mutability,
    pub body: BodyId,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirStruct {
    pub hir_id: HirId,
    pub ident: Ident,
    pub generics: HirGenerics,
    pub fields: Vec<HirFieldDef>,
    pub is_unit: bool,
    pub is_tuple: bool,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirFieldDef {
    pub hir_id: HirId,
    pub vis: Visibility,
    pub ident: Option<Ident>,
    pub ty: HirTy,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirEnum {
    pub hir_id: HirId,
    pub ident: Ident,
    pub generics: HirGenerics,
    pub variants: Vec<HirVariant>,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirVariant {
    pub hir_id: HirId,
    pub ident: Ident,
    pub data: HirVariantData,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirVariantData {
    Unit(Span),
    Tuple(Vec<HirFieldDef>, Span),
    Struct(Vec<HirFieldDef>, Span),
}

#[derive(Debug, Clone)]
pub struct HirTrait {
    pub hir_id: HirId,
    pub ident: Ident,
    pub generics: HirGenerics,
    pub supertraits: Vec<HirTypeBound>,
    pub items: Vec<HirTraitItem>,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    /// Stage 3.65: `unsafe trait Foo { ... }` — propagated from AST `TraitDecl.is_unsafe`.
    pub is_unsafe: bool,
    pub span: Span,
}

/// Associated type item in a trait: `type Item: Bound = Default;`
#[derive(Debug, Clone)]
pub struct HirAssocType {
    pub hir_id: HirId,
    pub ident: Ident,
    pub bounds: Vec<HirTypeBound>,
    pub default: Option<HirTy>,
    pub span: Span,
}

/// Associated const item in a trait: `const X: i32 = 42;`
#[derive(Debug, Clone)]
pub struct HirAssocConst {
    pub hir_id: HirId,
    pub ident: Ident,
    pub ty: HirTy,
    pub default: Option<BodyId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirImpl {
    pub hir_id: HirId,
    pub generics: HirGenerics,
    pub of_trait: Option<HirPath>,
    pub self_ty: HirTy,
    pub items: Vec<HirImplItem>,
    pub attrs: Vec<Attr>,
    /// Stage 3.65: `unsafe impl Trait for T { ... }` — propagated from AST `ImplDecl.is_unsafe`.
    pub is_unsafe: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirTypeAlias {
    pub hir_id: HirId,
    pub ident: Ident,
    pub generics: HirGenerics,
    pub ty: HirTy,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirExternBlock {
    pub hir_id: HirId,
    pub abi: Abi,
    pub items: Vec<HirForeignItem>,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirMod {
    pub hir_id: HirId,
    pub ident: Ident,
    pub kind: HirModKind,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirModKind {
    /// `mod foo { items }`
    Inline(Vec<HirItem>),
    /// `mod foo;` — loaded from external file
    Loaded,
}

#[derive(Debug, Clone)]
pub struct HirUse {
    pub hir_id: HirId,
    pub tree: HirUseTree,
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirUseTree {
    Path {
        prefix: HirPath,
        children: Vec<HirUseTree>,
    },
    Leaf(HirPath, Option<Ident>),
    Glob(HirPath),
}

// =====================================================================
// Bodies
// =====================================================================

/// Reference to a body stored in the HIR map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId {
    pub owner: OwnerId,
}

/// A body is the expression/statement tree of a fn/const/static.
/// Stored separately from the owner so that name resolution and type
/// inference can iterate owners first, then descend into bodies.
#[derive(Debug, Clone)]
pub struct Body {
    pub hir_id: HirId,
    pub params: Vec<HirParam>,
    pub value: HirExpr, // Block for fn; Expr for const/static
    pub span: Span,
}

// =====================================================================
// Generics
// =====================================================================

#[derive(Debug, Clone, Default)]
pub struct HirGenerics {
    pub params: Vec<HirGenericParam>,
    pub where_clause: Vec<HirWherePredicate>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirGenericParam {
    Lifetime(HirLifetimeParam),
    Type(HirTypeParam),
}

#[derive(Debug, Clone)]
pub struct HirLifetimeParam {
    pub hir_id: HirId,
    pub ident: Ident,
    pub bounds: Vec<Lifetime>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirTypeParam {
    pub hir_id: HirId,
    pub ident: Ident,
    pub bounds: Vec<HirTypeBound>,
    pub default: Option<HirTy>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirTypeBound {
    Trait(HirTraitBound),
    Lifetime(Lifetime),
}

#[derive(Debug, Clone)]
pub struct HirTraitBound {
    pub hir_id: HirId,
    pub path: HirPath,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirWherePredicate {
    pub hir_id: HirId,
    pub lifetime: Option<Lifetime>,
    pub bounded_ty: HirTy,
    pub bounds: Vec<HirTypeBound>,
    pub span: Span,
}

// =====================================================================
// Types
// =====================================================================

/// HIR form of `QSelf` — the `<Type as Trait>::` qualifier on a path.
///
/// Unlike the AST's `QSelf` (which holds an AST `Ty`), `HirQSelf` holds a
/// `HirTy` so that the inner type carries its own `HirId` and `InferTy`
/// slot. This preserves the HIR invariant that every type-bearing node
/// is a `HirTy`.
#[derive(Debug, Clone, Default)]
pub struct HirQSelf {
    /// The inner type `T` in `<T as Trait>::Name`.
    pub ty: Option<Box<HirTy>>,
    /// The position in `Path.segments` where the trait path begins.
    /// `0` means the entire path is the trait; non-zero means there are
    /// segments before the trait (rare, but rustc supports it).
    pub position: usize,
}

#[derive(Debug, Clone)]
pub struct HirTy {
    pub hir_id: HirId,
    pub kind: HirTyKind,
    pub inferred: Option<InferTy>, // None until Stage 2 typeck
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirTyKind {
    Bool,
    Char,
    Int(crate::ast::IntTy),
    Uint(crate::ast::UintTy),
    Float(crate::ast::FloatTy),
    Never,
    Tuple(Vec<HirTy>),
    Array(Box<HirTy>, Box<HirExpr>),
    Slice(Box<HirTy>),
    Ref(Option<Lifetime>, Mutability, Box<HirTy>),
    Ptr(Mutability, Box<HirTy>),
    FnPtr {
        inputs: Vec<HirTy>,
        output: Box<HirTy>,
        abi: Abi,
        is_unsafe: bool,
    },
    Path(HirQSelf, HirPath),
    TraitObject {
        bounds: Vec<HirTypeBound>,
        lifetime: Option<Lifetime>,
    },
    ImplTrait(Vec<HirTypeBound>),
    Infer,
}

// =====================================================================
// Paths (with resolution placeholder)
// =====================================================================

#[derive(Debug, Clone)]
pub struct HirPath {
    pub hir_id: HirId,
    pub segments: Vec<HirPathSegment>,
    pub leading: PathLeading,
    pub res: Res, // Res::Unknown until Stage 1.3 name resolution
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirPathSegment {
    pub ident: Ident,
    pub args: Option<GenericArgs>,
}

/// Name resolution result. Populated by Stage 1.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Res {
    /// Not yet resolved.
    Unknown,
    /// A local variable binding.
    Local(HirId),
    /// A top-level definition (fn/struct/enum/trait/etc.).
    ///
    /// Stage 3.30 (v3.10 process): now carries `DefKind` so downstream
    /// passes (MIR lower, typeck, codegen) can distinguish fn calls from
    /// struct ctors from enum variant ctors without re-querying HIR.
    /// This is the optimal fix per §15 — eliminates the root cause of
    /// "tuple struct ctor `Pair(1,2)` was being lowered as `Call`".
    Def(DefId, DefKind),
    /// A primitive type (i32, bool, etc.).
    PrimTy(PrimTy),
    /// The `Self` type of the current trait or impl.
    ///
    /// Stage 3.65: now carries `HirSelfKind` to distinguish:
    /// - `Trait` — `Self` inside a trait declaration refers to the trait's
    ///   Self type (which is the implementor's type).
    /// - `Impl` — `Self` inside an impl block refers to the impl's self_ty.
    ///
    /// This distinction matters for type-checking (e.g., whether `Self` can
    /// be assumed to satisfy the trait's supertraits).
    SelfTy(HirSelfKind),
    /// The `self` value of the current method.
    SelfCtor,
    /// A lifetime.
    Lifetime,
    /// An error recovery — name resolution failed.
    Err,
}

/// Stage 3.65: Discriminant for `Res::SelfTy` — distinguishes trait-Self
/// from impl-Self.
///
/// This matters for type-checking:
/// - In a trait declaration, `Self` is abstract — it can be any type that
///   implements the trait. The trait's supertraits are *bounds* on `Self`,
///   not facts.
/// - In an impl block, `Self` is concrete — it's the impl's `self_ty`.
///   The trait's supertraits are *facts* (proven by the impl).
///
/// Named `HirSelfKind` (not `SelfKind`) to avoid collision with the
/// pre-existing `ast::SelfKind` enum (which discriminates `self`/`&self`/
/// `&mut self`/`self: Self` method receivers — a different concept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirSelfKind {
    /// `Self` inside a trait declaration — abstract, satisfies supertrait bounds.
    Trait,
    /// `Self` inside an impl block — concrete, equals `impl self_ty`.
    Impl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimTy {
    Bool,
    Char,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
    Str,
}

// =====================================================================
// Patterns
// =====================================================================

#[derive(Debug, Clone)]
pub struct HirPat {
    pub hir_id: HirId,
    pub kind: HirPatKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirPatKind {
    Wild,
    Ident(BindingMode, Ident, Option<Box<HirPat>>),
    Struct(HirPath, Vec<HirPatField>, bool /* has_rest */),
    TupleStruct(HirPath, Vec<HirPat>),
    Tuple(Vec<HirPat>),
    Slice(Vec<HirPat>, Option<Box<HirPat>>),
    Or(Vec<HirPat>),
    Path(HirPath),
    Lit(Box<HirExpr>),
    Range(Option<Box<HirExpr>>, Option<Box<HirExpr>>, RangeEnd),
    Ref(Box<HirPat>, Mutability),
    Rest,
}

#[derive(Debug, Clone)]
pub struct HirPatField {
    pub hir_id: HirId,
    pub ident: Ident,
    pub pat: HirPat,
    pub is_shorthand: bool,
    pub span: Span,
}

// =====================================================================
// Expressions
// =====================================================================

#[derive(Debug, Clone)]
pub struct HirExpr {
    pub hir_id: HirId,
    pub kind: HirExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    Lit(HirLitKind),
    Path(HirPath),
    Block(HirBlock),
    Call {
        func: Box<HirExpr>,
        args: Vec<HirExpr>,
    },
    MethodCall {
        receiver: Box<HirExpr>,
        method: Ident,
        args: Vec<HirExpr>,
        generic_args: Option<GenericArgs>,
    },
    Field {
        receiver: Box<HirExpr>,
        ident: Ident,
    },
    Index {
        receiver: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    Unary {
        op: HirUnaryOp,
        expr: Box<HirExpr>,
    },
    Binary {
        op: HirBinOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },
    Assign {
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
        op: Option<HirBinOp>,
    },
    AddrOf {
        mutability: Mutability,
        expr: Box<HirExpr>,
    },
    Cast {
        expr: Box<HirExpr>,
        ty: HirTy,
    },
    Try {
        expr: Box<HirExpr>,
    },
    If {
        cond: Box<HirExpr>,
        then: HirBlock,
        else_: Option<Box<HirExpr>>,
    },
    Match {
        expr: Box<HirExpr>,
        arms: Vec<HirArm>,
    },
    Loop {
        body: HirBlock,
    },
    While {
        cond: Box<HirExpr>,
        body: HirBlock,
    },
    For {
        pat: HirPat,
        iter: Box<HirExpr>,
        body: HirBlock,
    },
    Closure {
        is_move: bool,
        params: Vec<HirParam>,
        body: Box<HirExpr>,
    },
    Return {
        expr: Option<Box<HirExpr>>,
    },
    Break {
        expr: Option<Box<HirExpr>>,
    },
    Continue,
    Range {
        start: Option<Box<HirExpr>>,
        end: Option<Box<HirExpr>>,
        end_kind: RangeEnd,
    },
    Tuple {
        elems: Vec<HirExpr>,
    },
    Array {
        elems: Vec<HirExpr>,
    },
    Repeat {
        elem: Box<HirExpr>,
        count: Box<HirExpr>,
    },
    Struct {
        path: HirPath,
        fields: Vec<HirExprField>,
    },
    MacroCall {
        path: HirPath,
        delim: MacroDelim,
    },
    /// Stage 13.12 + Stage 13.16: `println!(fmt, args...)` / `print!` / `eprintln!` / `eprint!`
    /// Carries the format string AND arguments through HIR to MIR lowerer for
    /// printf emission with the correct format specifiers.
    ///
    /// Stage 13.12: introduced with `msg: String` only.
    /// Stage 13.16: extended with `args: Vec<HirExpr>` to support format args.
    Println {
        msg: String,
        args: Vec<HirExpr>,
        newline: bool,
        stderr: bool,
    },
    Unsafe(HirBlock),
    Unit,
    /// Stage 8.5: `await expr` — async await expression.
    Await {
        expr: Box<HirExpr>,
    },
    /// Stage 8.5: `async { block }` — async block.
    Async {
        block: HirBlock,
    },
}

#[derive(Debug, Clone)]
pub struct HirArm {
    pub hir_id: HirId,
    pub pat: HirPat,
    pub guard: Option<HirExpr>,
    pub body: Box<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirExprField {
    pub hir_id: HirId,
    pub ident: Ident,
    pub expr: Option<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    Neg,
    Not,
    Deref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
pub enum HirLitKind {
    Bool(bool),
    Int(u128, Option<crate::ast::IntTy>),
    Uint(u128, Option<crate::ast::UintTy>),
    Float(f64, Option<crate::ast::FloatTy>),
    Char(char),
    Str(Symbol),
    ByteStr(Symbol),
    Byte(u8),
}

// =====================================================================
// Blocks and statements
// =====================================================================

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub hir_id: HirId,
    pub stmts: Vec<HirStmt>,
    pub expr: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    Local(Box<HirLocal>),
    // Boxed because HirExpr is large (many variants); keeping it inline would
    // bloat every HirStmt instance to the size of HirExpr.
    Expr(Box<HirExpr>, bool /* has_semicolon */),
    Semi,
    Empty(Span),
}

#[derive(Debug, Clone)]
pub struct HirLocal {
    pub hir_id: HirId,
    pub pat: HirPat,
    pub ty: Option<HirTy>,
    pub init: Option<HirExpr>,
    pub span: Span,
}

// =====================================================================
// Type inference placeholder
// =====================================================================

/// Placeholder for a type that will be inferred by Stage 2 typeck.
///
/// During HIR construction we create fresh `InferTy` vars; typeck will
/// unify them with concrete types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InferTy(pub u32);

/// A counter for generating fresh InferTy vars. Per-crate.
#[derive(Debug, Default)]
pub struct InferTyCounter {
    next: u32,
}

impl InferTyCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fresh(&mut self) -> InferTy {
        let v = InferTy(self.next);
        self.next += 1;
        v
    }

    pub fn count(&self) -> usize {
        self.next as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_ty_freshness() {
        let mut c = InferTyCounter::new();
        let a = c.fresh();
        let b = c.fresh();
        assert_ne!(a, b);
        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
        assert_eq!(c.count(), 2);
    }

    #[test]
    fn res_default_is_unknown() {
        let r = Res::Unknown;
        assert_eq!(r, Res::Unknown);
    }

    #[test]
    fn hir_id_in_struct() {
        let h = HirId::new(DefId(1), ItemLocalId(2));
        let ty = HirTy {
            hir_id: h,
            kind: HirTyKind::Bool,
            inferred: None,
            span: Span::DUMMY,
        };
        assert_eq!(ty.hir_id, h);
        assert!(matches!(ty.kind, HirTyKind::Bool));
        assert!(ty.inferred.is_none());
    }

    #[test]
    fn hir_path_default_res_unknown() {
        let p = HirPath {
            hir_id: HirId::new(DefId(0), ItemLocalId(1)),
            segments: vec![],
            leading: PathLeading::None,
            res: Res::Unknown,
            span: Span::DUMMY,
        };
        assert_eq!(p.res, Res::Unknown);
    }
}
