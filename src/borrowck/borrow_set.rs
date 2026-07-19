//! Borrow set: tracks active borrows on places.
//!
//! Per 04-ownership-borrowing.md, at any point in the program:
//! - A place can have multiple `&T` (shared) borrows, OR
//! - One `&mut T` (mutable) borrow, but not both.
//! - Raw pointers (`*const`/`*mut`) don't count as borrows (no aliasing rules).

use crate::borrowck::error::BorrowError;
use crate::session::Span;
use std::collections::HashMap;

/// Kind of borrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowKind {
    /// `&T` — shared, immutable borrow
    Shared,
    /// `&mut T` — exclusive, mutable borrow
    Mut,
    /// `&raw const T` / `&raw mut T` — raw pointer (no aliasing rules)
    Raw,
}

/// A borrow record.
#[derive(Debug, Clone)]
pub struct Borrow {
    /// The kind of borrow (shared/mut/raw).
    pub kind: BorrowKind,
    /// Where the borrow was created (for error reporting).
    pub span: Span,
}

/// The set of active borrows, keyed by place path.
#[derive(Debug, Default)]
pub struct BorrowSet {
    /// Map from place → list of active borrows on that place.
    borrows: HashMap<crate::borrowck::PlacePath, Vec<Borrow>>,
}

impl BorrowSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new borrow on a place. Returns `Err(BorrowError)` if the
    /// borrow conflicts with an existing borrow.
    ///
    /// Conflict rules:
    /// - Shared + Shared → OK
    /// - Shared + Mut → conflict
    /// - Mut + Shared → conflict
    /// - Mut + Mut → conflict
    /// - Raw + anything → OK (raw pointers don't participate in aliasing)
    pub fn add_borrow(
        &mut self,
        place: crate::borrowck::PlacePath,
        kind: BorrowKind,
        span: Span,
    ) -> Result<(), BorrowError> {
        let existing = self.borrows.entry(place).or_default();

        // Check for conflicts
        if kind != BorrowKind::Raw {
            for b in existing.iter() {
                if b.kind == BorrowKind::Raw {
                    continue;
                }
                // If new is Mut, any existing non-raw borrow is a conflict
                if kind == BorrowKind::Mut {
                    return Err(BorrowError::borrow_conflict(
                        &format!(
                            "cannot borrow as mutable because it is also borrowed as {:?}",
                            b.kind
                        ),
                        span,
                    ));
                }
                // If new is Shared and existing is Mut, conflict
                if kind == BorrowKind::Shared && b.kind == BorrowKind::Mut {
                    return Err(BorrowError::borrow_conflict(
                        "cannot borrow as immutable because it is also borrowed as mutable",
                        span,
                    ));
                }
            }
        }

        existing.push(Borrow { kind, span });
        Ok(())
    }

    /// Get the borrow kind on a place, if any active borrow exists.
    /// Returns the "strongest" borrow (Mut > Shared > Raw).
    pub fn borrow_kind(&self, place: &crate::borrowck::PlacePath) -> Option<BorrowKind> {
        self.borrows.get(place).and_then(|borrows| {
            borrows.iter().fold(None, |acc, b| match (acc, b.kind) {
                (None, _) => Some(b.kind),
                (Some(BorrowKind::Mut), _) => Some(BorrowKind::Mut),
                (Some(BorrowKind::Shared), BorrowKind::Mut) => Some(BorrowKind::Mut),
                (Some(BorrowKind::Shared), _) => Some(BorrowKind::Shared),
                (Some(BorrowKind::Raw), k) => Some(k),
            })
        })
    }

    /// Check if a place has any active (non-raw) borrows.
    pub fn is_borrowed(&self, place: &crate::borrowck::PlacePath) -> bool {
        self.borrows
            .get(place)
            .map(|bs| bs.iter().any(|b| b.kind != BorrowKind::Raw))
            .unwrap_or(false)
    }

    /// Remove all borrows on a place (called when the borrowed reference
    /// goes out of scope — in NLL, this is the last use, not the lexical
    /// scope end).
    pub fn clear_borrows(&mut self, place: &crate::borrowck::PlacePath) {
        self.borrows.remove(place);
    }

    /// Clear all borrows (e.g., at function return).
    pub fn clear_all(&mut self) {
        self.borrows.clear();
    }

    /// Total number of active borrows.
    pub fn len(&self) -> usize {
        self.borrows.values().map(|v| v.len()).sum()
    }

    /// Whether there are no active borrows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::borrowck::PlacePath;
    use crate::mir::lvalue::LocalId;

    fn place(n: u32) -> PlacePath {
        PlacePath::Local(LocalId(n))
    }

    #[test]
    fn shared_plus_shared_ok() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert_eq!(bs.borrow_kind(&place(0)), Some(BorrowKind::Shared));
    }

    #[test]
    fn shared_plus_mut_conflict() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(place(0), BorrowKind::Mut, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn mut_plus_shared_conflict() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(place(0), BorrowKind::Mut, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(place(0), BorrowKind::Shared, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn mut_plus_mut_conflict() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(place(0), BorrowKind::Mut, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(place(0), BorrowKind::Mut, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn raw_plus_anything_ok() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(place(0), BorrowKind::Raw, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        // Raw + Shared is OK. But Shared + Mut would conflict, so we
        // stop here — raw doesn't participate in aliasing rules.
        assert_eq!(bs.borrow_kind(&place(0)), Some(BorrowKind::Shared));
    }

    #[test]
    fn is_borrowed() {
        let mut bs = BorrowSet::new();
        assert!(!bs.is_borrowed(&place(0)));
        bs.add_borrow(place(0), BorrowKind::Shared, Span::DUMMY)
            .unwrap();
        assert!(bs.is_borrowed(&place(0)));
        bs.clear_borrows(&place(0));
        assert!(!bs.is_borrowed(&place(0)));
    }
}
