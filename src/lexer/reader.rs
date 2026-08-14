//! Lexer reader: character-by-character tokenization.
//!
//! Based on 02-grammar.md §1 (lexical structure).
//!
//! ## Stage 6.13 architectural split (TD-023)
//!
//! Per `docs/stage-committee-process.md` v3.21 §14.4 (refactoring as
//! architecture design) and §13.4 (stage-start design alignment with
//! 02-grammar.md §1.1-§1.9), this file has been split into 4 sub-modules:
//!
//! - `ident.rs`     — identifier + raw identifier + keyword recognition (§1.3+§1.4)
//! - `number.rs`    — numeric literals: dec/hex/oct/bin + suffix (§1.5+§1.6)
//! - `string.rs`    — char + string + byte + raw variants + escape (§1.7)
//! - `operators.rs` — doc comment + operators + punctuation (§1.1+§1.8)
//!
//! This file (`reader.rs`) retains: Lexer struct + cursor methods +
//! `skip_trivia` + `next_token` entry point + LexError.
//!
//! All sub-modules add methods to `impl<'a> Lexer<'a>` via their own
//! `impl` blocks. Cursor methods are `pub(super)` so sibling modules
//! can call them. Per §16, lexer-external code only sees `next_token`.

use crate::lexer::token::*;
use crate::session::{BytePos, Span};
use lasso::Rodeo;

// Stage 6.13: import ident helpers from the `ident` sub-module.
use super::ident::is_ident_start_byte;

/// Lexing error.
#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

// Stage 3.64 (P2 fix): implement `Display` + `std::error::Error` for `LexError`
// so it integrates with the standard Rust error-handling ecosystem
// (`?` propagation, `anyhow::Error`, `Box<dyn Error>`, etc.). Previously
// only carried a `message: String + span: Span` shape with no trait impls.

// Stage 15.16: implement `Spanned` for uniform span access.
impl crate::diagnostics::Spanned for LexError {
    fn span(&self) -> Span {
        self.span
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[lex error at {}] {}", self.span, self.message)
    }
}

