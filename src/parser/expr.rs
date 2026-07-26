//! Stage 6.12 (TD-022): Expression parsing sub-module (Pratt parser).
//!
//! Per 02-grammar.md §3.4 (Expression) + §2 (Pratt priority table).
//! Extracted from `parser.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns:
//! - `binop_bp` / `assign_op` (Pratt precedence helpers)
//! - `parse_expr` (entry) + `parse_assign_expr` + `parse_range_expr`
//! - 13 Pratt-level functions (`parse_or_expr` ... `parse_mul_expr` +
//!   `parse_cast_expr`)
//! - `parse_unary_expr` / `parse_postfix_expr` / `parse_primary_expr`
//! - `parse_if_expr` / `parse_match_expr`
//! - `is_expr_start` (lookahead helper)

use crate::ast;
use crate::ast::*;
use crate::lexer::token::*;
use crate::session::Span;

use super::parser::Parser;

impl<'a> Parser<'a> {
    pub(super) fn binop_bp(op: &TokenKind) -> Option<(BinOp, u8)> {
        Some(match op {
            TokenKind::OrOr => (BinOp::Or, 1),
            TokenKind::AndAnd => (BinOp::And, 2),
            TokenKind::EqEq => (BinOp::Eq, 3),
            TokenKind::NotEq => (BinOp::Ne, 3),
            TokenKind::Lt => (BinOp::Lt, 3),
            TokenKind::Gt => (BinOp::Gt, 3),
            TokenKind::LtEq => (BinOp::Le, 3),
            TokenKind::GtEq => (BinOp::Ge, 3),
            TokenKind::Or => (BinOp::BitOr, 4),
            TokenKind::Caret => (BinOp::BitXor, 5),
            TokenKind::And => (BinOp::BitAnd, 6),
            TokenKind::Shl => (BinOp::Shl, 7),
            TokenKind::Shr => (BinOp::Shr, 7),
            TokenKind::Plus => (BinOp::Add, 8),
            TokenKind::Minus => (BinOp::Sub, 8),
            TokenKind::Star => (BinOp::Mul, 9),
            TokenKind::Slash => (BinOp::Div, 9),
            TokenKind::Percent => (BinOp::Rem, 9),
            _ => return None,
        })
    }

    pub(super) fn assign_op(op: &TokenKind) -> Option<Option<BinOp>> {
        Some(match op {
            TokenKind::Eq => None,
            TokenKind::PlusEq => Some(BinOp::Add),
            TokenKind::MinusEq => Some(BinOp::Sub),
            TokenKind::StarEq => Some(BinOp::Mul),
            TokenKind::SlashEq => Some(BinOp::Div),
            TokenKind::PercentEq => Some(BinOp::Rem),
            TokenKind::AndEq => Some(BinOp::BitAnd),
            TokenKind::OrEq => Some(BinOp::BitOr),
            TokenKind::CaretEq => Some(BinOp::BitXor),
            TokenKind::ShlEq => Some(BinOp::Shl),
            TokenKind::ShrEq => Some(BinOp::Shr),
            _ => return None,
        })
    }

    pub fn parse_expr(&mut self) -> Expr {
        self.parse_assign_expr()
    }

