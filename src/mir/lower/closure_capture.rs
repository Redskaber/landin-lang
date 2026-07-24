//! Stage 6.2: Closure capture extraction from mir/lower/mod.rs (TD-011 split step 2).
//!
//! Extracted from `mir/lower/mod.rs` to reduce its LOC (3193 → ~3030).
//! Contains functions for walking HIR expressions to collect external locals
//! captured by closures (Stage 4.7 closure support).

use crate::hir::*;
use crate::mir::place::LocalId;

use super::MirLowerCtxt;

/// Stage 4.7: Walk a HirExpr tree and collect all external locals that are
/// referenced (via `HirExprKind::Path` with `Res::Local`) but not in
/// `param_hir_ids` (i.e., not closure parameters).
pub(crate) fn collect_captured_locals(
    cx: &MirLowerCtxt,
    expr: &HirExpr,
    param_hir_ids: &std::collections::HashSet<HirId>,
    captured: &mut Vec<(HirId, LocalId)>,
    seen: &mut std::collections::HashSet<HirId>,
) {
    match &expr.kind {
        HirExprKind::Path(path) => {
            if let Res::Local(hir_id) = path.res {
                if !param_hir_ids.contains(&hir_id) && !seen.contains(&hir_id) {
                    if let Some(local_id) = cx.local_of(hir_id) {
                        seen.insert(hir_id);
                        captured.push((hir_id, local_id));
                    }
                }
            }
        }
        HirExprKind::Call { func, args } => {
            collect_captured_locals(cx, func, param_hir_ids, captured, seen);
            for a in args {
                collect_captured_locals(cx, a, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::MethodCall { receiver, args, .. } => {
            collect_captured_locals(cx, receiver, param_hir_ids, captured, seen);
            for a in args {
                collect_captured_locals(cx, a, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Field { receiver, .. } => {
            collect_captured_locals(cx, receiver, param_hir_ids, captured, seen);
        }
        HirExprKind::Index { receiver, index } => {
            collect_captured_locals(cx, receiver, param_hir_ids, captured, seen);
            collect_captured_locals(cx, index, param_hir_ids, captured, seen);
        }
        HirExprKind::Unary { expr, .. } => {
            collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            collect_captured_locals(cx, lhs, param_hir_ids, captured, seen);
            collect_captured_locals(cx, rhs, param_hir_ids, captured, seen);
        }
        HirExprKind::Assign { lhs, rhs, .. } => {
            collect_captured_locals(cx, lhs, param_hir_ids, captured, seen);
            collect_captured_locals(cx, rhs, param_hir_ids, captured, seen);
        }
        HirExprKind::AddrOf { expr, .. } => {
            collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
        }
        HirExprKind::Cast { expr, .. } => {
            collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
        }
        HirExprKind::Try { expr } => {
            collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
        }
        HirExprKind::If { cond, then, else_ } => {
            collect_captured_locals(cx, cond, param_hir_ids, captured, seen);
            collect_block_captured(cx, then, param_hir_ids, captured, seen);
            if let Some(e) = else_ {
                collect_captured_locals(cx, e, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Match { expr, arms } => {
            collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
            for arm in arms {
                collect_captured_locals(cx, &arm.body, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Block(b) => {
            collect_block_captured(cx, b, param_hir_ids, captured, seen);
        }
        HirExprKind::Loop { body } => {
            collect_block_captured(cx, body, param_hir_ids, captured, seen);
        }
        HirExprKind::While { cond, body } => {
            collect_captured_locals(cx, cond, param_hir_ids, captured, seen);
            collect_block_captured(cx, body, param_hir_ids, captured, seen);
        }
        HirExprKind::For { iter, body, .. } => {
            collect_captured_locals(cx, iter, param_hir_ids, captured, seen);
            collect_block_captured(cx, body, param_hir_ids, captured, seen);
        }
        HirExprKind::Closure { body, .. } => {
            collect_captured_locals(cx, body, param_hir_ids, captured, seen);
        }
        HirExprKind::Return { expr } => {
            if let Some(e) = expr {
                collect_captured_locals(cx, e, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Break { expr, .. } => {
            if let Some(e) = expr {
                collect_captured_locals(cx, e, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Continue
        | HirExprKind::Lit(_)
        | HirExprKind::Unit
        | HirExprKind::MacroCall { .. } => {}
        HirExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_captured_locals(cx, s, param_hir_ids, captured, seen);
            }
            if let Some(e) = end {
                collect_captured_locals(cx, e, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Tuple { elems } => {
            for e in elems {
                collect_captured_locals(cx, e, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Array { elems } => {
            for e in elems {
                collect_captured_locals(cx, e, param_hir_ids, captured, seen);
            }
        }
        HirExprKind::Repeat { elem, count } => {
            collect_captured_locals(cx, elem, param_hir_ids, captured, seen);
            collect_captured_locals(cx, count, param_hir_ids, captured, seen);
        }
        HirExprKind::Struct { fields, .. } => {
            for f in fields {
                if let Some(e) = &f.expr {
                    collect_captured_locals(cx, e, param_hir_ids, captured, seen);
                }
            }
        }
        HirExprKind::Unsafe(b) => {
            collect_block_captured(cx, b, param_hir_ids, captured, seen);
        }
    }
}

/// Helper: walk a HirBlock's statements + final expr for captured locals.
pub(crate) fn collect_block_captured(
    cx: &MirLowerCtxt,
    block: &HirBlock,
    param_hir_ids: &std::collections::HashSet<HirId>,
    captured: &mut Vec<(HirId, LocalId)>,
    seen: &mut std::collections::HashSet<HirId>,
) {
    for stmt in &block.stmts {
        match stmt {
            HirStmt::Local(local) => {
                if let Some(init) = &local.init {
                    collect_captured_locals(cx, init, param_hir_ids, captured, seen);
                }
            }
            HirStmt::Expr(expr, _) => {
                collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
            }
            HirStmt::Semi | HirStmt::Empty(_) => {}
        }
    }
    if let Some(expr) = &block.expr {
        collect_captured_locals(cx, expr, param_hir_ids, captured, seen);
    }
}
