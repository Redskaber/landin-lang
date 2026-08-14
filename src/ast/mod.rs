//! AST: Abstract Syntax Tree.
//!
//! Based on 05-ast.md. Carries spans for all nodes.

pub mod kinds;

// Stage 3.63 (cross-stage naming standardization): explicit list instead of
// `pub use kinds::*;` to prevent accidental leakage of internal types.
// Matches the same pattern already established in src/hir/mod.rs and
// src/mir/mod.rs (Stage 3.57 P0-3 fix).
pub use kinds::{
    Abi, Arm, Attr, AttrArg, AttrArgs, BinOp, BindingMode, Block, ConstDecl, Crate, EnumDecl,
    EnumVariant, Expr, ExprField, ExternBlock, FloatTy, FnDecl, FnRetTy, FnSig, GenericArg,
    GenericArgs, GenericParam, Generics, Ident, ImplDecl, IntTy, Item, ItemKind, Lifetime,
    LifetimeParam, LitKind, LocalDecl, MacroDelim, MacroRule, MacroRulesDef, ModDecl, Mutability,
    Param, Pat, PatField, Path, PathLeading, PathSegment, QSelf, RangeEnd, SelfKind, StaticDecl,
    Stmt, StructDecl, StructField, TraitBound, TraitDecl, TraitItem, Ty, TypeAliasDecl, TypeBound,
    TypeParam, UintTy, UnaryOp, UseDecl, UseTree, VariantData, Visibility, WherePredicate,
};
