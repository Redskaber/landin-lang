//! Stage 6.13 (TD-023): Numeric literal lexing.
//!
//! Per 02-grammar.md §1.5 (Integer literal) + §1.6 (Float literal).
//! Extracted from `reader.rs` per `docs/stage-committee-process.md` v3.21
//! §14.4 + §13.4.
//!
//! Owns:
//! - `lex_number` (decimal integer/float dispatcher)
//! - `lex_hex` / `lex_oct` / `lex_bin` (hex/octal/binary integers)
//! - `try_lex_number_suffix` (parse `i32`/`u64`/`f32`/... suffix)

use crate::lexer::token::*;
use crate::session::{BytePos, Span};

use super::reader::{LexError, Lexer};

impl<'a> Lexer<'a> {
    /// Stage 14.102 (Phase 1 audit fix): Parse an integer suffix and emit
    /// a LexError for invalid suffixes (e.g., `0xFF_i33`).
    ///
    /// **Before**: `lex_hex`/`lex_oct`/`lex_bin` used `and_then` with
    /// `_ => None`, silently swallowing invalid suffixes.
    ///
    /// **After**: This helper emits a proper LexError for invalid suffixes,
    /// matching the decimal path's behavior.
    fn parse_int_suffix_with_error(&mut self, suffix_start: BytePos) -> Option<IntTy> {
        self.try_lex_number_suffix().and_then(|s| match s.as_str() {
            "i8" => Some(IntTy::I8),
            "i16" => Some(IntTy::I16),
            "i32" => Some(IntTy::I32),
            "i64" => Some(IntTy::I64),
            "i128" => Some(IntTy::I128),
            "isize" => Some(IntTy::Isize),
            "u8" => Some(IntTy::U8),
            "u16" => Some(IntTy::U16),
            "u32" => Some(IntTy::U32),
            "u64" => Some(IntTy::U64),
            "u128" => Some(IntTy::U128),
            "usize" => Some(IntTy::Usize),
            _ => {
                self.errors.push(LexError {
                    message: format!("invalid integer suffix: {s}"),
                    span: Span::new(suffix_start, self.pos),
                });
                None
            }
        })
    }

    /// Lex a number (integer or float).
    pub(super) fn lex_number(&mut self, start: BytePos) -> Token {
        // Check for hex/oct/bin prefix
        if self.peek() == Some(b'0') {
            match self.peek_at(1) {
                Some(b'x') | Some(b'X') => return self.lex_hex(start),
                Some(b'o') | Some(b'O') => return self.lex_oct(start),
                Some(b'b') | Some(b'B') => return self.lex_bin(start),
                _ => {}
            }
        }

        // Decimal integer or float
        // First: check for leading zero (invalid per 02-grammar.md §1.5)
        if self.peek() == Some(b'0') && self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
            self.errors.push(LexError {
                message: "leading zeros not allowed in decimal integer".into(),
                span: Span::new(start, start + 1),
            });
        }

