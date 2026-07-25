//! Stage 6.13 (TD-023): Identifier + raw identifier + keyword recognition.
//!
//! Per 02-grammar.md §1.3 (Keyword) + §1.4 (Identifier). Extracted from
//! `reader.rs` per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `lex_raw_identifier` (`r#name` raw identifier)
//! - `lex_ident` (regular identifier + keyword lookup)
//! - `is_ident_start_byte` (ASCII fast-path helper used by `next_token`)

use crate::lexer::token::*;
use crate::session::BytePos;

use super::reader::Lexer;

impl<'a> Lexer<'a> {
    /// Lex a raw identifier: `r#name`.
    ///
    /// Per 02-grammar.md §1.2: `r#` followed by an identifier-start character
    /// produces a `RawIdent` token (escapes reserved keywords so they can be
    /// used as ordinary identifiers, e.g. `r#match`, `r#fn`).
    ///
    /// Dispatch is in `next_token`: only `r#` + ident-start byte reaches here.
    /// `r#"..."#` (raw string) and `r#` followed by other characters are
    /// handled by separate dispatch arms.
    pub(super) fn lex_raw_identifier(&mut self, start: BytePos) -> Token {
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
    pub(super) fn lex_ident(&mut self, start: BytePos) -> Token {
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
pub(super) fn is_ident_start_byte(b: Option<u8>) -> bool {
    matches!(b, Some(c) if c.is_ascii_alphabetic() || c == b'_')
}
