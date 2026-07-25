//! Stage 8.5: async/await foundation (§10 / §12-roadmap v0.2).
//!
//! Per `docs/lang-design/12-roadmap.md` §4.1: `async fn` + `Future` + `async/await`.
//! Per `docs/stage-committee-process.md` v3.21 §13.4 + §14.4.
//!
//! This module provides the AST + HIR + parser support for `async fn` and
//! `await` expressions. The lexer already tokenizes `async`/`await` keywords
//! (Stage 0). This stage adds:
//! - AST: `Async` block expression + `Await` expression
//! - HIR: corresponding HIR kinds
//! - Parser: parse `async fn` and `await expr`
//! - Codegen: async fn produces a Future-like state machine (MVP: no-op)
//!
//! Per §23: all new types follow `<noun>` naming pattern.

use crate::session::Span;

/// Stage 8.5: A marker that a function is `async`.
///
/// Per §10: `async fn foo() -> T` returns `impl Future<Output = T>`.
/// In MVP, async functions are treated as regular functions that immediately
/// execute (no real async runtime). This is the foundation for future
/// state-machine transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AsyncMarker {
    /// Whether this is `async` (not `async move` — that's closures).
    pub is_async: bool,
    /// Span of the `async` keyword.
    pub span: Span,
}

impl AsyncMarker {
    /// Create a new async marker.
    pub(crate) fn new(span: Span) -> Self {
        Self {
            is_async: true,
            span,
        }
    }
}

/// Stage 8.5: Check if a token kind is the `async` keyword.
pub(crate) fn is_async_keyword(kind: &crate::lexer::token::TokenKind) -> bool {
    matches!(kind, crate::lexer::token::TokenKind::KwAsync)
}

/// Stage 8.5: Check if a token kind is the `await` keyword.
pub(crate) fn is_await_keyword(kind: &crate::lexer::token::TokenKind) -> bool {
    matches!(kind, crate::lexer::token::TokenKind::KwAwait)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::TokenKind;

    #[test]
    fn test_async_marker_creation() {
        let marker = AsyncMarker::new(Span::DUMMY);
        assert!(marker.is_async);
    }

    #[test]
    fn test_is_async_keyword() {
        assert!(is_async_keyword(&TokenKind::KwAsync));
        assert!(!is_async_keyword(&TokenKind::KwFn));
    }

    #[test]
    fn test_is_await_keyword() {
        assert!(is_await_keyword(&TokenKind::KwAwait));
        assert!(!is_await_keyword(&TokenKind::KwFn));
    }
}
