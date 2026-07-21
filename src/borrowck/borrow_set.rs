//! Borrow set: tracks active borrows on places.
//!
//! Per 04-ownership-borrowing.md, at any point in the program:
//! - A place can have multiple `&T` (shared) borrows, OR
//! - One `&mut T` (mutable) borrow, but not both.
//! - Raw pointers (`*const`/`*mut`) don't count as borrows (no aliasing rules).
//!
//! Stage 2.4c (P0-15): The borrow set now uses field-sensitive
//! `PlacePath` (Local + Vec<ProjElem>) so that `a.x` and `a.y` are
//! distinct places. Conflict checks use `PlacePath::overlaps` to detect
//! when one borrow conflicts with another (e.g., `&mut a.x` conflicts
//! with `&a` because `a` contains `a.x`).

use crate::borrowck::error::BorrowError;
use crate::borrowck::PlacePath;
use crate::mir::place::BorrowKind;
use crate::session::Span;

// Stage 3.63 (cross-stage naming standardization): `BorrowKind` is now
// imported from `crate::mir::place` as the single source of truth.
// The former duplicate `borrowck::borrow_set::BorrowKind` (with its
// `BkKind` alias in `borrowck::mod`) has been removed — DRY restored.

/// A borrow record.
#[derive(Debug, Clone)]
pub struct Borrow {
    /// The kind of borrow (shared/mut/raw).
    pub kind: BorrowKind,
    /// The place being borrowed (field-sensitive path).
    pub place: PlacePath,
    /// Where the borrow was created (for error reporting).
    pub span: Span,
    /// The local that holds the borrow reference (e.g., `r` in `r = &x`).
    /// Used by NLL to kill the borrow when `ref_local` is last used.
    /// `None` for borrows where the LHS wasn't a simple local (rare).
    pub ref_local: Option<crate::mir::place::LocalId>,
}

/// The set of active borrows. Stores borrows in a flat `Vec` so we can
/// check each one against a new borrow via `PlacePath::overlaps`.
///
/// (Stage 2.4c: previously keyed by `HashMap<PlacePath, Vec<Borrow>>`
/// with exact-key lookup — this missed the `a.x` vs `a` overlap case.
/// Switched to flat Vec + linear scan until we have a more efficient
/// tree-based structure in Stage 3.)
#[derive(Debug, Default)]
pub struct BorrowSet {
    borrows: Vec<Borrow>,
}

