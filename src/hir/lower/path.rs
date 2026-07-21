//! Path lowering.

use crate::ast;
use crate::hir::kinds::*;
use crate::hir::lower::cx::HirLowerCtxt;

/// Lower an AST path to HIR. Sets `res: Res::Unknown` (Stage 1.3 will
/// populate it via name resolution).
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
        span: path.span,
    }
}
