//! Stage 6.12 (TD-022): Statement parsing sub-module.
//!
//! Per 02-grammar.md §3.6 (Statement). Extracted from `parser.rs`
//! per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `parse_block` (`{ stmt* }`)
//! - `parse_let` (`let pat (: ty)? = expr;`)

use crate::ast::*;
use crate::lexer::token::*;

use super::parser::Parser;

impl<'a> Parser<'a> {
    // --- Blocks and statements ---

    pub(super) fn parse_block(&mut self) -> Block {
        let span = self.current_span();
        self.expect(&TokenKind::LBrace, "`{`");
        let mut stmts = Vec::new();
        let mut trailing_expr = None;

        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            // Progress guard: snapshot the position before attempting to parse
            // a statement. If `parse_expr` returns without consuming any token
            // (which happens for unsupported syntax like `Point { x: 1 }` when
            // struct literals aren't yet supported, or `if let` patterns that
            // fall through to `parse_path` returning an empty path), we MUST
            // force-advance to avoid an infinite loop / OOM kill.
            let pos_before = self.pos;

            // Check for let statement
            if *self.peek() == TokenKind::KwLet {
                let stmt = self.parse_let();
                stmts.push(Stmt::Local(stmt));
                continue;
            }

            // Parse expression
            let expr = self.parse_expr();

            // Progress guard: if no token was consumed, force-advance and emit
            // an error so the user gets a useful message instead of a hang.
            if self.pos == pos_before {
                let bad_span = self.current_span();
                self.errors.push(crate::parser::ParseError::new(
                    format!(
                        "could not parse expression starting at `{}` (parser made no progress — \
                         this syntax may be unsupported in Stage 0)",
                        self.peek(),
                    ),
                    bad_span,
                ));
                self.bump();
                // Skip the bad token and continue; do not push a stmt.
                continue;
            }

            let has_semi = self.eat(&TokenKind::Semicolon);

            if !has_semi && *self.peek() == TokenKind::RBrace {
                // Trailing expression
                trailing_expr = Some(Box::new(expr));
                break;
            }

            stmts.push(Stmt::Expr(expr, has_semi));
        }

        self.expect(&TokenKind::RBrace, "`}`");
        Block {
            stmts,
            expr: trailing_expr,
            span,
        }
    }

    pub(super) fn parse_let(&mut self) -> LocalDecl {
        let span = self.current_span();
        self.bump(); // let
        let pat = self.parse_pat();
        let ty = if *self.peek() == TokenKind::Colon {
            self.bump();
            Some(self.parse_ty())
        } else {
            None
        };
        let init = if *self.peek() == TokenKind::Eq {
            self.bump();
            Some(self.parse_expr())
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon, "`;`");
        LocalDecl {
            pat,
            ty,
            init,
            span,
        }
    }
}
