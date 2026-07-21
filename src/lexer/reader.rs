//! Lexer reader: character-by-character tokenization.
//!
//! Based on 02-grammar.md §1 (lexical structure).

use crate::lexer::token::*;
use crate::session::{BytePos, Span};
use lasso::{Rodeo, Spur};

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
    src: &'a str,
    bytes: &'a [u8],
    pos: BytePos,
    /// String interner for identifiers and string literals.
    interner: &'a mut Rodeo,
    /// Collected errors (non-fatal: lexer continues after error).
    errors: Vec<LexError>,
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
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos as usize).copied()
    }

    /// Peek at the byte at offset n from current position.
    fn peek_at(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.pos as usize + n).copied()
    }

    /// Advance by one byte and return the consumed byte.
    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    /// Current span starting at `start` and ending at current position.
    fn span_from(&self, start: BytePos) -> Span {
        Span::new(start, self.pos)
    }

    /// Skip whitespace and comments.
    ///
    /// Stops at the first character that should begin a real token, INCLUDING
    /// doc comments (`///` and `//!`). Doc comments are tokenized as
    /// `TokenKind::DocComment` so that the parser/attribute system can attach
    /// them to items. Per 02-grammar.md §1.12, `////` and `//!/` are regular
    /// line comments (not doc comments) because the 4th byte is `/`.
    fn skip_trivia(&mut self) {
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
                        message: format!("unexpected character: {:?}", c),
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
    fn lex_doc_comment(&mut self, start: BytePos) -> Token {
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

    /// Lex a raw identifier: `r#name`.
    ///
    /// Per 02-grammar.md §1.2: `r#` followed by an identifier-start character
    /// produces a `RawIdent` token (escapes reserved keywords so they can be
    /// used as ordinary identifiers, e.g. `r#match`, `r#fn`).
    ///
    /// Dispatch is in `next_token`: only `r#` + ident-start byte reaches here.
    /// `r#"..."#` (raw string) and `r#` followed by other characters are
    /// handled by separate dispatch arms.
    fn lex_raw_identifier(&mut self, start: BytePos) -> Token {
        self.bump(); // r
        self.bump(); // #
        let name_start = self.pos;
        // ASCII fast path
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        // UTF-8 continuation (r#日本語 allowed)
        if let Some(b) = self.peek() {
            if !b.is_ascii() {
                let rest = &self.src[self.pos as usize..];
                let chars = rest.char_indices();
                for (_, c) in chars {
                    if unicode_xid::UnicodeXID::is_xid_continue(c) {
                        self.pos += c.len_utf8() as u32;
                    } else {
                        break;
                    }
                }
            }
        }
        // Note: `r#self`, `r#Self`, `r#crate`, `r#super`, `r#_` are rejected by Rust,
        // but per 02-grammar.md §1.2 we accept any identifier form here; name resolution
        // (Stage 1) will enforce the additional constraints.
        let text = &self.src[name_start as usize..self.pos as usize];
        let sym = self.interner.get_or_intern(text);
        Token {
            kind: TokenKind::RawIdent(sym),
            span: self.span_from(start),
        }
    }

    /// Lex an identifier or keyword.
    fn lex_ident(&mut self, start: BytePos) -> Token {
        // Simple ASCII path (fast)
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        // UTF-8 path (slow but correct)
        if let Some(b) = self.peek() {
            if !b.is_ascii() {
                let rest = &self.src[self.pos as usize..];
                let chars = rest.char_indices();
                for (_, c) in chars {
                    if unicode_xid::UnicodeXID::is_xid_continue(c) {
                        self.pos += c.len_utf8() as u32;
                    } else {
                        break;
                    }
                }
            }
        }

        let text = &self.src[start as usize..self.pos as usize];
        let span = self.span_from(start);

        // Check for keywords
        if let Some(kw) = keyword_from_str(text) {
            // Stage 3.67: intern the keyword string so downstream passes
            // (parser, resolver) can look it up via `interner.get("self")`
            // etc. Previously the resolver had to pre-intern these strings
            // (taking `&mut Rodeo`); now the lexer does it at the source.
            // This eliminates the `&mut Rodeo` smell in `resolve_crate`.
            self.interner.get_or_intern(text);
            return Token { kind: kw, span };
        }

        // Intern identifier
        let sym = self.interner.get_or_intern(text);
        Token {
            kind: TokenKind::Ident(sym),
            span,
        }
    }

    /// Lex a number (integer or float).
    fn lex_number(&mut self, start: BytePos) -> Token {
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

    fn lex_hex(&mut self, start: BytePos) -> Token {
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
        let suffix = self.try_lex_number_suffix().and_then(|s| match s.as_str() {
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
            _ => None,
        });
        Token {
            kind: TokenKind::IntLit(val, suffix),
            span,
        }
    }

    fn lex_oct(&mut self, start: BytePos) -> Token {
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
        let suffix = self.try_lex_number_suffix().and_then(|s| match s.as_str() {
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
            _ => None,
        });
        Token {
            kind: TokenKind::IntLit(val, suffix),
            span,
        }
    }

    fn lex_bin(&mut self, start: BytePos) -> Token {
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
        let suffix = self.try_lex_number_suffix().and_then(|s| match s.as_str() {
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
            _ => None,
        });
        Token {
            kind: TokenKind::IntLit(val, suffix),
            span,
        }
    }

    /// Try to lex a number type suffix (e.g., "i32", "u64", "f64").
    fn try_lex_number_suffix(&mut self) -> Option<String> {
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

    /// Lex a string literal: "..."
    fn lex_string(&mut self, start: BytePos) -> Token {
        self.bump(); // opening "
        let mut buf = String::new();
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated string literal".into(),
                        span: self.span_from(start),
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
                    let c = rest.chars().next().unwrap();
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
    fn lex_raw_string(&mut self, start: BytePos, _hashes: usize) -> Token {
        self.bump(); // r
        self.bump(); // "
        let buf_start = self.pos;
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated raw string".into(),
                        span: self.span_from(start),
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
    fn lex_raw_string_hash(&mut self, start: BytePos) -> Token {
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
    fn lex_byte_string(&mut self, start: BytePos) -> Token {
        self.bump(); // b
        self.bump(); // "
        let mut buf = Vec::new();
        loop {
            match self.peek() {
                None => {
                    self.errors.push(LexError {
                        message: "unterminated byte string".into(),
                        span: self.span_from(start),
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
    fn lex_byte(&mut self, start: BytePos) -> Token {
        self.bump(); // b
        self.bump(); // '
        let val = match self.peek() {
            None => {
                self.errors.push(LexError {
                    message: "unterminated byte literal".into(),
                    span: self.span_from(start),
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
    fn lex_byte_escape(&mut self) -> Option<char> {
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
                });
                return None;
            }
            _ => {
                self.errors.push(LexError {
                    message: format!("invalid byte escape: \\{}", b as char),
                    span: Span::new(self.pos - 1, self.pos),
                });
                return None;
            }
        })
    }

    /// Lex a raw byte string: br"..." or br#"..."#
    fn lex_raw_byte_string(&mut self, start: BytePos) -> Token {
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
    fn lex_char_or_lifetime(&mut self, start: BytePos) -> Token {
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
                        let escaped = self.lex_escape_from_str(text);
                        Token {
                            kind: TokenKind::CharLit(escaped),
                            span: self.span_from(start),
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
                let c = rest.chars().next().unwrap();
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
                });
                Token {
                    kind: TokenKind::CharLit('\0'),
                    span: self.span_from(start),
                }
            }
        }
    }

    /// Lex an escape sequence after `\`. Returns the decoded character.
    fn lex_escape(&mut self) -> Option<char> {
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
                });
                return None;
            }
        })
    }

    /// Parse escape from a string slice (for char literal re-parsing).
    fn lex_escape_from_str(&self, s: &str) -> char {
        // Simplified: only handle common escapes
        match s {
            "\\n" => '\n',
            "\\r" => '\r',
            "\\t" => '\t',
            "\\\\" => '\\',
            "\\0" => '\0',
            "\\'" => '\'',
            "\\\"" => '"',
            _ => s.chars().last().unwrap_or('\0'),
        }
    }

    // --- Multi-char operators ---

    fn lex_dot(&mut self, start: BytePos) -> Token {
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

    fn lex_lt(&mut self, start: BytePos) -> Token {
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

    fn lex_gt(&mut self, start: BytePos) -> Token {
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

    fn lex_eq(&mut self, start: BytePos) -> Token {
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

    fn lex_bang(&mut self, start: BytePos) -> Token {
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

    fn lex_plus(&mut self, start: BytePos) -> Token {
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

    fn lex_minus(&mut self, start: BytePos) -> Token {
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

    fn lex_star(&mut self, start: BytePos) -> Token {
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

    fn lex_slash(&mut self, start: BytePos) -> Token {
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

    fn lex_percent(&mut self, start: BytePos) -> Token {
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

    fn lex_and(&mut self, start: BytePos) -> Token {
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

    fn lex_or(&mut self, start: BytePos) -> Token {
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

    fn lex_caret(&mut self, start: BytePos) -> Token {
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

    fn lex_colon(&mut self, start: BytePos) -> Token {
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

/// Check if a byte can start an identifier (ASCII fast path).
///
/// Per 02-grammar.md §1.2: an identifier starts with `XID_Start` (Unicode)
/// or `_`. The ASCII subset is `[a-zA-Z_]`. Non-ASCII `XID_Start` characters
/// are validated by the unicode-xid crate inside `lex_ident` / `lex_raw_identifier`.
///
/// This helper is used by the dispatch in `next_token` to distinguish `r#name`
/// (raw identifier) from `r#"..."#` (raw string) and `r'...'` (none — `r` is
/// treated as identifier).
fn is_ident_start_byte(b: Option<u8>) -> bool {
    matches!(b, Some(c) if c.is_ascii_alphabetic() || c == b'_')
}
