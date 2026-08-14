//! Parse errors.

use crate::session::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Generic,
    UnexpectedToken,
    MissingToken,
    InvalidExpression,
    InvalidStatement,
    InvalidType,
    InvalidItem,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub kind: ParseErrorKind,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            kind: ParseErrorKind::Generic,
        }
    }
}

// Stage 3.64 (P2 fix): implement `Display` + `std::error::Error` for `ParseError`
// so it integrates with the standard Rust error-handling ecosystem
// (`?` propagation, `anyhow::Error`, `Box<dyn Error>`, etc.). Previously
// only carried a `message: String + span: Span` shape with no trait impls.
// Stage 15.16: implement `Spanned` for uniform span access.
impl crate::diagnostics::Spanned for ParseError {
    fn span(&self) -> crate::session::Span {
        self.span
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[parse error at {}] {}", self.span, self.message)
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
