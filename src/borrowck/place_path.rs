//! Stage 6.14 (TD-024): PlacePath — field-sensitive place representation.
//!
//! Per 04-ownership-borrowing.md §4 (NLL data structures). Extracted from
//! `mod.rs` per `docs/stage-committee-process.md` v3.21 §14.4 + §13.4.
//!
//! Owns:
//! - `PlacePath` struct (root + projection chain)
//! - `PlaceRoot` enum (Local | Static)
//! - `ProjElem` enum (Deref | Field | Index | ConstantIndex)
//! - `impl PlacePath` (local / static_def / project / contains / overlaps)

use crate::mir::place::*;

/// A field-sensitive place path for borrow/move tracking.
///
/// Per the Stage 2.x gate review (P0-15), the previous `PlacePath`
/// collapsed projections — `a.x` and `a.y` both mapped to `Local(a)`,
/// causing false-positive borrow conflicts. This new representation
/// preserves the projection chain so that:
///   - `a.x` and `a.y` are distinct (no false conflict)
///   - `a` and `a.x` overlap (borrowing `a` conflicts with `a.x`)
///   - `*p` and `p` are distinct (the pointer vs the pointee)
///
/// The `projections` field is a `Vec<ProjElem>` (not `Vec<ProjectionElem>`)
/// because we want `Copy + PartialEq + Eq + Hash` for use as a HashMap
/// key, and the MIR `ProjectionElem` carries a `Ty` which doesn't
/// implement those traits. `ProjElem` is a stripped-down version that
/// only carries the discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlacePath {
    /// The root local (or static def_id) of the place.
    pub root: PlaceRoot,
    /// Projection chain from root to the leaf. Empty means "the root
    /// place itself" (e.g., `x` with no field access).
    pub projections: Vec<ProjElem>,
}

/// The root of a place path: either a local or a static.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaceRoot {
    Local(LocalId),
    Static(crate::hir::DefId),
}

/// A stripped-down projection element used inside `PlacePath`.
///
/// This mirrors `crate::mir::place::ProjectionElem` but omits the
/// payload types that don't implement Hash/Eq (like `Ty`). Field
/// projections use just the `FieldId`; index projections use the
/// `LocalId` of the index variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjElem {
    /// `*base` — dereference
    Deref,
    /// `base.field_id`
    Field(FieldId),
    /// `base[idx_local]`
    Index(LocalId),
    /// `base[N]` (constant index)
    ConstantIndex { offset: u64, from_end: bool },
}

impl PlacePath {
    /// Construct a path for a bare local (no projections).
    pub fn local(id: LocalId) -> Self {
        Self {
            root: PlaceRoot::Local(id),
            projections: Vec::new(),
        }
    }

    /// Construct a path for a static.
    pub fn static_def(def_id: crate::hir::DefId) -> Self {
        Self {
            root: PlaceRoot::Static(def_id),
            projections: Vec::new(),
        }
    }

    /// Append a projection element to this path, returning a new path.
    /// Used when building up a path from a place's projection chain.
    pub fn project(&self, elem: ProjElem) -> Self {
        let mut new = self.clone();
        new.projections.push(elem);
        new
    }

    /// Whether this path *contains* another path as a prefix.
    ///
    /// `a.x.y` contains `a.x` and `a`. Used to detect overlap:
    /// if you borrow `a.x`, then any access to `a`, `a.x`, or
    /// `a.x.y` overlaps; but `a.y` does not.
    pub fn contains(&self, other: &PlacePath) -> bool {
        if self.root != other.root {
            return false;
        }
        if other.projections.len() > self.projections.len() {
            return false;
        }
        other
            .projections
            .iter()
            .zip(self.projections.iter())
            .all(|(a, b)| a == b)
    }

    /// Whether two paths *overlap* (one contains the other).
    /// This is the symmetric closure of `contains`.
    pub fn overlaps(&self, other: &PlacePath) -> bool {
        self.contains(other) || other.contains(self)
    }
}
