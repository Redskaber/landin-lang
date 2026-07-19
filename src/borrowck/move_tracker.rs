//! Move tracker: tracks which places have been moved.
//!
//! Per 04-ownership-borrowing.md, moving a value transfers ownership.
//! After a move, the source place cannot be read until it is
//! re-initialized (assigned to).

use crate::borrowck::PlacePath;
use std::collections::HashSet;

/// Tracks which places have been moved (their value has been moved out).
#[derive(Debug, Default)]
pub struct MoveTracker {
    /// Set of places that are currently "moved" (uninitialized).
    moved: HashSet<PlacePath>,
}

impl MoveTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a place has been moved.
    pub fn record_move(&mut self, place: PlacePath) {
        self.moved.insert(place);
    }

    /// Check if a place has been moved (and not yet re-initialized).
    pub fn is_moved(&self, place: &PlacePath) -> bool {
        self.moved.contains(place)
    }

    /// Mark a place as re-initialized (e.g., after an assignment).
    /// This "un-moves" the place, making it usable again.
    pub fn un_move(&mut self, place: &PlacePath) {
        self.moved.remove(place);
    }

    /// Clear all moves (e.g., at the start of a new scope).
    pub fn clear(&mut self) {
        self.moved.clear();
    }

    /// Number of currently-moved places.
    pub fn len(&self) -> usize {
        self.moved.len()
    }

    /// Whether there are no moved places.
    pub fn is_empty(&self) -> bool {
        self.moved.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::lvalue::LocalId;

    fn place(n: u32) -> PlacePath {
        PlacePath::local(LocalId(n))
    }

    #[test]
    fn record_and_check() {
        let mut mt = MoveTracker::new();
        assert!(!mt.is_moved(&place(0)));
        mt.record_move(place(0));
        assert!(mt.is_moved(&place(0)));
        assert!(!mt.is_moved(&place(1)));
    }

    #[test]
    fn un_move() {
        let mut mt = MoveTracker::new();
        mt.record_move(place(0));
        assert!(mt.is_moved(&place(0)));
        mt.un_move(&place(0));
        assert!(!mt.is_moved(&place(0)));
    }

    #[test]
    fn clear() {
        let mut mt = MoveTracker::new();
        mt.record_move(place(0));
        mt.record_move(place(1));
        assert_eq!(mt.len(), 2);
        mt.clear();
        assert!(mt.is_empty());
    }
}
