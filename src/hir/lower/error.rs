//! Lowering error type.

use crate::session::Span;

/// An error encountered during AST → HIR lowering.
///
/// Non-fatal: lowering continues after an error, producing a best-effort
/// HIR with placeholder nodes where needed. Stage 1.3+ will integrate
/// with the `Diagnostic` system for proper error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowerErrorKind {
    Generic,
    InvalidAst,
    MissingBody,
    DuplicateOwner,
}

#[derive(Debug, Clone)]
pub struct LowerError {
    pub message: String,
    pub span: Span,
    pub kind: LowerErrorKind,
}

impl LowerError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            kind: LowerErrorKind::Generic,
        }
    }
}

// Stage 15.16: implement `Spanned` for uniform span access.
impl crate::diagnostics::Spanned for LowerError {
    fn span(&self) -> crate::session::Span {
        self.span
    }
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lower error: {} at {}", self.message, self.span)
    }
}

// Stage 3.64 (P2 fix): implement `std::error::Error` for `LowerError`
// to complete the standard error-ecosystem integration. `Display` was
// already implemented; this adds the marker trait.
impl std::error::Error for LowerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
