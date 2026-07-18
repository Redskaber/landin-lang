//! Generics + where clause + use tree lowering.

use crate::ast;
use crate::hir::kinds::*;
use crate::hir::lower::cx::LowerCtxt;
use crate::hir::lower::ty;

pub fn lower_generics(cx: &mut LowerCtxt, generics: &ast::Generics) -> HirGenerics {
    HirGenerics {
        params: generics
            .params
            .iter()
            .map(|p| lower_generic_param(cx, p))
            .collect(),
        where_clause: generics
            .where_clause
            .iter()
            .map(|w| lower_where_predicate(cx, w))
            .collect(),
        span: generics.span,
    }
}

pub fn lower_generic_param(cx: &mut LowerCtxt, param: &ast::GenericParam) -> HirGenericParam {
    match param {
        ast::GenericParam::Lifetime(lp) => HirGenericParam::Lifetime(HirLifetimeParam {
            hir_id: cx.fresh_hir_id(),
            ident: lp.ident,
            bounds: lp.bounds.clone(),
            span: lp.span,
        }),
        ast::GenericParam::Type(tp) => HirGenericParam::Type(HirTypeParam {
            hir_id: cx.fresh_hir_id(),
            ident: tp.ident,
            bounds: lower_type_bounds(cx, &tp.bounds),
            default: tp.default.as_ref().map(|t| ty::lower_ty(cx, t)),
            span: tp.span,
        }),
    }
}

pub fn lower_type_bounds(cx: &mut LowerCtxt, bounds: &[ast::TypeBound]) -> Vec<HirTypeBound> {
    bounds.iter().map(|b| lower_type_bound(cx, b)).collect()
}

pub fn lower_type_bound(cx: &mut LowerCtxt, bound: &ast::TypeBound) -> HirTypeBound {
    match bound {
        ast::TypeBound::Trait(tb) => HirTypeBound::Trait(HirTraitBound {
            hir_id: cx.fresh_hir_id(),
            path: crate::hir::lower::path::lower_path(cx, &tb.path),
            span: tb.span,
        }),
        ast::TypeBound::Lifetime(lt) => HirTypeBound::Lifetime(lt.clone()),
    }
}

pub fn lower_where_predicate(cx: &mut LowerCtxt, pred: &ast::WherePredicate) -> HirWherePredicate {
    HirWherePredicate {
        hir_id: cx.fresh_hir_id(),
        lifetime: pred.lifetime.clone(),
        bounded_ty: ty::lower_ty(cx, &pred.bounded_ty),
        bounds: lower_type_bounds(cx, &pred.bounds),
        span: pred.span,
    }
}

pub fn lower_use_tree(cx: &mut LowerCtxt, tree: &ast::UseTree) -> HirUseTree {
    match tree {
        ast::UseTree::Path { prefix, children } => HirUseTree::Path {
            prefix: crate::hir::lower::path::lower_path(cx, prefix),
            children: children.iter().map(|t| lower_use_tree(cx, t)).collect(),
        },
        ast::UseTree::Leaf(path, alias) => {
            HirUseTree::Leaf(crate::hir::lower::path::lower_path(cx, path), *alias)
        }
        ast::UseTree::Glob(path) => HirUseTree::Glob(crate::hir::lower::path::lower_path(cx, path)),
    }
}
