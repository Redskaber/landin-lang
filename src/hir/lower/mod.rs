//! AST → HIR lowering.
//!
//! Per Stage 1.2 plan: convert all AST nodes to HIR nodes, assigning fresh
//! `HirId`s and populating `Res::Unknown` placeholders for Stage 1.3 name
//! resolution.
//!
//! Public entry point: [`lower_crate`].

pub mod body;
pub mod cx;
pub mod error;
pub mod generics;
pub mod item;
pub mod pat;
pub mod path;
pub mod ty;

pub use cx::HirLowerCtxt;
pub use error::LowerError;

use crate::ast;
use crate::hir::HirCrate;
use lasso::Rodeo;

/// Lower an AST crate to HIR.
///
/// This is the main entry point for Stage 1.2. It walks the AST in
/// pre-order, allocating `DefId`s for each owner and `HirId`s for each
/// node, and produces a flat `HirCrate` with all owners and bodies.
///
/// `Res` fields on all `HirPath` nodes are set to `Res::Unknown`; the
/// Stage 1.3 name resolver will fill them in.
/// `InferTy` fields on all `HirTy` nodes are set to `None`; the Stage 2
/// type checker will fill them in.
pub fn lower_crate(ast: &ast::Crate, interner: &Rodeo) -> (HirCrate, Vec<LowerError>) {
    let mut cx = HirLowerCtxt::new(interner);
    for item in &ast.items {
        cx.lower_item(item);
    }
    // Stage 18.78 P0-A: Return errors alongside HIR so driver can populate
    // CompileErrors.lower. Previously errors were silently discarded.
    cx.into_hir()
}