        // Consume integer part
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }

        // Check for float: . followed by digit, or e/E
        let mut is_float = false;
        if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
            is_float = true;
            self.bump(); // .
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() || b == b'_' {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        // Exponent
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() || b == b'_' {
                    self.bump();
                } else {
                    break;
                }
            }
        }

        let text = &self.src[start as usize..self.pos as usize];
        let span = self.span_from(start);

        // Check for type suffix
        let suffix_start = self.pos;
        let suffix = self.try_lex_number_suffix();

        // RP0-1 fix: if suffix is f32/f64 on an integer, treat as float
        if !is_float {
            if let Some(ref s) = suffix {
                if s == "f32" || s == "f64" {
                    is_float = true;
                }
            }
        }

        if is_float {
            let cleaned: String = text.chars().filter(|c| *c != '_').collect();
            let val: f64 = cleaned.parse().unwrap_or(f64::NAN);
            Token {
                kind: TokenKind::FloatLit(
                    val,
                    suffix.map(|s| match s.as_str() {
                        "f32" => FloatTy::F32,
                        "f64" => FloatTy::F64,
                        _ => {
                            self.errors.push(LexError {
                                message: format!("invalid float suffix: {s}"),
                                span: Span::new(suffix_start, self.pos),
                            });
                            FloatTy::F64
                        }
                    }),
                ),
                span,
            }
        } else {
            let cleaned: String = text.chars().filter(|c| *c != '_').collect();
            let int_ty = suffix.and_then(|s| match s.as_str() {
                "i8" => Some(IntTy::I8),
                "i16" => Some(IntTy::I16),
                "i32" => Some(IntTy::I32),
                "i64" => Some(IntTy::I64),
                "i128" => Some(IntTy::I128),
                "isize" => Some(IntTy::Isize),
                "u8" => Some(IntTy::U8),
                "u16" => Some(IntTy::U16),
                "u32" => Some(IntTy::U32),
                "u64" => Some(IntTy::U64),
                "u128" => Some(IntTy::U128),
                "usize" => Some(IntTy::Usize),
                _ => {
                    self.errors.push(LexError {
                        message: format!("invalid integer suffix: {s}"),
                        span: Span::new(suffix_start, self.pos),
                    });
                    None
                }
            });
            // Parse the integer value
            let val = cleaned.parse::<u128>().unwrap_or(u128::MAX);
            Token {
                kind: TokenKind::IntLit(val, int_ty),
                span,
            }
        }
    }

    pub(super) fn lex_hex(&mut self, start: BytePos) -> Token {
        self.bump(); // 0
        self.bump(); // x
        let digit_start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_hexdigit() || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        // RP0-4 fix: error on empty hex literal
        if self.pos == digit_start {
            self.errors.push(LexError {
                message: "hexadecimal literal has no digits".into(),
                span: Span::new(start, self.pos),
            });
            return Token {
                kind: TokenKind::IntLit(0, None),
                span: self.span_from(start),
            };
        }
        let text = &self.src[start as usize + 2..self.pos as usize];
        let cleaned: String = text.chars().filter(|c| *c != '_').collect();
        let val = u128::from_str_radix(&cleaned, 16).unwrap_or(u128::MAX);
        let span = self.span_from(start);
        let suffix_start = self.pos;
        let suffix = self.parse_int_suffix_with_error(suffix_start);
        Token {
            kind: TokenKind::IntLit(val, suffix),
            span,
        }
    }

    pub(super) fn lex_oct(&mut self, start: BytePos) -> Token {
        self.bump(); // 0
        self.bump(); // o
        let digit_start = self.pos;
        while let Some(b) = self.peek() {
            if (b'0'..=b'7').contains(&b) || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        // RP0-4 fix: error on empty octal literal
        if self.pos == digit_start {
            self.errors.push(LexError {
                message: "octal literal has no digits".into(),
                span: Span::new(start, self.pos),
            });
            return Token {
                kind: TokenKind::IntLit(0, None),
                span: self.span_from(start),
            };
        }
        let text = &self.src[start as usize + 2..self.pos as usize];
        let cleaned: String = text.chars().filter(|c| *c != '_').collect();
        let val = u128::from_str_radix(&cleaned, 8).unwrap_or(u128::MAX);
        let span = self.span_from(start);
        let suffix_start = self.pos;
        let suffix = self.parse_int_suffix_with_error(suffix_start);
        Token {
            kind: TokenKind::IntLit(val, suffix),
            span,
        }
    }

    pub(super) fn lex_bin(&mut self, start: BytePos) -> Token {
        self.bump(); // 0
        self.bump(); // b
        let digit_start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'0' || b == b'1' || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        // RP0-4 fix: error on empty binary literal
        if self.pos == digit_start {
            self.errors.push(LexError {
                message: "binary literal has no digits".into(),
                span: Span::new(start, self.pos),
            });
            return Token {
                kind: TokenKind::IntLit(0, None),
                span: self.span_from(start),
            };
        }
        let text = &self.src[start as usize + 2..self.pos as usize];
        let cleaned: String = text.chars().filter(|c| *c != '_').collect();
        let val = u128::from_str_radix(&cleaned, 2).unwrap_or(u128::MAX);
        let span = self.span_from(start);
        let suffix_start = self.pos;
        let suffix = self.parse_int_suffix_with_error(suffix_start);
        Token {
            kind: TokenKind::IntLit(val, suffix),
            span,
        }
    }

    /// Try to lex a number type suffix (e.g., "i32", "u64", "f64").
    pub(super) fn try_lex_number_suffix(&mut self) -> Option<String> {
        let start = self.pos;
        // Suffix must start with a letter
        match self.peek() {
            Some(b) if b.is_ascii_alphabetic() => {}
            _ => return None,
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() {
                self.bump();
            } else {
                break;
            }
        }
        Some(self.src[start as usize..self.pos as usize].to_string())
    }
}
