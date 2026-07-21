//! Body + expression + statement lowering.

use crate::ast::{self, BinOp, Expr, LitKind, Stmt, UnaryOp};
use crate::hir::kinds::*;
use crate::hir::lower::cx::HirLowerCtxt;
use crate::hir::lower::pat;
use crate::hir::lower::ty;
use crate::session::Span;

impl<'a> HirLowerCtxt<'a> {
    /// Lower a fn body (Block) to a HIR Body and store it.
    pub fn lower_body_internal(&mut self, block: &ast::Block, params: Vec<HirParam>) -> BodyId {
        let body = lower_body(self, block, params);
        self.store_body(body)
    }
}

/// Lower an AST Block to a HIR Body. The Body's `value` is a Block expression.
pub fn lower_body(cx: &mut HirLowerCtxt, block: &ast::Block, params: Vec<HirParam>) -> Body {
    let hir_id = cx.fresh_hir_id();
    let value = lower_block_expr(cx, block);
    Body {
        hir_id,
        params,
        value,
        span: block.span,
    }
}

/// Lower a single expression as a Body (for const/static initializers).
pub fn lower_body_from_expr(
    cx: &mut HirLowerCtxt,
    expr: &ast::Expr,
    params: Vec<HirParam>,
) -> BodyId {
    let hir_id = cx.fresh_hir_id();
    let value = lower_expr(cx, expr);
    let span = expr_span(expr);
    let body = Body {
        hir_id,
        params,
        value,
        span,
    };
    cx.store_body(body)
}

fn expr_span(expr: &ast::Expr) -> Span {
    use crate::ast::Expr::*;
    match expr {
        Lit(_, s) => *s,
        Path(_, _, s) => *s,
        Block(_, s) => *s,
        Call { span, .. } => *span,
        MethodCall { span, .. } => *span,
        Field { span, .. } => *span,
        Index { span, .. } => *span,
        Unary { span, .. } => *span,
        Binary { span, .. } => *span,
        Assign { span, .. } => *span,
        AddrOf { span, .. } => *span,
        Cast { span, .. } => *span,
        Try { span, .. } => *span,
        If { span, .. } => *span,
        Match { span, .. } => *span,
        Loop { span, .. } => *span,
        While { span, .. } => *span,
        For { span, .. } => *span,
        Closure { span, .. } => *span,
        Return { span, .. } => *span,
        Break { span, .. } => *span,
        Continue { span } => *span,
        Range { span, .. } => *span,
        Tuple { span, .. } => *span,
        Array { span, .. } => *span,
        Repeat { span, .. } => *span,
        Struct { span, .. } => *span,
        MacroCall { span, .. } => *span,
        Unsafe(_, s) => *s,
        Unit(s) => *s,
    }
}

fn lower_block_expr(cx: &mut HirLowerCtxt, block: &ast::Block) -> HirExpr {
    let hir_id = cx.fresh_hir_id();
    let stmts: Vec<HirStmt> = block.stmts.iter().map(|s| lower_stmt(cx, s)).collect();
    let expr = block.expr.as_ref().map(|e| Box::new(lower_expr(cx, e)));
    HirExpr {
        hir_id,
        kind: HirExprKind::Block(HirBlock {
            hir_id,
            stmts,
            expr,
            span: block.span,
        }),
        span: block.span,
    }
}

fn lower_stmt(cx: &mut HirLowerCtxt, stmt: &Stmt) -> HirStmt {
    match stmt {
        Stmt::Local(local) => HirStmt::Local(Box::new(HirLocal {
            hir_id: cx.fresh_hir_id(),
            pat: pat::lower_pat(cx, &local.pat),
            ty: local.ty.as_ref().map(|t| ty::lower_ty(cx, t)),
            init: local.init.as_ref().map(|e| lower_expr(cx, e)),
            span: local.span,
        })),
        Stmt::Expr(e, has_semi) => HirStmt::Expr(Box::new(lower_expr(cx, e)), *has_semi),
        Stmt::Semi => HirStmt::Semi,
        Stmt::Empty(s) => HirStmt::Empty(*s),
    }
}