impl std::error::Error for LexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// The lexer: converts source text to tokens.
pub struct Lexer<'a> {
    pub(super) src: &'a str,
    pub(super) bytes: &'a [u8],
    pub(super) pos: BytePos,
    /// String interner for identifiers and string literals.
    pub(super) interner: &'a mut Rodeo,
    /// Collected errors (non-fatal: lexer continues after error).
    pub(super) errors: Vec<LexError>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, interner: &'a mut Rodeo) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            interner,
            errors: Vec::new(),
        }
    }

    pub fn into_errors(self) -> Vec<LexError> {
        self.errors
    }

    /// Check if we've consumed all input.
    pub fn is_at_end(&self) -> bool {
        self.pos as usize >= self.bytes.len()
    }

    /// Peek at the current byte without consuming.
    pub(super) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos as usize).copied()
    }

    /// Peek at the byte at offset n from current position.
    pub(super) fn peek_at(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.pos as usize + n).copied()
    }

    /// Advance by one byte and return the consumed byte.
    pub(super) fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    /// Current span starting at `start` and ending at current position.
    pub(super) fn span_from(&self, start: BytePos) -> Span {
        Span::new(start, self.pos)
    }

    /// Skip whitespace and comments.
    ///
    /// Stops at the first character that should begin a real token, INCLUDING
    /// doc comments (`///` and `//!`). Doc comments are tokenized as
    /// `TokenKind::DocComment` so that the parser/attribute system can attach
    /// them to items. Per 02-grammar.md §1.12, `////` and `//!/` are regular
    /// line comments (not doc comments) because the 4th byte is `/`.
    pub(super) fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => {
                    self.bump();
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    // Check whether this is a doc comment.
                    // `///` and `//!` are doc comments only if NOT followed by
                    // another `/` (so `////` and `//!/` are regular comments).
                    let third = self.peek_at(2);
                    let fourth = self.peek_at(3);
                    let is_doc = matches!(third, Some(b'/') | Some(b'!')) && fourth != Some(b'/');
                    if is_doc {
                        // Stop trivia so next_token can dispatch to lex_doc_comment.
                        break;
                    }
                    // Regular line comment: skip to end of line.
                    while let Some(b) = self.peek() {
                        if b == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    // Block comment (nestable)
                    self.bump(); // /
                    self.bump(); // *
                    let mut depth = 1;
                    while depth > 0 {
                        match self.peek() {
                            None => {
                                self.errors.push(LexError {
                                    message: "unterminated block comment".into(),
                                    span: self.span_from(self.pos),
                                });
                                return;
                            }
                            Some(b'/') if self.peek_at(1) == Some(b'*') => {
                                self.bump();
                                self.bump();
                                depth += 1;
                            }
                            Some(b'*') if self.peek_at(1) == Some(b'/') => {
                                self.bump();
                                self.bump();
                                depth -= 1;
                            }
                            _ => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => break,
            }
        }
    }

    /// Tokenize the next token. Returns `Eof` at end.
    pub fn next_token(&mut self) -> Token {
        self.skip_trivia();

        let start = self.pos;
        let b = match self.peek() {
            Some(b) => b,
            None => {
                return Token {
                    kind: TokenKind::Eof,
                    span: Span::new(start, start),
                }
            }
        };

        // Dispatch based on first byte
        // IMPORTANT: byte/raw string/raw ident dispatch MUST come before generic ident
        match b {
            b'0'..=b'9' => self.lex_number(start),
            b'"' => self.lex_string(start),
            b'\'' => self.lex_char_or_lifetime(start),
            // Doc comment: `///` (outer) or `//!` (inner).
            // Detected here (after skip_trivia stopped on it) rather than inside
            // skip_trivia because we want a real Token, not trivia.
            b'/' if self.peek_at(1) == Some(b'/')
                && matches!(self.peek_at(2), Some(b'/') | Some(b'!'))
                && self.peek_at(3) != Some(b'/') =>
            {
                self.lex_doc_comment(start)
            }
            // Byte string: b"..."
            b'b' if self.peek_at(1) == Some(b'"') => self.lex_byte_string(start),
            // Byte literal: b'...'
            b'b' if self.peek_at(1) == Some(b'\'') => self.lex_byte(start),
            // Raw byte string: br"..." or br#"..."# (NOT br + identifier like "break")
            b'b' if self.peek_at(1) == Some(b'r')
                && matches!(self.peek_at(2), Some(b'"') | Some(b'#')) =>
            {
                self.lex_raw_byte_string(start)
            }
            // Raw string: r"..."
            b'r' if self.peek_at(1) == Some(b'"') => self.lex_raw_string(start, 0),
            // Raw identifier: r#name (r# followed by an identifier-start character)
            // MUST come before raw string with hashes (r#"...")
            b'r' if self.peek_at(1) == Some(b'#') && is_ident_start_byte(self.peek_at(2)) => {
                self.lex_raw_identifier(start)
            }
            // Raw string with hashes: r#"..."#
            b'r' if self.peek_at(1) == Some(b'#') => self.lex_raw_string_hash(start),
            // Identifiers (including b and r when not followed by " or # or ')
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_ident(start),
            b'.' => self.lex_dot(start),
            b'<' => self.lex_lt(start),
            b'>' => self.lex_gt(start),
            b'=' => self.lex_eq(start),
            b'!' => self.lex_bang(start),
            b'+' => self.lex_plus(start),
            b'-' => self.lex_minus(start),
            b'*' => self.lex_star(start),
            b'/' => self.lex_slash(start),
            b'%' => self.lex_percent(start),
            b'&' => self.lex_and(start),
            b'|' => self.lex_or(start),
            b'^' => self.lex_caret(start),
            b':' => self.lex_colon(start),
            b'(' => {
                self.bump();
                Token {
                    kind: TokenKind::LParen,
                    span: self.span_from(start),
                }
            }
            b')' => {
                self.bump();
                Token {
                    kind: TokenKind::RParen,
                    span: self.span_from(start),
                }
            }
            b'{' => {
                self.bump();
                Token {
                    kind: TokenKind::LBrace,
                    span: self.span_from(start),
                }
            }
            b'}' => {
                self.bump();
                Token {
                    kind: TokenKind::RBrace,
                    span: self.span_from(start),
                }
            }
            b'[' => {
                self.bump();
                Token {
                    kind: TokenKind::LBracket,
                    span: self.span_from(start),
                }
            }
            b']' => {
                self.bump();
                Token {
                    kind: TokenKind::RBracket,
                    span: self.span_from(start),
                }
            }
            b',' => {
                self.bump();
                Token {
                    kind: TokenKind::Comma,
                    span: self.span_from(start),
                }
            }
            b';' => {
                self.bump();
                Token {
                    kind: TokenKind::Semicolon,
                    span: self.span_from(start),
                }
            }
            b'#' => {
                self.bump();
                Token {
                    kind: TokenKind::Hash,
                    span: self.span_from(start),
                }
            }
            b'@' => {
                self.bump();
                Token {
                    kind: TokenKind::At,
                    span: self.span_from(start),
                }
            }
            // Stage 18.02: `$` for macro_rules! patterns.
            b'$' => {
                self.bump();
                Token {
                    kind: TokenKind::Dollar,
                    span: self.span_from(start),
                }
            }
            b'?' => {
                self.bump();
                Token {
                    kind: TokenKind::Question,
                    span: self.span_from(start),
                }
            }
            _ => {
                // Try to handle UTF-8 identifiers
                let rest = &self.src[self.pos as usize..];
                if let Some(c) = rest.chars().next() {
                    if unicode_xid::UnicodeXID::is_xid_start(c) {
                        return self.lex_ident(start);
                    }
                    // Unknown character: report error and SKIP (don't recurse)
                    self.errors.push(LexError {
                        // Stage 18.76 P1-D: Use Display instead of Debug format.
                        // Per §1.0 原則 3 "显式 > 隐式": user-facing messages
                        // should show the character, not its Debug representation.
                        message: format!("unexpected character: {}", c),
                        span: self.span_from(start),
                    });
                    self.pos += c.len_utf8() as u32;
                    // Return a special error marker token to avoid recursion
                    // The outer tokenize loop will call next_token again
                    return Token {
                        kind: TokenKind::Eof,
                        span: self.span_from(start),
                    };
                }
                self.errors.push(LexError {
                    message: format!("unexpected byte: 0x{b:02x}"),
                    span: self.span_from(start),
                });
                self.bump();
                // Return Eof to let outer loop continue (avoid recursion)
                Token {
                    kind: TokenKind::Eof,
                    span: self.span_from(start),
                }
            }
        }
    }
}
