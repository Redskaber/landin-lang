//! Borrow error type.

use crate::session::Span;

/// A borrow/ownership error encountered during borrow checking.
#[derive(Debug, Clone)]
pub struct BorrowError {
    pub message: String,
    pub span: Span,
    pub kind: BorrowErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowErrorKind {
    /// Using a value after it has been moved.
    UseAfterMove,
    /// Moving a value that is currently borrowed.
    MoveBorrowed,
    /// Assigning to a place that is currently borrowed.
    AssignBorrowed,
    /// Creating a conflicting borrow (e.g., &mut while & exists).
    BorrowConflict,
    /// Borrowing a moved value.
    BorrowMoved,
}

impl BorrowError {
    pub fn new(message: &str, span: Span, kind: BorrowErrorKind) -> Self {
        Self {
            message: message.to_string(),
            span,
            kind,
        }
    }

    pub fn use_after_move(message: &str, span: Span) -> Self {
        Self::new(message, span, BorrowErrorKind::UseAfterMove)
    }

    pub fn move_borrowed(message: &str, span: Span) -> Self {
        Self::new(message, span, BorrowErrorKind::MoveBorrowed)
    }

    pub fn assign_borrowed(message: &str, span: Span) -> Self {
        Self::new(message, span, BorrowErrorKind::AssignBorrowed)
    }

    pub fn borrow_conflict(message: &str, span: Span) -> Self {
        Self::new(message, span, BorrowErrorKind::BorrowConflict)
    }

    pub fn borrow_moved(message: &str, span: Span) -> Self {
        Self::new(message, span, BorrowErrorKind::BorrowMoved)
    }
}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "borrow error: {} at {}", self.message, self.span)
    }
}