    pub(super) fn parse_assign_expr(&mut self) -> Expr {
        let lhs = self.parse_range_expr();
        let span = lhs.span();

        if let Some(op) = Self::assign_op(self.peek()) {
            self.bump();
            let rhs = self.parse_assign_expr();
            return Expr::Assign {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                op,
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_range_expr(&mut self) -> Expr {
        // Check for range start..
        let lhs = self.parse_or_expr();
        let span = lhs.span();

        match self.peek() {
            TokenKind::DotDot => {
                self.bump();
                // Check if there's a rhs
                let end = if self.is_expr_start() {
                    Some(Box::new(self.parse_or_expr()))
                } else {
                    None
                };
                Expr::Range {
                    start: Some(Box::new(lhs)),
                    end,
                    end_kind: RangeEnd::Excluded,
                    span,
                }
            }
            TokenKind::DotDotEq => {
                self.bump();
                let end = Some(Box::new(self.parse_or_expr()));
                Expr::Range {
                    start: Some(Box::new(lhs)),
                    end,
                    end_kind: RangeEnd::Included,
                    span,
                }
            }
            _ => lhs,
        }
    }

    pub(super) fn parse_or_expr(&mut self) -> Expr {
        let mut lhs = self.parse_and_expr();
        while let Some((op, _bp)) = Self::binop_bp(self.peek()) {
            if op != BinOp::Or {
                break;
            }
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_and_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_and_expr(&mut self) -> Expr {
        let mut lhs = self.parse_cmp_expr();
        while let Some((op, _bp)) = Self::binop_bp(self.peek()) {
            if op != BinOp::And {
                break;
            }
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_cmp_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_cmp_expr(&mut self) -> Expr {
        let mut lhs = self.parse_bitor_expr();
        while let Some((op, _bp)) = Self::binop_bp(self.peek()) {
            if !matches!(
                op,
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
            ) {
                break;
            }
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_bitor_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_bitor_expr(&mut self) -> Expr {
        let mut lhs = self.parse_bitxor_expr();
        while *self.peek() == TokenKind::Or {
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_bitxor_expr();
            lhs = Expr::Binary {
                op: BinOp::BitOr,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_bitxor_expr(&mut self) -> Expr {
        let mut lhs = self.parse_bitand_expr();
        while *self.peek() == TokenKind::Caret {
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_bitand_expr();
            lhs = Expr::Binary {
                op: BinOp::BitXor,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_bitand_expr(&mut self) -> Expr {
        let mut lhs = self.parse_shift_expr();
        while *self.peek() == TokenKind::And {
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_shift_expr();
            lhs = Expr::Binary {
                op: BinOp::BitAnd,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_shift_expr(&mut self) -> Expr {
        let mut lhs = self.parse_add_expr();
        while matches!(self.peek(), TokenKind::Shl | TokenKind::Shr) {
            let (op, _) = Self::binop_bp(self.peek()).unwrap();
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_add_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_add_expr(&mut self) -> Expr {
        let mut lhs = self.parse_mul_expr();
        while matches!(self.peek(), TokenKind::Plus | TokenKind::Minus) {
            let (op, _) = Self::binop_bp(self.peek()).unwrap();
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_mul_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_mul_expr(&mut self) -> Expr {
        let mut lhs = self.parse_cast_expr();
        while matches!(
            self.peek(),
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent
        ) {
            let (op, _) = Self::binop_bp(self.peek()).unwrap();
            let span = lhs.span();
            self.bump();
            let rhs = self.parse_cast_expr();
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_cast_expr(&mut self) -> Expr {
        let lhs = self.parse_unary_expr();
        let span = lhs.span();
        if *self.peek() == TokenKind::KwAs {
            self.bump();
            let ty = self.parse_ty();
            return Expr::Cast {
                expr: Box::new(lhs),
                ty,
                span,
            };
        }
        lhs
    }

    pub(super) fn parse_unary_expr(&mut self) -> Expr {
        let span = self.current_span();
        match self.peek() {
            TokenKind::Minus => {
                self.bump();
                let expr = self.parse_unary_expr();
                Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::Not => {
                self.bump();
                let expr = self.parse_unary_expr();
                Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::Star => {
                self.bump();
                let expr = self.parse_unary_expr();
                Expr::Unary {
                    op: UnaryOp::Deref,
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::And => {
                self.bump();
                let mutability = if *self.peek() == TokenKind::KwMut {
                    self.bump();
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                let expr = self.parse_unary_expr();
                Expr::AddrOf {
                    mutability,
                    expr: Box::new(expr),
                    span,
                }
            }
            _ => self.parse_postfix_expr(),
        }
    }

    pub(super) fn parse_postfix_expr(&mut self) -> Expr {
        let mut expr = self.parse_primary_expr();

        loop {
            let span = expr.span();
            match self.peek() {
                TokenKind::Dot => {
                    self.bump();
                    // Field access or method call or tuple field
                    match self.peek() {
                        TokenKind::Ident(_) => {
                            let ident = self.ident_from_token();
                            self.bump();
                            // Check for turbofish: `method::<i32>` (generic
                            // args on a method call). Per Round 8c fix.
                            let generic_args = self.try_parse_turbofish_or_generic_args();
                            // Check for method call
                            if *self.peek() == TokenKind::LParen {
                                self.bump();
                                let mut args = Vec::new();
                                while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                                    args.push(self.parse_expr());
                                    if !self.eat(&TokenKind::Comma) {
                                        break;
                                    }
                                }
                                self.expect(&TokenKind::RParen, "`)`");
                                expr = Expr::MethodCall {
                                    receiver: Box::new(expr),
                                    method: ident,
                                    args,
                                    generic_args,
                                    span,
                                };
                            } else {
                                expr = Expr::Field {
                                    receiver: Box::new(expr),
                                    ident,
                                    span,
                                };
                            }
                        }
                        TokenKind::IntLit(value, _) => {
                            // Tuple field access: t.0, t.1, etc.
                            // Stage 3.30 fix: intern the integer as a string
                            // so MIR lower can recover the field index via
                            // `name_str.parse::<u32>()`. Was: used
                            // `Spur::default()` (lost the index entirely —
                            // all tuple field accesses resolved to field 0).
                            //
                            // Clone the value to release the immutable borrow
                            // from `self.peek()` before calling `self.bump()`.
                            let value: u128 = *value;
                            self.bump();
                            let field_name = format!("{}", value);
                            let field_spur = self.interner.get_or_intern(field_name.as_str());
                            expr = Expr::Field {
                                receiver: Box::new(expr),
                                ident: Ident::new(field_spur, span),
                                span,
                            };
                        }
                        _ => {
                            self.errors.push(crate::parser::ParseError::new(
                                format!("expected field name after `.`, found {}", self.peek()),
                                span,
                            ));
                            break;
                        }
                    }
                }
                TokenKind::LParen => {
                    self.bump();
                    let mut args = Vec::new();
                    while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        args.push(self.parse_expr());
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`");
                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                        span,
                    };
                }
                TokenKind::LBracket => {
                    self.bump();
                    let index = self.parse_expr();
                    self.expect(&TokenKind::RBracket, "`]`");
                    expr = Expr::Index {
                        receiver: Box::new(expr),
                        index: Box::new(index),
                        span,
                    };
                }
                TokenKind::Question => {
                    self.bump();
                    expr = Expr::Try {
                        expr: Box::new(expr),
                        span,
                    };
                }
                _ => break,
            }
        }
        expr
    }

    pub(super) fn parse_primary_expr(&mut self) -> Expr {
        let span = self.current_span();
        match self.peek().clone() {
            TokenKind::IntLit(val, suffix) => {
                self.bump();
                let lit_kind = match suffix {
                    Some(crate::lexer::token::IntTy::I8) => LitKind::Int(val, Some(ast::IntTy::I8)),
                    Some(crate::lexer::token::IntTy::I16) => {
                        LitKind::Int(val, Some(ast::IntTy::I16))
                    }
                    Some(crate::lexer::token::IntTy::I32) => {
                        LitKind::Int(val, Some(ast::IntTy::I32))
                    }
                    Some(crate::lexer::token::IntTy::I64) => {
                        LitKind::Int(val, Some(ast::IntTy::I64))
                    }
                    Some(crate::lexer::token::IntTy::I128) => {
                        LitKind::Int(val, Some(ast::IntTy::I128))
                    }
                    Some(crate::lexer::token::IntTy::Isize) => {
                        LitKind::Int(val, Some(ast::IntTy::Isize))
                    }
                    Some(crate::lexer::token::IntTy::U8) => {
                        LitKind::Uint(val, Some(ast::UintTy::U8))
                    }
                    Some(crate::lexer::token::IntTy::U16) => {
                        LitKind::Uint(val, Some(ast::UintTy::U16))
                    }
                    Some(crate::lexer::token::IntTy::U32) => {
                        LitKind::Uint(val, Some(ast::UintTy::U32))
                    }
                    Some(crate::lexer::token::IntTy::U64) => {
                        LitKind::Uint(val, Some(ast::UintTy::U64))
                    }
                    Some(crate::lexer::token::IntTy::U128) => {
                        LitKind::Uint(val, Some(ast::UintTy::U128))
                    }
                    Some(crate::lexer::token::IntTy::Usize) => {
                        LitKind::Uint(val, Some(ast::UintTy::Usize))
                    }
                    None => LitKind::Int(val, None),
                };
                Expr::Lit(lit_kind, span)
            }
            TokenKind::FloatLit(val, suffix) => {
                self.bump();
                Expr::Lit(
                    LitKind::Float(val, suffix.map(|_| crate::ast::FloatTy::F64)),
                    span,
                )
            }
            TokenKind::KwTrue => {
                self.bump();
                Expr::Lit(LitKind::Bool(true), span)
            }
            TokenKind::KwFalse => {
                self.bump();
                Expr::Lit(LitKind::Bool(false), span)
            }
            TokenKind::CharLit(c) => {
                self.bump();
                Expr::Lit(LitKind::Char(c), span)
            }
            TokenKind::StrLit(sym) => {
                self.bump();
                Expr::Lit(LitKind::Str(sym), span)
            }
            TokenKind::ByteLit(b) => {
                self.bump();
                Expr::Lit(LitKind::Byte(b), span)
            }
            TokenKind::ByteStrLit(sym) => {
                self.bump();
                Expr::Lit(LitKind::ByteStr(sym), span)
            }
            TokenKind::LParen => {
                self.bump();
                if *self.peek() == TokenKind::RParen {
                    self.bump();
                    return Expr::Unit(span);
                }
                let first = self.parse_expr();
                if *self.peek() == TokenKind::Comma {
                    self.bump();
                    let mut elems = vec![first];
                    while !matches!(self.peek(), TokenKind::RParen | TokenKind::Eof) {
                        elems.push(self.parse_expr());
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RParen, "`)`");
                    return Expr::Tuple { elems, span };
                }
                self.expect(&TokenKind::RParen, "`)`");
                first
            }
            TokenKind::LBracket => {
                self.bump();
                if *self.peek() == TokenKind::RBracket {
                    self.bump();
                    return Expr::Array {
                        elems: Vec::new(),
                        span,
                    };
                }
                let first = self.parse_expr();
                if *self.peek() == TokenKind::Semicolon {
                    self.bump();
                    let count = self.parse_expr();
                    self.expect(&TokenKind::RBracket, "`]`");
                    return Expr::Repeat {
                        elem: Box::new(first),
                        count: Box::new(count),
                        span,
                    };
                }
                let mut elems = vec![first];
                while self.eat(&TokenKind::Comma) {
                    if *self.peek() == TokenKind::RBracket {
                        break;
                    }
                    elems.push(self.parse_expr());
                }
                self.expect(&TokenKind::RBracket, "`]`");
                Expr::Array { elems, span }
            }
            // Note: Block expressions { ... } are NOT parsed here to avoid
            // ambiguity with for/match/while/struct literal blocks.
            // Use explicit blocks via parse_block() in control flow expressions.
            TokenKind::KwIf => self.parse_if_expr(),
            TokenKind::KwMatch => self.parse_match_expr(),
            TokenKind::KwLoop => {
                self.bump();
                let body = self.parse_block();
                Expr::Loop { body, span }
            }
            TokenKind::KwWhile => {
                self.bump();
                // while let Pat = expr block — Stage 13.2 (TD-031): now fully supported
                if *self.peek() == TokenKind::KwLet {
                    self.bump(); // let
                    let pat = self.parse_pat();
                    self.expect(&TokenKind::Eq, "`=`");
                    let prev = self.no_struct_literal;
                    self.no_struct_literal = true;
                    let scrutinee = self.parse_expr();
                    self.no_struct_literal = prev;
                    let body = self.parse_block();
                    Expr::WhileLet {
                        pat,
                        expr: Box::new(scrutinee),
                        body,
                        span,
                    }
                } else {
                    let prev = self.no_struct_literal;
                    self.no_struct_literal = true;
                    let c = self.parse_expr();
                    self.no_struct_literal = prev;
                    let body = self.parse_block();
                    Expr::While {
                        cond: Box::new(c),
                        body,
                        span,
                    }
                }
            }
            TokenKind::KwFor => {
                self.bump();
                let pat = self.parse_pat();
                self.expect(&TokenKind::KwIn, "`in`");
                let prev = self.no_struct_literal;
                self.no_struct_literal = true;
                let iter = self.parse_expr();
                self.no_struct_literal = prev;
                let body = self.parse_block();
                Expr::For {
                    pat,
                    iter: Box::new(iter),
                    body,
                    span,
                }
            }
            TokenKind::KwReturn => {
                self.bump();
                let expr = if self.is_expr_start() {
                    Some(Box::new(self.parse_expr()))
                } else {
                    None
                };
                Expr::Return { expr, span }
            }
            TokenKind::KwBreak => {
                self.bump();
                let expr = if self.is_expr_start() {
                    Some(Box::new(self.parse_expr()))
                } else {
                    None
                };
                Expr::Break { expr, span }
            }
            TokenKind::KwContinue => {
                self.bump();
                Expr::Continue { span }
            }
            TokenKind::KwUnsafe => {
                self.bump();
                let block = self.parse_block();
                Expr::Unsafe(block, span)
            }
            // Stage 8.5: `async { block }` — async block expression.
            TokenKind::KwAsync => {
                self.bump();
                let block = self.parse_block();
                Expr::Async { block, span }
            }
            // Stage 8.5: `await expr` — await expression.
            // Note: in Rust, `await` is a postfix `.await`, but Landin MVP
            // supports `await expr` as a prefix form for simplicity.
            TokenKind::KwAwait => {
                self.bump();
                let inner = self.parse_unary_expr();
                Expr::Await {
                    expr: Box::new(inner),
                    span,
                }
            }
            // Block expression: `{ stmts; trailing_expr }`.
            // We do allow this in expression position now (Round 2e fix);
            // the `no_struct_literal` flag handles the if/while/for/match
            // condition disambiguation separately, so a `{` following a path
            // is still treated as struct literal, but a bare `{` starting an
            // expression is a block.
            TokenKind::LBrace => {
                let block = self.parse_block();
                Expr::Block(block, span)
            }
            TokenKind::Or | TokenKind::OrOr => {
                // Closure: |args| expr or || expr (empty params)
                // Possibly preceded by `move` keyword — handled below in KwMove arm.
                let is_double = *self.peek() == TokenKind::OrOr;
                self.bump(); // | or ||
                let mut params = Vec::new();
                if !is_double {
                    // Single |: parse params until closing |.
                    // IMPORTANT: use parse_pat_no_or (NOT parse_pat) so that
                    // `|` inside a pattern is NOT interpreted as an or-pattern
                    // separator. Without this, `|x| x` gets misparsed as
                    // `|(x | x)|` (or-pattern), consuming the closing `|`.
                    while !matches!(self.peek(), TokenKind::Or | TokenKind::Eof) {
                        let pat = self.parse_pat_no_or();
                        let ty = if *self.peek() == TokenKind::Colon {
                            self.bump();
                            Some(self.parse_ty())
                        } else {
                            None
                        };
                        params.push(Param {
                            pat,
                            ty: ty.unwrap_or(Ty::Infer(span)),
                            attrs: Vec::new(),
                            is_self: false,
                            self_kind: None,
                            span,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(&TokenKind::Or); // closing |
                }
                let body = self.parse_expr();
                Expr::Closure {
                    is_move: false,
                    params,
                    body: Box::new(body),
                    span,
                }
            }
            // move closure: `move |args| expr` or `move || expr`
            TokenKind::KwMove if matches!(self.peek_at(1), TokenKind::Or | TokenKind::OrOr) => {
                self.bump(); // move
                let is_double = *self.peek() == TokenKind::OrOr;
                self.bump(); // | or ||
                let mut params = Vec::new();
                if !is_double {
                    while !matches!(self.peek(), TokenKind::Or | TokenKind::Eof) {
                        let pat = self.parse_pat_no_or();
                        let ty = if *self.peek() == TokenKind::Colon {
                            self.bump();
                            Some(self.parse_ty())
                        } else {
                            None
                        };
                        params.push(Param {
                            pat,
                            ty: ty.unwrap_or(Ty::Infer(span)),
                            attrs: Vec::new(),
                            is_self: false,
                            self_kind: None,
                            span,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(&TokenKind::Or); // closing |
                }
                let body = self.parse_expr();
                Expr::Closure {
                    is_move: true,
                    params,
                    body: Box::new(body),
                    span,
                }
            }
            _ => {
                // Path expression — possibly struct literal `Foo { x: 1, y: 2 }`
                // or macro call `ident!(...)`, or turbofish `Vec::<i32>()`.
                let path = self.parse_path_in_expr();
                let path_span = span;
                // Macro call: `!` followed by `(`/`{`/`[`
                if *self.peek() == TokenKind::Not
                    && matches!(
                        self.peek_at(1),
                        TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket
                    )
                {
                    self.bump(); // !
                    let delim = match self.peek() {
                        TokenKind::LParen => MacroDelim::Paren,
                        TokenKind::LBrace => MacroDelim::Brace,
                        TokenKind::LBracket => MacroDelim::Bracket,
                        _ => unreachable!(),
                    };
                    // Skip the macro body tokens for Stage 0 — we just balance
                    // the delimiters. Stage 4 macro expansion will re-parse them.
                    self.skip_delim_group();
                    return Expr::MacroCall {
                        path,
                        delim,
                        span: path_span,
                    };
                }
                // Struct literal: `Path { field: expr, .. }`
                // Disambiguation: struct literals are NOT allowed in `if`/`while`/`for`/`match`
                // condition positions (those `{` belong to the block). We use a
                // `no_struct_literal` flag controlled by the condition-parsing sites.
                if *self.peek() == TokenKind::LBrace && !self.no_struct_literal {
                    self.bump(); // {
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                        // `..base` (struct update syntax)
                        if *self.peek() == TokenKind::DotDot {
                            self.bump();
                            let _base = self.parse_expr();
                            // For Stage 0 we don't store the base expression in the AST;
                            // the existing Expr::Struct doesn't have a base field.
                            // Stage 1 will extend the AST.
                            break;
                        }
                        let field_ident = self.expect_ident("struct literal field name");
                        let (field_expr, is_shorthand_unused) = if *self.peek() == TokenKind::Colon
                        {
                            self.bump();
                            (self.parse_expr(), false)
                        } else {
                            // Shorthand: `field` means `field: field`
                            let shorthand = Expr::Path(
                                None,
                                Path {
                                    segments: vec![PathSegment {
                                        ident: field_ident,
                                        args: None,
                                    }],
                                    leading: PathLeading::None,
                                    span: self.current_span(),
                                },
                                self.current_span(),
                            );
                            (shorthand, true)
                        };
                        let _ = is_shorthand_unused;
                        let f_span = self.current_span();
                        fields.push(ExprField {
                            ident: field_ident,
                            expr: Some(field_expr),
                            span: f_span,
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace, "`}`");
                    return Expr::Struct {
                        path,
                        fields,
                        span: path_span,
                    };
                }
                Expr::Path(None, path, path_span)
            }
        }
    }

    /// Parse an `if` expression, including `if let` (Stage 13.2 TD-031).
    ///
    /// - `if <cond> { ... } else { ... }` → `Expr::If`
    /// - `if let <pat> = <expr> { ... } else { ... }` → `Expr::IfLet` (Stage 13.2)
    ///
    /// Per §13.4 design alignment + 05-ast.md §12.4: `IfLet` is parsed at AST
    /// level but desugars to `Match` in HIR lowering (Strategy B).
    pub(super) fn parse_if_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.bump(); // if

        // if let Pat = expr block — Stage 13.2 (TD-031): now fully supported
        if *self.peek() == TokenKind::KwLet {
            self.bump(); // let
            let pat = self.parse_pat();
            self.expect(&TokenKind::Eq, "`=`");
            // Set no_struct_literal so the scrutinee `{` doesn't get eaten.
            let prev = self.no_struct_literal;
            self.no_struct_literal = true;
            let scrutinee = self.parse_expr();
            self.no_struct_literal = prev;
            let then = self.parse_block();
            let else_ = if *self.peek() == TokenKind::KwElse {
                self.bump();
                // else can be either { block } or if expr
                if *self.peek() == TokenKind::LBrace {
                    let block = self.parse_block();
                    Some(Box::new(Expr::Block(block, self.current_span())))
                } else {
                    Some(Box::new(self.parse_expr()))
                }
            } else {
                None
            };
            return Expr::IfLet {
                pat,
                expr: Box::new(scrutinee),
                then,
                else_,
                span,
            };
        }

        // Regular if: parse cond with no_struct_literal = true
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let cond = self.parse_expr();
        self.no_struct_literal = prev;
        let then = self.parse_block();
        let else_ = if *self.peek() == TokenKind::KwElse {
            self.bump();
            // else can be either { block } or if expr
            if *self.peek() == TokenKind::LBrace {
                let block = self.parse_block();
                Some(Box::new(Expr::Block(block, self.current_span())))
            } else {
                Some(Box::new(self.parse_expr()))
            }
        } else {
            None
        };
        Expr::If {
            cond: Box::new(cond),
            then,
            else_,
            span,
        }
    }

    pub(super) fn parse_match_expr(&mut self) -> Expr {
        let span = self.current_span();
        self.bump(); // match
                     // Scrutinee: no_struct_literal = true (so `{` belongs to match arms)
        let prev = self.no_struct_literal;
        self.no_struct_literal = true;
        let expr = self.parse_expr();
        self.no_struct_literal = prev;
        self.expect(&TokenKind::LBrace, "`{`");
        let mut arms = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            let pat = self.parse_pat();
            // Match guard: `if expr`
            let guard = if *self.peek() == TokenKind::KwIf {
                self.bump();
                let prev = self.no_struct_literal;
                self.no_struct_literal = true;
                let g = self.parse_expr();
                self.no_struct_literal = prev;
                Some(g)
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow, "`=>`");
            let body = self.parse_expr();
            self.eat(&TokenKind::Comma);
            let arm_span = self.current_span();
            arms.push(Arm {
                pat,
                guard,
                body: Box::new(body),
                span: arm_span,
            });
        }
        self.expect(&TokenKind::RBrace, "`}`");
        Expr::Match {
            expr: Box::new(expr),
            arms,
            span,
        }
    }

    pub(super) fn is_expr_start(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::IntLit(_, _)
                | TokenKind::FloatLit(_, _)
                | TokenKind::KwTrue
                | TokenKind::KwFalse
                | TokenKind::CharLit(_)
                | TokenKind::StrLit(_)
                | TokenKind::ByteLit(_)
                | TokenKind::ByteStrLit(_)
                | TokenKind::RawStrLit(_, _)
                | TokenKind::Ident(_)
                | TokenKind::RawIdent(_)
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::KwIf
                | TokenKind::KwMatch
                | TokenKind::KwLoop
                | TokenKind::KwWhile
                | TokenKind::KwFor
                | TokenKind::KwReturn
                | TokenKind::KwBreak
                | TokenKind::KwContinue
                | TokenKind::KwUnsafe
                | TokenKind::Minus
                | TokenKind::Not
                | TokenKind::Star
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::OrOr
                | TokenKind::PathSep
                | TokenKind::KwSelf_
                | TokenKind::KwSelfType
                | TokenKind::KwCrate
                | TokenKind::KwSuper
                | TokenKind::KwMove // for `move ||` closure
                | TokenKind::KwAsync // Stage 8.5: `async { block }`
                | TokenKind::KwAwait // Stage 8.5: `await expr`
        )
    }
}

pub(super) trait ExprSpan {
    fn span(&self) -> Span;
}

/// Helper trait for getting span from expressions.
impl ExprSpan for Expr {
    fn span(&self) -> Span {
        match self {
            Expr::Lit(_, s) => *s,
            Expr::Path(_, _, s) => *s,
            Expr::Block(_, s) => *s,
            Expr::Call { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
            Expr::Field { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
            Expr::AddrOf { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::Try { span, .. } => *span,
            Expr::If { span, .. } => *span,
            Expr::IfLet { span, .. } => *span,
            Expr::Match { span, .. } => *span,
            Expr::Loop { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::WhileLet { span, .. } => *span,
            Expr::For { span, .. } => *span,
            Expr::Closure { span, .. } => *span,
            Expr::Return { span, .. } => *span,
            Expr::Break { span, .. } => *span,
            Expr::Continue { span } => *span,
            Expr::Range { span, .. } => *span,
            Expr::Tuple { span, .. } => *span,
            Expr::Array { span, .. } => *span,
            Expr::Repeat { span, .. } => *span,
            Expr::Struct { span, .. } => *span,
            Expr::MacroCall { span, .. } => *span,
            Expr::Unsafe(_, s) => *s,
            Expr::Unit(s) => *s,
            Expr::Await { span, .. } => *span,
            Expr::Async { span, .. } => *span,
        }
    }
}
