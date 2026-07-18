//! Resolution error type.

use crate::session::Span;

/// An error encountered during name resolution.
///
/// Non-fatal: resolution continues after an error, producing `Res::Err`
/// for unresolved paths. This allows the compiler to report multiple
/// errors in one pass.
#[derive(Debug, Clone)]
pub struct ResolveError {
    pub message: String,
    pub span: Span,
}

impl ResolveError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolve error: {} at {}", self.message, self.span)
    }
}
