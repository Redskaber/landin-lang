//! Lexer: source text → token stream.
//!
//! Based on 02-grammar.md §1 (lexical structure).
//! Hand-written recursive lexer (no flex/re2c).

pub mod reader;
pub mod token;

// Stage 6.13 (TD-023) sub-modules of `reader` — declared here so they
// resolve to `src/lexer/{ident,number,string,operators}.rs`
// (sibling to `reader.rs`). Each sub-module adds methods to `impl Lexer`
// via its own `impl` block, per §14.4 (refactoring as architecture design)
// aligned with 02-grammar.md §1.1-§1.9 lexical categories.
mod ident;
mod number;
mod operators;
mod string;

pub use reader::{LexError, LexErrorKind, Lexer};
// Stage 3.63 (cross-stage naming standardization): explicit list instead of
// `pub use token::*;` to prevent accidental leakage of internal types.
// Matches the same pattern already established in src/hir/mod.rs and
// src/mir/mod.rs (Stage 3.57 P0-3 fix).
pub use token::{keyword_from_str, FloatTy, IntSuffix, Symbol, Token, TokenKind};
// Stage 18.155: public ident validation helper (used by landinc for project
// name validation).
pub use ident::is_valid_ident;

/// Collect all tokens from source.
pub fn tokenize(src: &str, interner: &mut lasso::Rodeo) -> (Vec<Token>, Vec<LexError>) {
    let mut lexer = Lexer::new(src, interner);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        let is_eof = tok.kind == TokenKind::Eof;
        // If Eof but we haven't consumed all source, it's an error recovery Eof
        // — skip it and continue lexing
        if is_eof {
            // Check if we're at the end of source
            if lexer.is_at_end() {
                tokens.push(tok);
                break;
            } else {
                // Error recovery Eof — don't add to tokens, continue
                continue;
            }
        }
        tokens.push(tok);
    }
    let errors = lexer.into_errors();
    // Ensure Eof is at the end
    if tokens
        .last()
        .map(|t| t.kind != TokenKind::Eof)
        .unwrap_or(true)
    {
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: crate::session::Span::DUMMY,
        });
    }
    (tokens, errors)
}