/// Lower an AST expression to HIR.
pub fn lower_expr(cx: &mut HirLowerCtxt, expr: &Expr) -> HirExpr {
    let hir_id = cx.fresh_hir_id();
    let span = expr_span(expr);
    let kind = match expr {
        Expr::Lit(lit, _) => HirExprKind::Lit(lower_lit_kind(lit)),
        Expr::Path(_, path, _) => HirExprKind::Path(crate::hir::lower::path::lower_path(cx, path)),
        Expr::Block(block, _) => {
            let stmts: Vec<HirStmt> = block.stmts.iter().map(|s| lower_stmt(cx, s)).collect();
            let block_expr = block.expr.as_ref().map(|e| Box::new(lower_expr(cx, e)));
            HirExprKind::Block(HirBlock {
                hir_id,
                stmts,
                expr: block_expr,
                span: block.span,
            })
        }
        Expr::Call { func, args, .. } => HirExprKind::Call {
            func: Box::new(lower_expr(cx, func)),
            args: args.iter().map(|a| lower_expr(cx, a)).collect(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
            generic_args,
            ..
        } => HirExprKind::MethodCall {
            receiver: Box::new(lower_expr(cx, receiver)),
            method: *method,
            args: args.iter().map(|a| lower_expr(cx, a)).collect(),
            generic_args: generic_args.clone(),
        },
        Expr::Field {
            receiver, ident, ..
        } => HirExprKind::Field {
            receiver: Box::new(lower_expr(cx, receiver)),
            ident: *ident,
        },
        Expr::Index {
            receiver, index, ..
        } => HirExprKind::Index {
            receiver: Box::new(lower_expr(cx, receiver)),
            index: Box::new(lower_expr(cx, index)),
        },
        Expr::Unary { op, expr, .. } => HirExprKind::Unary {
            op: lower_unary_op(*op),
            expr: Box::new(lower_expr(cx, expr)),
        },
        Expr::Binary { op, lhs, rhs, .. } => HirExprKind::Binary {
            op: lower_bin_op(*op),
            lhs: Box::new(lower_expr(cx, lhs)),
            rhs: Box::new(lower_expr(cx, rhs)),
        },
        Expr::Assign { lhs, rhs, op, .. } => HirExprKind::Assign {
            lhs: Box::new(lower_expr(cx, lhs)),
            rhs: Box::new(lower_expr(cx, rhs)),
            op: op.map(lower_bin_op),
        },
        Expr::AddrOf {
            mutability, expr, ..
        } => HirExprKind::AddrOf {
            mutability: *mutability,
            expr: Box::new(lower_expr(cx, expr)),
        },
        Expr::Cast { expr, ty, .. } => HirExprKind::Cast {
            expr: Box::new(lower_expr(cx, expr)),
            ty: ty::lower_ty(cx, ty),
        },
        Expr::Try { expr, .. } => HirExprKind::Try {
            expr: Box::new(lower_expr(cx, expr)),
        },
        Expr::If {
            cond, then, else_, ..
        } => HirExprKind::If {
            cond: Box::new(lower_expr(cx, cond)),
            then: lower_block(cx, then),
            else_: else_.as_ref().map(|e| Box::new(lower_expr(cx, e))),
        },
        Expr::Match { expr, arms, .. } => HirExprKind::Match {
            expr: Box::new(lower_expr(cx, expr)),
            arms: arms
                .iter()
                .map(|arm| HirArm {
                    hir_id: cx.fresh_hir_id(),
                    pat: pat::lower_pat(cx, &arm.pat),
                    guard: arm.guard.as_ref().map(|g| lower_expr(cx, g)),
                    body: Box::new(lower_expr(cx, &arm.body)),
                    span: arm.span,
                })
                .collect(),
        },
        Expr::Loop { body, .. } => HirExprKind::Loop {
            body: lower_block(cx, body),
        },
        Expr::While { cond, body, .. } => HirExprKind::While {
            cond: Box::new(lower_expr(cx, cond)),
            body: lower_block(cx, body),
        },
        Expr::For {
            pat, iter, body, ..
        } => HirExprKind::For {
            pat: pat::lower_pat(cx, pat),
            iter: Box::new(lower_expr(cx, iter)),
            body: lower_block(cx, body),
        },
        Expr::Closure {
            is_move,
            params,
            body,
            ..
        } => HirExprKind::Closure {
            is_move: *is_move,
            params: params.iter().map(|p| cx.lower_param(p)).collect(),
            body: Box::new(lower_expr(cx, body)),
        },
        Expr::Return { expr, .. } => HirExprKind::Return {
            expr: expr.as_ref().map(|e| Box::new(lower_expr(cx, e))),
        },
        Expr::Break { expr, .. } => HirExprKind::Break {
            expr: expr.as_ref().map(|e| Box::new(lower_expr(cx, e))),
        },
        Expr::Continue { .. } => HirExprKind::Continue,
        Expr::Range {
            start,
            end,
            end_kind,
            ..
        } => HirExprKind::Range {
            start: start.as_ref().map(|e| Box::new(lower_expr(cx, e))),
            end: end.as_ref().map(|e| Box::new(lower_expr(cx, e))),
            end_kind: *end_kind,
        },
        Expr::Tuple { elems, .. } => HirExprKind::Tuple {
            elems: elems.iter().map(|e| lower_expr(cx, e)).collect(),
        },
        Expr::Array { elems, .. } => HirExprKind::Array {
            elems: elems.iter().map(|e| lower_expr(cx, e)).collect(),
        },
        Expr::Repeat { elem, count, .. } => HirExprKind::Repeat {
            elem: Box::new(lower_expr(cx, elem)),
            count: Box::new(lower_expr(cx, count)),
        },
        Expr::Struct { path, fields, .. } => HirExprKind::Struct {
            path: crate::hir::lower::path::lower_path(cx, path),
            fields: fields
                .iter()
                .map(|f| HirExprField {
                    hir_id: cx.fresh_hir_id(),
                    ident: f.ident,
                    expr: f.expr.as_ref().map(|e| lower_expr(cx, e)),
                    span: f.span,
                })
                .collect(),
        },
        Expr::MacroCall { path, delim, .. } => HirExprKind::MacroCall {
            path: crate::hir::lower::path::lower_path(cx, path),
            delim: *delim,
        },
        Expr::Unsafe(block, _) => HirExprKind::Unsafe(lower_block(cx, block)),
        Expr::Unit(_) => HirExprKind::Unit,
    };
    HirExpr { hir_id, kind, span }
}

fn lower_block(cx: &mut HirLowerCtxt, block: &ast::Block) -> HirBlock {
    let hir_id = cx.fresh_hir_id();
    let stmts: Vec<HirStmt> = block.stmts.iter().map(|s| lower_stmt(cx, s)).collect();
    let expr = block.expr.as_ref().map(|e| Box::new(lower_expr(cx, e)));
    HirBlock {
        hir_id,
        stmts,
        expr,
        span: block.span,
    }
}

fn lower_lit_kind(lit: &LitKind) -> HirLitKind {
    match lit {
        LitKind::Bool(b) => HirLitKind::Bool(*b),
        LitKind::Int(n, ty) => HirLitKind::Int(*n, *ty),
        LitKind::Uint(n, ty) => HirLitKind::Uint(*n, *ty),
        LitKind::Float(f, ty) => HirLitKind::Float(*f, *ty),
        LitKind::Char(c) => HirLitKind::Char(*c),
        LitKind::Str(s) => HirLitKind::Str(*s),
        LitKind::ByteStr(s) => HirLitKind::ByteStr(*s),
        LitKind::Byte(b) => HirLitKind::Byte(*b),
    }
}

fn lower_unary_op(op: UnaryOp) -> HirUnaryOp {
    match op {
        UnaryOp::Neg => HirUnaryOp::Neg,
        UnaryOp::Not => HirUnaryOp::Not,
        UnaryOp::Deref => HirUnaryOp::Deref,
    }
}

fn lower_bin_op(op: BinOp) -> HirBinOp {
    match op {
        BinOp::Add => HirBinOp::Add,
        BinOp::Sub => HirBinOp::Sub,
        BinOp::Mul => HirBinOp::Mul,
        BinOp::Div => HirBinOp::Div,
        BinOp::Rem => HirBinOp::Rem,
        BinOp::BitAnd => HirBinOp::BitAnd,
        BinOp::BitOr => HirBinOp::BitOr,
        BinOp::BitXor => HirBinOp::BitXor,
        BinOp::Shl => HirBinOp::Shl,
        BinOp::Shr => HirBinOp::Shr,
        BinOp::And => HirBinOp::And,
        BinOp::Or => HirBinOp::Or,
        BinOp::Eq => HirBinOp::Eq,
        BinOp::Ne => HirBinOp::Ne,
        BinOp::Lt => HirBinOp::Lt,
        BinOp::Le => HirBinOp::Le,
        BinOp::Gt => HirBinOp::Gt,
        BinOp::Ge => HirBinOp::Ge,
    }
}
