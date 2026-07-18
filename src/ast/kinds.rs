//! AST node definitions.
//!
//! Based on 05-ast.md §2-§11.

use crate::lexer::token::Symbol;
use crate::session::Span;
/// A crate: top-level collection of items.
#[derive(Debug, Clone)]
pub struct Crate {
    pub items: Vec<Item>,
    pub attrs: Vec<Attr>,
}

/// An item with visibility, attributes, and span.
#[derive(Debug, Clone)]
pub struct Item {
    pub vis: Visibility,
    pub attrs: Vec<Attr>,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Fn(FnDecl),
    Const(ConstDecl),
    Static(StaticDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
    TypeAlias(TypeAliasDecl),
    ExternBlock(ExternBlock),
    Mod(ModDecl),
    Use(UseDecl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct Attr {
    pub path: Path,
    pub args: Option<AttrArgs>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum AttrArgs {
    Empty,
    Literal(LitKind),
    Eq(Expr),
    List(Vec<AttrArg>),
}

#[derive(Debug, Clone)]
pub struct AttrArg {
    pub name: Option<Ident>,
    pub value: Option<Expr>,
}

// --- Function ---

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub sig: FnSig,
    pub body: Option<Block>,
    pub generics: Generics,
}

#[derive(Debug, Clone)]
pub struct FnSig {
    pub inputs: Vec<Param>,
    pub output: FnRetTy,
    pub abi: Abi,
    pub is_unsafe: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    Landin,
    C,
    System,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub pat: Pat,
    pub ty: Ty,
    pub attrs: Vec<Attr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum FnRetTy {
    Default(Span),
    Ty(Ty),
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Box<Expr>>,
    pub span: Span,
}

// --- Generics ---

#[derive(Debug, Clone, Default)]
pub struct Generics {
    pub params: Vec<GenericParam>,
    pub where_clause: Vec<WherePredicate>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GenericParam {
    Lifetime(LifetimeParam),
    Type(TypeParam),
}

#[derive(Debug, Clone)]
pub struct LifetimeParam {
    pub ident: Ident,
    pub bounds: Vec<Lifetime>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub ident: Ident,
    pub bounds: Vec<TypeBound>,
    pub default: Option<Ty>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Lifetime {
    pub ident: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeBound {
    Trait(TraitBound),
    Lifetime(Lifetime),
}

#[derive(Debug, Clone)]
pub struct TraitBound {
    pub path: Path,
    pub args: Vec<GenericArg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WherePredicate {
    pub lifetime: Option<Lifetime>,
    pub bounded_ty: Ty,
    pub bounds: Vec<TypeBound>,
    pub span: Span,
}

// --- Types ---

#[derive(Debug, Clone)]
pub enum Ty {
    Bool(Span),
    Char(Span),
    Int(IntTy, Span),
    Uint(UintTy, Span),
    Float(FloatTy, Span),
    Never(Span),
    Tuple(Vec<Ty>, Span),
    Array(Box<Ty>, Box<Expr>, Span),
    Slice(Box<Ty>, Span),
    Ref(Option<Lifetime>, Mutability, Box<Ty>, Span),
    Ptr(Mutability, Box<Ty>, Span),
    FnPtr {
        inputs: Vec<Ty>,
        output: Box<Ty>,
        abi: Abi,
        is_unsafe: bool,
        span: Span,
    },
    Path(QSelf, Path, Span),
    TraitObject {
        bounds: Vec<TypeBound>,
        lifetime: Option<Lifetime>,
        span: Span,
    },
    ImplTrait(Vec<TypeBound>, Span),
    Infer(Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UintTy {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatTy {
    F32,
    F64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Mutable,
    Immutable,
}

#[derive(Debug, Clone, Default)]
pub struct QSelf {
    pub ty: Option<Box<Ty>>,
    pub position: usize,
}

#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub leading: PathLeading,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathLeading {
    None,
    Root,  // ::
    Crate, // crate::
    Super, // super::
    Self_, // self::
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub ident: Ident,
    pub args: Option<GenericArgs>,
}

#[derive(Debug, Clone)]
pub enum GenericArgs {
    AngleBracketed(Vec<GenericArg>),
    Parenthesized(Vec<Ty>, Box<Ty>),
}

#[derive(Debug, Clone)]
pub enum GenericArg {
    Lifetime(Lifetime),
    Type(Ty),
    Assoc(Ident, Ty),
}

// --- Patterns ---

#[derive(Debug, Clone)]
pub enum Pat {
    Wild(Span),
    Ident(BindingMode, Ident, Option<Box<Pat>>),
    Struct(Path, Vec<PatField>, bool, Span),
    TupleStruct(Path, Vec<Pat>, Span),
    Tuple(Vec<Pat>, Span),
    Slice(Vec<Pat>, Option<Box<Pat>>, Span),
    Or(Vec<Pat>, Span),
    Path(Path, Span),
    Lit(Box<Expr>),
    Range(Option<Box<Expr>>, Option<Box<Expr>>, RangeEnd, Span),
    Ref(Box<Pat>, Mutability, Span),
    Rest(Span),
}

#[derive(Debug, Clone)]
pub struct PatField {
    pub ident: Ident,
    pub pat: Pat,
    pub is_shorthand: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMode {
    ByValue,
    ByRef(Mutability),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeEnd {
    Included,
    Excluded,
}

// --- Expressions ---

#[derive(Debug, Clone)]
pub enum Expr {
    Lit(LitKind, Span),
    Path(Option<QSelf>, Path, Span),
    Block(Block, Span),
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
        generic_args: Option<GenericArgs>,
        span: Span,
    },
    Field {
        receiver: Box<Expr>,
        ident: Ident,
        span: Span,
    },
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Assign {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        op: Option<BinOp>,
        span: Span,
    },
    AddrOf {
        mutability: Mutability,
        expr: Box<Expr>,
        span: Span,
    },
    Cast {
        expr: Box<Expr>,
        ty: Ty,
        span: Span,
    },
    Try {
        expr: Box<Expr>,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then: Block,
        else_: Option<Box<Expr>>,
        span: Span,
    },
    Match {
        expr: Box<Expr>,
        arms: Vec<Arm>,
        span: Span,
    },
    Loop {
        body: Block,
        span: Span,
    },
    While {
        cond: Box<Expr>,
        body: Block,
        span: Span,
    },
    For {
        pat: Pat,
        iter: Box<Expr>,
        body: Block,
        span: Span,
    },
    Closure {
        is_move: bool,
        params: Vec<Param>,
        body: Box<Expr>,
        span: Span,
    },
    Return {
        expr: Option<Box<Expr>>,
        span: Span,
    },
    Break {
        expr: Option<Box<Expr>>,
        span: Span,
    },
    Continue {
        span: Span,
    },
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        end_kind: RangeEnd,
        span: Span,
    },
    Tuple {
        elems: Vec<Expr>,
        span: Span,
    },
    Array {
        elems: Vec<Expr>,
        span: Span,
    },
    Repeat {
        elem: Box<Expr>,
        count: Box<Expr>,
        span: Span,
    },
    Struct {
        path: Path,
        fields: Vec<ExprField>,
        span: Span,
    },
    Unsafe(Block, Span),
    Unit(Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    Deref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
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
pub struct Arm {
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExprField {
    pub ident: Ident,
    pub expr: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LitKind {
    Bool(bool),
    Int(u128, Option<IntTy>),
    Uint(u128, Option<UintTy>),
    Float(f64, Option<FloatTy>),
    Char(char),
    Str(Symbol),
    ByteStr(Symbol),
    Byte(u8),
}

// --- Statements ---

#[derive(Debug, Clone)]
pub enum Stmt {
    Local(LocalDecl),
    Expr(Expr, bool /* has_semicolon */),
    Semi,
    Empty(Span),
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub pat: Pat,
    pub ty: Option<Ty>,
    pub init: Option<Expr>,
    pub span: Span,
}

// --- Other declarations ---

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub ident: Ident,
    pub ty: Ty,
    pub expr: Expr,
    pub is_const: bool,
    pub is_mut: bool,
    pub span: Span,
}

pub type StaticDecl = ConstDecl;

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub ident: Ident,
    pub generics: Generics,
    pub fields: Vec<StructField>,
    pub is_unit: bool,
    pub is_tuple: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub vis: Visibility,
    pub ident: Option<Ident>,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub ident: Ident,
    pub generics: Generics,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub ident: Ident,
    pub data: VariantData,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum VariantData {
    Unit(Span),
    Tuple(Vec<StructField>, Span),
    Struct(Vec<StructField>, Span),
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub ident: Ident,
    pub generics: Generics,
    pub supertraits: Vec<TypeBound>,
    pub items: Vec<TraitItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TraitItem {
    Fn(FnSig, Option<Block>),
    Type(Ident, Vec<TypeBound>, Option<Ty>),
    Const(Ident, Ty, Option<Expr>),
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub generics: Generics,
    pub of_trait: Option<Path>,
    pub self_ty: Ty,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub ident: Ident,
    pub generics: Generics,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ExternBlock {
    pub abi: Abi,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ModDecl {
    Inline(Vec<Item>, Span),
    Loaded(Span),
}

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub tree: UseTree,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum UseTree {
    Path {
        prefix: Path,
        children: Vec<UseTree>,
    },
    Leaf(Path, Option<Ident>),
    Glob(Path),
}

// --- Identifier ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ident {
    pub name: Symbol,
    pub span: Span,
}

impl Ident {
    pub fn new(name: Symbol, span: Span) -> Self {
        Self { name, span }
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.name)
    }
}
