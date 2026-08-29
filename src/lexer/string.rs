//! Stage 6.13 (TD-023): Char + string + byte + raw variants + escape lexing.
//!
//! Per 02-grammar.md §1.7 (Char + string). Extracted from `reader.rs`
//! per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns 10 functions:
//! - `lex_string` / `lex_raw_string` / `lex_raw_string_hash`
//! - `lex_byte_string` / `lex_byte` / `lex_byte_escape` / `lex_raw_byte_string`
//! - `lex_char_or_lifetime`
//! - `lex_escape` / `lex_escape_from_str`

use crate::lexer::token::*;
use crate::lexer::LexErrorKind;
use crate::session::{BytePos, Span};
use lasso::Spur;

use super::reader::{LexError, Lexer};

impl<'a> Lexer<'a> {
    /// Lex a string literal: "..."
    pub(super) fn lex_string(&mut self, start: BytePos) -> Token {
        self.bump(); // opening "
        let mut buf = String::new();
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated string literal".into(),
                        span: self.span_from(start),
                        kind: LexErrorKind::Generic,
                    });
                    break;
                }
                Some(b'"') => {
                    self.bump();
                    break;
                }
                Some(b'\\') => {
                    self.bump();
                    if let Some(c) = self.lex_escape() {
                        buf.push(c);
                    }
                }
                Some(_) => {
                    // Read full UTF-8 char
                    let rest = &self.src[self.pos as usize..];
                    // Guarded by `Some(_)` arm: rest has at least one char.
                    let c = rest.chars().next().expect("Some(_) arm => rest non-empty");
                    buf.push(c);
                    self.pos += c.len_utf8() as u32;
                }
            }
        }
        let sym = self.interner.get_or_intern(buf);
        Token {
            kind: TokenKind::StrLit(sym),
            span: self.span_from(start),
        }
    }

    /// Lex a raw string: r"..." or r#"..."#
    pub(super) fn lex_raw_string(&mut self, start: BytePos, _hashes: usize) -> Token {
        self.bump(); // r
        self.bump(); // "
        let buf_start = self.pos;
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated raw string".into(),
                        span: self.span_from(start),
                        kind: LexErrorKind::Generic,
                    });
                    break;
                }
                Some(b'"') => {
                    self.bump();
                    break;
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
        // Content is between buf_start and pos-1 (excluding closing ").
        // Guard against the unterminated-string case where we broke out of
        // the loop without consuming the closing quote — in that case
        // `self.pos` may equal `buf_start`, and slicing `[buf_start..pos-1]`
        // would panic with "byte range starts at X but ends at X-1".
        let content_end = self.pos.saturating_sub(1).max(buf_start);
        let content = &self.src[buf_start as usize..content_end as usize];
        let sym = self.interner.get_or_intern(content);
        Token {
            kind: TokenKind::RawStrLit(sym, 0),
            span: self.span_from(start),
        }
    }

    /// Lex a raw string with hashes: r#"..."#
    pub(super) fn lex_raw_string_hash(&mut self, start: BytePos) -> Token {
        self.bump(); // r
                     // Count hashes
        let mut hash_count = 0;
        while self.peek() == Some(b'#') {
            self.bump();
            hash_count += 1;
        }
        if self.peek() != Some(b'"') {
            self.errors.push(LexError {
                message: "expected `\"` after `r#...`".into(),
                span: self.span_from(start),
                kind: LexErrorKind::Generic,
            });
            return Token {
                kind: TokenKind::Eof,
                span: self.span_from(start),
            };
        }
        self.bump(); // "
        let buf_start = self.pos;
        // Find closing "#*hash_count
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated raw string".into(),
                        span: self.span_from(start),
                        kind: LexErrorKind::Generic,
                    });
                    break;
                }
                Some(b'"') => {
                    let close_start = self.pos;
                    self.bump();
                    let mut found = 0;
                    while found < hash_count && self.peek() == Some(b'#') {
                        self.bump();
                        found += 1;
                    }
                    if found == hash_count {
                        let content = &self.src[buf_start as usize..close_start as usize];
                        let sym = self.interner.get_or_intern(content);
                        return Token {
                            kind: TokenKind::RawStrLit(sym, hash_count),
                            span: self.span_from(start),
                        };
                    }
                    // Not enough hashes, continue
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
        Token {
            kind: TokenKind::Eof,
            span: self.span_from(start),
        }
    }

    /// Lex a byte string: b"..."
    pub(super) fn lex_byte_string(&mut self, start: BytePos) -> Token {
        self.bump(); // b
        self.bump(); // "
        let mut buf = Vec::new();
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated byte string".into(),
                        span: self.span_from(start),
                        kind: LexErrorKind::Generic,
                    });
                    break;
                }
                Some(b'"') => {
                    self.bump();
                    break;
                }
                Some(b'\\') => {
                    self.bump();
                    if let Some(c) = self.lex_escape() {
                        buf.push(c as u8);
                    }
                }
                Some(b) if b.is_ascii() => {
                    buf.push(b);
                    self.bump();
                }
                Some(_) => {
                    self.errors.push(LexError {
                        message: "non-ASCII byte in byte string".into(),
                        span: Span::new(self.pos, self.pos + 1),
                        kind: LexErrorKind::Generic,
                    });
                    self.bump();
                }
            }
        }
        let sym = self.interner.get_or_intern(String::from_utf8_lossy(&buf));
        Token {
            kind: TokenKind::ByteStrLit(sym),
            span: self.span_from(start),
        }
    }

    /// Lex a byte literal: b'A'
    /// Uses lex_byte_escape (no \u{} allowed per v1.2.2 spec)
    pub(super) fn lex_byte(&mut self, start: BytePos) -> Token {
        self.bump(); // b
        self.bump(); // '
        let val = match self.peek() {
            None => {
                self.errors.push(LexError {
                    message: "unterminated byte literal".into(),
                    span: self.span_from(start),
                    kind: LexErrorKind::Generic,
                });
                0
            }
            Some(b'\\') => {
                self.bump();
                self.lex_byte_escape().map(|c| c as u8).unwrap_or(0)
            }
            Some(b) if b.is_ascii() => {
                self.bump();
                b
            }
            Some(_) => {
                self.errors.push(LexError {
                    message: "non-ASCII byte in byte literal".into(),
                    span: Span::new(self.pos, self.pos + 1),
                    kind: LexErrorKind::Generic,
                });
                self.bump();
                0
            }
        };
        if self.peek() == Some(b'\'') {
            self.bump();
        }
        Token {
            kind: TokenKind::ByteLit(val),
            span: self.span_from(start),
        }
    }

    /// Lex a byte escape sequence (no \u{} allowed, per v1.2.2 spec)
    pub(super) fn lex_byte_escape(&mut self) -> Option<char> {
        let b = self.bump()?;
        Some(match b {
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'\\' => '\\',
            b'0' => '\0',
            b'\'' => '\'',
            b'"' => '"',
            b'x' => {
                // \xHH (2 hex digits, ASCII only)
                let h1 = self.bump().and_then(|c| (c as char).to_digit(16))?;
                let h2 = self.bump().and_then(|c| (c as char).to_digit(16))?;
                char::from_u32(h1 * 16 + h2)?
            }
            // \u{} is FORBIDDEN in byte escapes (v1.2.2 spec)
            b'u' => {
                self.errors.push(LexError {
                    message: "\\u{} escape not allowed in byte literal".into(),
                    span: Span::new(self.pos - 1, self.pos),
                    kind: LexErrorKind::Generic,
                });
                return None;
            }
            _ => {
                self.errors.push(LexError {
                    message: format!("invalid byte escape: \\{}", b as char),
                    span: Span::new(self.pos - 1, self.pos),
                    kind: LexErrorKind::Generic,
                });
                return None;
            }
        })
    }

    /// Lex a raw byte string: br"..." or br#"..."#
    pub(super) fn lex_raw_byte_string(&mut self, start: BytePos) -> Token {
        self.bump(); // b
        self.bump(); // r
                     // Count hashes
        let mut hash_count = 0;
        while self.peek() == Some(b'#') {
            self.bump();
            hash_count += 1;
        }
        if self.peek() != Some(b'"') {
            self.errors.push(LexError {
                message: "expected `\"` after `br#...`".into(),
                span: self.span_from(start),
                kind: LexErrorKind::Generic,
            });
            return Token {
                kind: TokenKind::Eof,
                span: self.span_from(start),
            }; // Error recovery
        }
        self.bump(); // "
        let buf_start = self.pos;
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated raw byte string".into(),
                        span: self.span_from(start),
                        kind: LexErrorKind::Generic,
                    });
                    break;
                }
                Some(b'"') => {
                    let close_start = self.pos;
                    self.bump();
                    let mut found = 0;
                    while found < hash_count && self.peek() == Some(b'#') {
                        self.bump();
                        found += 1;
                    }
                    if found == hash_count {
                        let content = &self.src[buf_start as usize..close_start as usize];
                        let sym = self.interner.get_or_intern(content);
                        return Token {
                            kind: TokenKind::ByteStrLit(sym),
                            span: self.span_from(start),
                        };
                    }
                }
                Some(b) if b.is_ascii() => {
                    self.bump();
                }
                Some(_) => {
                    self.errors.push(LexError {
                        message: "non-ASCII byte in raw byte string".into(),
                        span: Span::new(self.pos, self.pos + 1),
                        kind: LexErrorKind::Generic,
                    });
                    self.bump();
                }
            }
        }
        Token {
            kind: TokenKind::ByteStrLit(Spur::default()),
            span: self.span_from(start),
        }
    }

    /// Lex a char literal: 'a' or an escape: '\n'
    /// Or a lifetime: 'a
    pub(super) fn lex_char_or_lifetime(&mut self, start: BytePos) -> Token {
        self.bump(); // '
        match self.peek() {
            // Lifetime: 'a, 'b, 'static (identifier follows)
            Some(b) if b.is_ascii_alphabetic() || b == b'_' => {
                // Check if it's actually a char literal or a lifetime
                // Lifetime: 'ident (no closing quote)
                // Char: 'c' (closing quote)
                let save_pos = self.pos;
                // Read identifier
                while let Some(b) = self.peek() {
                    if b.is_ascii_alphanumeric() || b == b'_' {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.peek() == Some(b'\'') {
                    // It was a char literal like 'a'
                    self.bump(); // closing '
                    let text = &self.src[save_pos as usize..(self.pos - 1) as usize];
                    let c = text.chars().next().unwrap_or('\0');
                    // Handle escapes
                    if text.len() > 1 && text.as_bytes()[0] == b'\\' {
                        match self.lex_escape_from_str(text) {
                            Some(escaped) => Token {
                                kind: TokenKind::CharLit(escaped),
                                span: self.span_from(start),
                            },
                            None => {
                                // Stage 14.102: Unrecognized escape — emit error
                                self.errors.push(LexError {
                                    message: format!("invalid character escape: `{}`", text),
                                    span: self.span_from(start),
                                    kind: LexErrorKind::Generic,
                                });
                                Token {
                                    kind: TokenKind::CharLit('\0'),
                                    span: self.span_from(start),
                                }
                            }
                        }
                    } else {
                        Token {
                            kind: TokenKind::CharLit(c),
                            span: self.span_from(start),
                        }
                    }
                } else {
                    // It's a lifetime
                    let text = &self.src[save_pos as usize..self.pos as usize];
                    let sym = self.interner.get_or_intern(text);
                    // Special case: 'static
                    Token {
                        kind: TokenKind::Lifetime(sym),
                        span: self.span_from(start),
                    }
                }
            }
            // Escape: '\n', '\t', etc.
            Some(b'\\') => {
                self.bump();
                let c = self.lex_escape().unwrap_or('\0');
                if self.peek() == Some(b'\'') {
                    self.bump();
                }
                Token {
                    kind: TokenKind::CharLit(c),
                    span: self.span_from(start),
                }
            }
            // Regular char: 'a'
            Some(_) => {
                let rest = &self.src[self.pos as usize..];
                // Guarded by `Some(_)` arm: rest has at least one char.
                let c = rest.chars().next().expect("Some(_) arm => rest non-empty");
                self.pos += c.len_utf8() as u32;
                if self.peek() == Some(b'\'') {
                    self.bump();
                }
                Token {
                    kind: TokenKind::CharLit(c),
                    span: self.span_from(start),
                }
            }
            None => {
                self.errors.push(LexError {
                    message: "unterminated char literal".into(),
                    span: self.span_from(start),
                    kind: LexErrorKind::Generic,
                });
                Token {
                    kind: TokenKind::CharLit('\0'),
                    span: self.span_from(start),
                }
            }
        }
    }

    /// Lex an escape sequence after `\`. Returns the decoded character.
    pub(super) fn lex_escape(&mut self) -> Option<char> {
        let b = self.bump()?;
        Some(match b {
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'\\' => '\\',
            b'0' => '\0',
            b'\'' => '\'',
            b'"' => '"',
            b'x' => {
                // \xHH (2 hex digits)
                let h1 = self.bump().and_then(|c| (c as char).to_digit(16))?;
                let h2 = self.bump().and_then(|c| (c as char).to_digit(16))?;
                char::from_u32(h1 * 16 + h2)?
            }
            b'u' => {
                // \u{...}
                if self.peek() == Some(b'{') {
                    self.bump();
                }
                let mut val = 0u32;
                while let Some(b) = self.peek() {
                    if b == b'}' {
                        self.bump();
                        break;
                    }
                    if let Some(d) = (b as char).to_digit(16) {
                        val = val * 16 + d;
                        self.bump();
                    } else {
                        break;
                    }
                }
                char::from_u32(val)?
            }
            _ => {
                self.errors.push(LexError {
                    message: format!("invalid escape: \\{}", b as char),
                    span: Span::new(self.pos - 1, self.pos),
                    kind: LexErrorKind::Generic,
                });
                return None;
            }
        })
    }

    /// Parse escape from a string slice (for char literal re-parsing).
    /// Stage 14.102 (Phase 1 audit fix): `lex_escape_from_str` now returns
    /// `Option<char>` instead of silently falling back to the last char.
    ///
    /// **Before**: `'\q'` silently became `'q'` (violating §1.0 原则 5
    /// "报错 > 静默").
    ///
    /// **After**: Returns `None` for unrecognized escapes. Callers check
    /// the result and push a `LexError` if needed.
    pub(super) fn lex_escape_from_str(&self, s: &str) -> Option<char> {
        match s {
            "\\n" => Some('\n'),
            "\\r" => Some('\r'),
            "\\t" => Some('\t'),
            "\\\\" => Some('\\'),
            "\\0" => Some('\0'),
            "\\'" => Some('\''),
            "\\\"" => Some('"'),
            _ => None, // Unrecognized escape — caller should error
        }
    }
}
