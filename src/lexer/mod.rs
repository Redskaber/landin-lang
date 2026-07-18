//! Lexer: source text → token stream.
//!
//! Based on 02-grammar.md §1 (lexical structure).
//! Hand-written recursive lexer (no flex/re2c).

pub mod reader;
pub mod token;

pub use reader::{LexError, Lexer};
pub use token::*;

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
