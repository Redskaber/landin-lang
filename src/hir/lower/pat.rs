//! Pattern lowering.

use crate::ast::{self};
use crate::hir::kinds::*;
use crate::hir::lower::cx::HirLowerCtxt;
use crate::hir::lower::path;
use crate::session::Span;

pub fn lower_pat(cx: &mut HirLowerCtxt, pat: &ast::Pat) -> HirPat {
    let hir_id = cx.fresh_hir_id();
    let span = pat_span(pat);
    let kind = match pat {
        ast::Pat::Wild(_s) => HirPatKind::Wild,
        ast::Pat::Ident(mode, ident, sub) => HirPatKind::Ident(
            *mode,
            *ident,
            sub.as_ref().map(|p| Box::new(lower_pat(cx, p))),
        ),
        ast::Pat::Struct(p, fields, has_rest, _) => HirPatKind::Struct(
            path::lower_path(cx, p),
            fields
                .iter()
                .map(|f| HirPatField {
                    hir_id: cx.fresh_hir_id(),
                    ident: f.ident,
                    pat: lower_pat(cx, &f.pat),
                    is_shorthand: f.is_shorthand,
                    span: f.span,
                })
                .collect(),
            *has_rest,
        ),
        ast::Pat::TupleStruct(p, pats, _) => HirPatKind::TupleStruct(
            path::lower_path(cx, p),
            pats.iter().map(|p| lower_pat(cx, p)).collect(),
        ),
        ast::Pat::Tuple(pats, _) => {
            HirPatKind::Tuple(pats.iter().map(|p| lower_pat(cx, p)).collect())
        }
        ast::Pat::Slice(pats, rest, _) => HirPatKind::Slice(
            pats.iter().map(|p| lower_pat(cx, p)).collect(),
            rest.as_ref().map(|p| Box::new(lower_pat(cx, p))),
        ),
        ast::Pat::Or(pats, _) => HirPatKind::Or(pats.iter().map(|p| lower_pat(cx, p)).collect()),
        ast::Pat::Path(p, _) => HirPatKind::Path(path::lower_path(cx, p)),
        ast::Pat::Lit(expr) => {
            HirPatKind::Lit(Box::new(crate::hir::lower::body::lower_expr(cx, expr)))
        }
        ast::Pat::Range(start, end, end_kind, _) => HirPatKind::Range(
            start
                .as_ref()
                .map(|e| Box::new(crate::hir::lower::body::lower_expr(cx, e))),
            end.as_ref()
                .map(|e| Box::new(crate::hir::lower::body::lower_expr(cx, e))),
            *end_kind,
        ),
        ast::Pat::Ref(p, mutability, _) => HirPatKind::Ref(Box::new(lower_pat(cx, p)), *mutability),
        ast::Pat::Rest(_) => HirPatKind::Rest,
    };
    HirPat { hir_id, kind, span }
}

fn pat_span(pat: &ast::Pat) -> Span {
    use ast::Pat::*;
    match pat {
        Wild(s) => *s,
        // Stage 18.57: Use ident.span instead of Span::DUMMY.
        // Per §1.0 原則 3 "显式 > 隐式": span is explicitly sourced from AST.
        Ident(_, ident, _) => ident.span,
        Struct(_, _, _, s) => *s,
        TupleStruct(_, _, s) => *s,
        Tuple(_, s) => *s,
        Slice(_, _, s) => *s,
        Or(_, s) => *s,
        Path(_, s) => *s,
        // Stage 18.57: Use the literal expression's span instead of Span::DUMMY.
        Lit(expr) => expr_span(expr),
        Range(_, _, _, s) => *s,
        Ref(_, _, s) => *s,
        Rest(s) => *s,
    }
}

/// Stage 18.57: Extract span from an AST expression for pattern literals.
///
/// Per §1.0 原則 6 "通用 > 特例": one helper for all expr variants.
fn expr_span(expr: &ast::Expr) -> Span {
    use ast::Expr::*;
    match expr {
        Lit(_, s) => *s,
        Path(_, _, s) => *s,
        _ => Span::DUMMY, // fallback for non-literal expr patterns (rare)
    }
}
