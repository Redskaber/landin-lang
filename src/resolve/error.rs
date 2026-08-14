//! Resolution error type.

use crate::session::Span;

/// Stage 18.58: Structured kind for resolution errors.
///
/// Mirrors `BorrowErrorKind` design — enables machine-readable error
/// classification instead of free-form string matching.
///
/// Per §1.0 原則 3 "显式 > 隐式": error kind is explicit, not inferred
/// from message text.
/// Per §1.0 原則 6 "通用 > 特例": one enum for all resolve error patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveErrorKind {
    /// Generic resolve error (backward compat for `new(message, span)`).
    Generic,
    /// `cannot find type in this scope` — undefined type name.
    CannotFindType,
    /// `cannot find value in this scope` — undefined value/function.
    CannotFindValue,
    /// `cannot find trait in this scope` — undefined trait.
    CannotFindTrait,
    /// `cannot find macro in this scope` — undefined macro.
    CannotFindMacro,
    /// `duplicate definition for X` — name collision.
    DuplicateDefinition,
    /// `associated type X not found in trait` — invalid assoc type reference.
    AssocTypeNotFound,
    /// `cannot find trait in qualified path` — undefined trait in `<T as Trait>::Item`.
    UndefinedTraitInQualified,
}

/// An error encountered during name resolution.
///
/// Non-fatal: resolution continues after an error, producing `Res::Err`
/// for unresolved paths. This allows the compiler to report multiple
/// errors in one pass.
#[derive(Debug, Clone)]
pub struct ResolveError {
    pub message: String,
    pub span: Span,
    /// Stage 18.58: Structured error kind for machine-readable classification.
    pub kind: ResolveErrorKind,
}

impl ResolveError {
    /// Construct a generic resolve error (backward compat).
    ///
    /// Kind defaults to `ResolveErrorKind::Generic`. Callers that know the
    /// specific error pattern should use `with_kind` instead.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            kind: ResolveErrorKind::Generic,
        }
    }

    /// Stage 18.58: Construct a resolve error with an explicit kind.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": the kind is explicit, not inferred.
    /// Per §10 naming: `with_kind` follows `<prep>_<noun>` pattern.
    pub fn with_kind(kind: ResolveErrorKind, message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            kind,
        }
    }
}

// Stage 15.16: implement `Spanned` for uniform span access.
impl crate::diagnostics::Spanned for ResolveError {
    fn span(&self) -> crate::session::Span {
        self.span
    }
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolve error: {} at {}", self.message, self.span)
    }
}

// Stage 3.64 (P2 fix): implement `std::error::Error` for `ResolveError`.
impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
