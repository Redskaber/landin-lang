//! Path lowering.

use crate::ast;
use crate::hir::kinds::*;
use crate::hir::lower::cx::HirLowerCtxt;

/// Lower an AST path to HIR. Sets `res: Res::Unknown` (Stage 1.3 will
/// populate it via name resolution).
///
/// Stage 127 (v0.13 — TD-TRAIT-METHOD-AMBIGUITY): `qself` defaults to `None`
/// here. Callers that need UFCS (`<T as Trait>::method`) should use
/// `lower_path_with_qself` instead. Per §1.0 原則 6 (通解 > 特例): one
/// `lower_path` for the common case, one `lower_path_with_qself` for the
/// qualified case — no per-call-site special handling.
pub fn lower_path(cx: &mut HirLowerCtxt, path: &ast::Path) -> HirPath {
    HirPath {
        hir_id: cx.fresh_hir_id(),
        segments: path
            .segments
            .iter()
            .map(|s| HirPathSegment {
                ident: s.ident,
                args: s.args.clone(),
            })
            .collect(),
        leading: path.leading,
        res: Res::Unknown,
        qself: None,
        span: path.span,
    }
}

/// Stage 127 (v0.13 — TD-TRAIT-METHOD-AMBIGUITY): Lower an AST path with
/// an explicit QSelf (UFCS — `<T as Trait>::method`).
///
/// Per §1.0 原則 3 (显式 > 隐式): the trait context is preserved in HIR
/// so typeck + MIR lower can resolve the trait method without ambiguity.
/// Per §1.0 原則 6 (通解 > 特例): one function handles all UFCS forms —
/// Type-only `<T>::method` (qself.ty = Some, position = 0),
/// Trait-qualified `<T as Trait>::method` (qself.ty = Some, position > 0),
/// and short-form `Trait::method` (qself.ty = None, position = 0).
pub fn lower_path_with_qself(
    cx: &mut HirLowerCtxt,
    path: &ast::Path,
    qself: &ast::QSelf,
) -> HirPath {
    use crate::hir::lower::ty::lower_ty;
    let hir_qself = HirQSelf {
        ty: qself.ty.as_ref().map(|t| Box::new(lower_ty(cx, t))),
        position: qself.position,
    };
    HirPath {
        hir_id: cx.fresh_hir_id(),
        segments: path
            .segments
            .iter()
            .map(|s| HirPathSegment {
                ident: s.ident,
                args: s.args.clone(),
            })
            .collect(),
        leading: path.leading,
        res: Res::Unknown,
        qself: Some(hir_qself),
        span: path.span,
    }
}