impl BorrowSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new borrow on a place. Returns `Err(BorrowError)` if the
    /// borrow conflicts with an existing borrow.
    ///
    /// Conflict rules (Stage 2.4c, field-sensitive):
    /// - Two borrows on the *same* place conflict iff:
    ///     - at least one is Mut, AND
    ///     - neither is Raw
    /// - A new borrow on `p` conflicts with an existing borrow on `q` iff
    ///   `p.overlaps(q)` (one contains the other) AND the rules above
    ///   apply to the overlap.
    /// - Disjoint places never conflict: `a.x` vs `a.y` is OK.
    pub fn add_borrow(
        &mut self,
        place: PlacePath,
        kind: BorrowKind,
        span: Span,
    ) -> Result<(), BorrowError> {
        self.add_borrow_with_ref(place, kind, span, None)
    }

    /// Add a borrow with an associated `ref_local` (the local that holds
    /// the borrow reference). Used by NLL to expire the borrow at the
    /// ref_local's last use.
    pub fn add_borrow_with_ref(
        &mut self,
        place: PlacePath,
        kind: BorrowKind,
        span: Span,
        ref_local: Option<crate::mir::place::LocalId>,
    ) -> Result<(), BorrowError> {
        if kind != BorrowKind::Raw {
            for b in self.borrows.iter() {
                if b.kind == BorrowKind::Raw {
                    continue;
                }
                if !place.overlaps(&b.place) {
                    continue;
                }
                // Overlapping non-raw borrows: at least one is Mut → conflict.
                if kind == BorrowKind::Mut || b.kind == BorrowKind::Mut {
                    return Err(BorrowError::borrow_conflict(
                        &format!(
                            "cannot borrow as {:?} because it is also borrowed as {:?} (overlapping place)",
                            kind, b.kind
                        ),
                        span,
                    ));
                }
            }
        }
        self.borrows.push(Borrow {
            kind,
            place,
            span,
            ref_local,
        });
        Ok(())
    }

    /// Get the "strongest" borrow kind currently active on a place or
    /// any place that overlaps it. Returns `Some(Mut)` if any overlapping
    /// Mut borrow exists, else `Some(Shared)` if any overlapping Shared
    /// borrow exists, else `None`.
    ///
    /// Used by `check_place_write` and `check_operand(Move)` to decide
    /// if a write/move would conflict with an existing borrow.
    pub fn borrow_kind(&self, place: &PlacePath) -> Option<BorrowKind> {
        let mut strongest: Option<BorrowKind> = None;
        for b in &self.borrows {
            if b.kind == BorrowKind::Raw {
                continue;
            }
            if !place.overlaps(&b.place) {
                continue;
            }
            strongest = match (strongest, b.kind) {
                (None, _) => Some(b.kind),
                (Some(BorrowKind::Mut), _) => Some(BorrowKind::Mut),
                (Some(BorrowKind::Shared), BorrowKind::Mut) => Some(BorrowKind::Mut),
                (Some(BorrowKind::Shared), _) => Some(BorrowKind::Shared),
                (Some(BorrowKind::Raw), k) => Some(k),
            };
        }
        strongest
    }

    /// Check if a place (or any overlapping place) has any active
    /// non-raw borrows.
    pub fn is_borrowed(&self, place: &PlacePath) -> bool {
        self.borrows
            .iter()
            .any(|b| b.kind != BorrowKind::Raw && place.overlaps(&b.place))
    }

    /// Remove all borrows whose `ref_local` matches the given local.
    /// Called by NLL when the borrow reference's last use is reached.
    pub fn kill_borrows_of_local(&mut self, local: crate::mir::place::LocalId) {
        self.borrows.retain(|b| b.ref_local != Some(local));
    }

    /// Transfer all borrows whose `ref_local` is `from` to `to`.
    ///
    /// G2+ fix (Stage 2.4e): When MIR lower produces:
    ///   tmp = &x        (ref_local = tmp)
    ///   r = Move(tmp)   (transfer ref_local to r)
    /// we need to update the borrow's ref_local from `tmp` to `r`,
    /// so NLL tracks `r`'s lifetime (not `tmp`'s).
    ///
    /// Without this transfer, NLL would kill the borrow after `tmp`'s
    /// last use (which is the Move), causing subsequent borrows on `x`
    /// to incorrectly succeed.
    pub fn transfer_borrow_ref(
        &mut self,
        from: crate::mir::place::LocalId,
        to: crate::mir::place::LocalId,
    ) {
        for b in self.borrows.iter_mut() {
            if b.ref_local == Some(from) {
                b.ref_local = Some(to);
            }
        }
    }

    /// Remove all borrows whose place *exactly matches* `place` (no
    /// overlap semantics — only the exact path). Called when a borrow
    /// expires (NLL last-use, Stage 2.4c-d).
    pub fn clear_borrows(&mut self, place: &PlacePath) {
        self.borrows.retain(|b| b.place != *place);
    }

    /// Remove all borrows whose place is *rooted* at the given local
    /// (i.e., the borrow is on `local` itself or any projection of it).
    /// Used when a local goes out of scope.
    pub fn clear_borrows_on_local(&mut self, local: crate::mir::place::LocalId) {
        self.borrows.retain(|b| match b.place.root {
            crate::borrowck::PlaceRoot::Local(l) => l != local,
            _ => true,
        });
    }

    /// Clear all borrows (e.g., at function return).
    pub fn clear_all(&mut self) {
        self.borrows.clear();
    }

    /// Total number of active borrows.
    pub fn len(&self) -> usize {
        self.borrows.len()
    }

    /// Whether there are no active borrows.
    pub fn is_empty(&self) -> bool {
        self.borrows.is_empty()
    }

    /// Iterate over all active borrows.
    pub fn iter(&self) -> impl Iterator<Item = &Borrow> {
        self.borrows.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::borrowck::{PlaceRoot, ProjElem};
    use crate::mir::place::{FieldId, LocalId};

    fn local_place(n: u32) -> PlacePath {
        PlacePath::local(LocalId(n))
    }

    fn field_place(n: u32, field: u32) -> PlacePath {
        PlacePath {
            root: PlaceRoot::Local(LocalId(n)),
            projections: vec![ProjElem::Field(FieldId(field))],
        }
    }

    #[test]
    fn shared_plus_shared_ok() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert_eq!(bs.borrow_kind(&local_place(0)), Some(BorrowKind::Shared));
    }

    #[test]
    fn shared_plus_mut_conflict() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Mut, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn mut_plus_shared_conflict() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Mut, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn mut_plus_mut_conflict() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Mut, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Mut, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn raw_plus_anything_ok() {
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Raw, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert_eq!(bs.borrow_kind(&local_place(0)), Some(BorrowKind::Shared));
    }

    #[test]
    fn is_borrowed() {
        let mut bs = BorrowSet::new();
        assert!(!bs.is_borrowed(&local_place(0)));
        bs.add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .unwrap();
        assert!(bs.is_borrowed(&local_place(0)));
        bs.clear_borrows(&local_place(0));
        assert!(!bs.is_borrowed(&local_place(0)));
    }

    // === Stage 2.4c (P0-15): Field-sensitivity regression tests ===

    #[test]
    fn disjoint_fields_dont_conflict() {
        // `&a.x` and `&a.y` should NOT conflict with each other — they're disjoint.
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(field_place(0, 0), BorrowKind::Mut, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(field_place(0, 1), BorrowKind::Mut, Span::DUMMY)
            .is_ok());
        // The whole `a` overlaps with both — so it IS borrowed (you can't
        // move `a` while `&mut a.x` is alive). borrow_kind should return Mut.
        assert_eq!(bs.borrow_kind(&local_place(0)), Some(BorrowKind::Mut));
        // But a *different* local `b` is not borrowed:
        assert_eq!(bs.borrow_kind(&local_place(1)), None);
    }

    #[test]
    fn whole_and_part_conflict() {
        // `&a` and `&mut a.x` should conflict — `a` contains `a.x`.
        let mut bs = BorrowSet::new();
        assert!(bs
            .add_borrow(local_place(0), BorrowKind::Shared, Span::DUMMY)
            .is_ok());
        assert!(bs
            .add_borrow(field_place(0, 0), BorrowKind::Mut, Span::DUMMY)
            .is_err());
    }

    #[test]
    fn whole_is_borrowed_if_part_is() {
        // After `&a.x`, `is_borrowed(a)` should be true (overlap).
        let mut bs = BorrowSet::new();
        bs.add_borrow(field_place(0, 0), BorrowKind::Shared, Span::DUMMY)
            .unwrap();
        assert!(bs.is_borrowed(&local_place(0)));
    }
}
