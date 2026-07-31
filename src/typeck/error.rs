//! Type error type.

use crate::mir::ty::Ty;
use crate::session::Span;

/// A type error encountered during type checking.
///
/// Non-fatal: type checking continues after an error, producing
/// `TyKind::Error` for the affected types. This allows the compiler
/// to report multiple errors in one pass.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
    pub expected: Option<Ty>,
    pub found: Option<Ty>,
}

impl TypeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            expected: None,
            found: None,
        }
    }

    pub fn mismatch(expected: Ty, found: Ty, span: Span) -> Self {
        Self {
            message: format!(
                "mismatched types: expected {:?}, found {:?}",
                expected.kind, found.kind
            ),
            span,
            expected: Some(expected),
            found: Some(found),
        }
    }

    pub fn unresolved(span: Span) -> Self {
        Self {
            message: "type annotations needed".into(),
            span,
            expected: None,
            found: None,
        }
    }
}

// Stage 15.16: implement `Spanned` for uniform span access.
impl crate::diagnostics::Spanned for TypeError {
    fn span(&self) -> crate::session::Span {
        self.span
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type error: {} at {}", self.message, self.span)
    }
}

// Stage 3.64 (P2 fix): implement `std::error::Error` for `TypeError`.
impl std::error::Error for TypeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
