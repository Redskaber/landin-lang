//! Stage 6.13 (TD-023): Doc comment + operators + punctuation lexing.
//!
//! Per 02-grammar.md §1.1 (Comment) + §1.8 (Operator + Punctuation).
//! Extracted from `reader.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns 15 functions:
//! - `lex_doc_comment` (`///` outer / `//!` inner)
//! - 14 single/multi-char operators: `lex_dot` / `lex_lt` / `lex_gt` /
//!   `lex_eq` / `lex_bang` / `lex_plus` / `lex_minus` / `lex_star` /
//!   `lex_slash` / `lex_percent` / `lex_and` / `lex_or` / `lex_caret` /
//!   `lex_colon`

use crate::lexer::token::*;
use crate::session::BytePos;

use super::reader::Lexer;

impl<'a> Lexer<'a> {
    /// Lex a doc comment: `/// text` (outer) or `//! text` (inner).
    ///
    /// Per 02-grammar.md §1.12: produces a `TokenKind::DocComment(symbol, is_inner)`
    /// where `is_inner` is `true` for `//!` and `false` for `///`.
    ///
    /// The symbol contains the comment body (text after `/// ` or `//! ` with
    /// leading horizontal whitespace stripped). The trailing newline is NOT
    /// included. Block doc comments (`/** ... */` and `/*! ... */`) are
    /// out of scope for Stage 0 and will be added in Stage 1 (attribute system).
    ///
    /// Pre-condition: the lexer is positioned at the first `/` of `///` or `//!`,
    /// and the 4th byte is NOT `/` (so this is a real doc comment, not `////`).
    pub(super) fn lex_doc_comment(&mut self, start: BytePos) -> Token {
        // Consume the `//` prefix.
        self.bump(); // /
        self.bump(); // /
                     // Determine whether this is an inner doc comment.
        let is_inner = match self.peek() {
            Some(b'!') => {
                self.bump();
                true
            }
            Some(b'/') => {
                self.bump();
                false
            }
            _ => unreachable!(
                "lex_doc_comment called without /// or //! prefix (dispatch invariant)"
            ),
        };
        // Skip a single leading space (the conventional `/// text` form).
        // Additional leading whitespace is preserved in the symbol so that
        // indentation-sensitive tools (rustdoc-style) can see it.
        if self.peek() == Some(b' ') {
            self.bump();
        }
        // Read the comment body until end of line (NOT including the newline).
        let body_start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' {
                break;
            }
            self.bump();
        }
        let body = &self.src[body_start as usize..self.pos as usize];
        let sym = self.interner.get_or_intern(body);
        Token {
            kind: TokenKind::DocComment(sym, is_inner),
            span: self.span_from(start),
        }
    }

    // --- Multi-char operators ---

    pub(super) fn lex_dot(&mut self, start: BytePos) -> Token {
        self.bump(); // .
        if self.peek() == Some(b'.') {
            self.bump();
            if self.peek() == Some(b'=') {
                self.bump();
                Token {
                    kind: TokenKind::DotDotEq,
                    span: self.span_from(start),
                }
            } else {
                Token {
                    kind: TokenKind::DotDot,
                    span: self.span_from(start),
                }
            }
        } else {
            Token {
                kind: TokenKind::Dot,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_lt(&mut self, start: BytePos) -> Token {
        self.bump(); // <
        match self.peek() {
            Some(b'=') => {
                self.bump();
                Token {
                    kind: TokenKind::LtEq,
                    span: self.span_from(start),
                }
            }
            Some(b'<') => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token {
                        kind: TokenKind::ShlEq,
                        span: self.span_from(start),
                    }
                } else {
                    Token {
                        kind: TokenKind::Shl,
                        span: self.span_from(start),
                    }
                }
            }
            _ => Token {
                kind: TokenKind::Lt,
                span: self.span_from(start),
            },
        }
    }

    pub(super) fn lex_gt(&mut self, start: BytePos) -> Token {
        self.bump(); // >
        match self.peek() {
            Some(b'=') => {
                self.bump();
                Token {
                    kind: TokenKind::GtEq,
                    span: self.span_from(start),
                }
            }
            Some(b'>') => {
                self.bump();
                if self.peek() == Some(b'=') {
                    self.bump();
                    Token {
                        kind: TokenKind::ShrEq,
                        span: self.span_from(start),
                    }
                } else {
                    Token {
                        kind: TokenKind::Shr,
                        span: self.span_from(start),
                    }
                }
            }
            _ => Token {
                kind: TokenKind::Gt,
                span: self.span_from(start),
            },
        }
    }

    pub(super) fn lex_eq(&mut self, start: BytePos) -> Token {
        self.bump(); // =
        match self.peek() {
            Some(b'=') => {
                self.bump();
                Token {
                    kind: TokenKind::EqEq,
                    span: self.span_from(start),
                }
            }
            Some(b'>') => {
                self.bump();
                Token {
                    kind: TokenKind::FatArrow,
                    span: self.span_from(start),
                }
            }
            _ => Token {
                kind: TokenKind::Eq,
                span: self.span_from(start),
            },
        }
    }

    pub(super) fn lex_bang(&mut self, start: BytePos) -> Token {
        self.bump(); // !
        if self.peek() == Some(b'=') {
            self.bump();
            Token {
                kind: TokenKind::NotEq,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Not,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_plus(&mut self, start: BytePos) -> Token {
        self.bump();
        if self.peek() == Some(b'=') {
            self.bump();
            Token {
                kind: TokenKind::PlusEq,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Plus,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_minus(&mut self, start: BytePos) -> Token {
        self.bump();
        match self.peek() {
            Some(b'=') => {
                self.bump();
                Token {
                    kind: TokenKind::MinusEq,
                    span: self.span_from(start),
                }
            }
            Some(b'>') => {
                self.bump();
                Token {
                    kind: TokenKind::Arrow,
                    span: self.span_from(start),
                }
            }
            _ => Token {
                kind: TokenKind::Minus,
                span: self.span_from(start),
            },
        }
    }

    pub(super) fn lex_star(&mut self, start: BytePos) -> Token {
        self.bump();
        if self.peek() == Some(b'=') {
            self.bump();
            Token {
                kind: TokenKind::StarEq,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Star,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_slash(&mut self, start: BytePos) -> Token {
        self.bump();
        if self.peek() == Some(b'=') {
            self.bump();
            Token {
                kind: TokenKind::SlashEq,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Slash,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_percent(&mut self, start: BytePos) -> Token {
        self.bump();
        if self.peek() == Some(b'=') {
            self.bump();
            Token {
                kind: TokenKind::PercentEq,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Percent,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_and(&mut self, start: BytePos) -> Token {
        self.bump();
        match self.peek() {
            Some(b'&') => {
                self.bump();
                Token {
                    kind: TokenKind::AndAnd,
                    span: self.span_from(start),
                }
            }
            Some(b'=') => {
                self.bump();
                Token {
                    kind: TokenKind::AndEq,
                    span: self.span_from(start),
                }
            }
            _ => Token {
                kind: TokenKind::And,
                span: self.span_from(start),
            },
        }
    }

    pub(super) fn lex_or(&mut self, start: BytePos) -> Token {
        self.bump();
        match self.peek() {
            Some(b'|') => {
                self.bump();
                Token {
                    kind: TokenKind::OrOr,
                    span: self.span_from(start),
                }
            }
            Some(b'=') => {
                self.bump();
                Token {
                    kind: TokenKind::OrEq,
                    span: self.span_from(start),
                }
            }
            _ => Token {
                kind: TokenKind::Or,
                span: self.span_from(start),
            },
        }
    }

    pub(super) fn lex_caret(&mut self, start: BytePos) -> Token {
        self.bump();
        if self.peek() == Some(b'=') {
            self.bump();
            Token {
                kind: TokenKind::CaretEq,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Caret,
                span: self.span_from(start),
            }
        }
    }

    pub(super) fn lex_colon(&mut self, start: BytePos) -> Token {
        self.bump();
        if self.peek() == Some(b':') {
            self.bump();
            Token {
                kind: TokenKind::PathSep,
                span: self.span_from(start),
            }
        } else {
            Token {
                kind: TokenKind::Colon,
                span: self.span_from(start),
            }
        }
    }
}
