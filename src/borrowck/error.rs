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
    /// Using `Operand::Copy` on a type that does not implement Copy.
    /// The MIR lower should have used `Operand::Move` instead.
    NotCopy,
}

impl BorrowError {
    pub fn new(message: &str, span: Span, kind: BorrowErrorKind) -> Self {
        Self {
            message: message.to_string(),
            span,
            kind,
        }
    }

    pub fn new_owned(message: String, span: Span, kind: BorrowErrorKind) -> Self {
        Self {
            message,
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

    /// Construct a NotCopy error. Used when `Operand::Copy` is applied
    /// to a type that doesn't implement Copy (e.g., a struct without
    /// `#[derive(Copy)]`, a `Vec`, a `String`, a `Box`).
    pub fn not_copy(message: String, span: Span) -> Self {
        Self::new_owned(message, span, BorrowErrorKind::NotCopy)
    }
}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "borrow error: {} at {}", self.message, self.span)
    }
}
