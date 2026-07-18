//! HIR identifiers: `HirId`, `DefId`, `ItemLocalId`, `OwnerId`.
//!
//! Per 06-mir.md §3 (HIR layer):
//! - `DefId` uniquely identifies a definition (item or body) within a crate.
//! - `ItemLocalId` uniquely identifies a node within an owner's body.
//! - `HirId` is the pair (owner: DefId, local_id: ItemLocalId) and uniquely
//!   identifies any HIR node.
//!
//! These IDs are stable across incremental recompilation (in future) and
//! serve as keys into HIR maps, typeck tables, borrow-check info, etc.

use std::fmt;

/// Identifies a definition (an item or a body) within a crate.
///
/// Per-crate monotonically increasing. The crate context assigns DefIds
/// during HIR construction in pre-order traversal of the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DefId(pub u32);

impl DefId {
    pub fn new(n: u32) -> Self {
        Self(n)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for DefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DefId({})", self.0)
    }
}

/// Local identifier within an owner's body.
///
/// Each owner (fn, const, static) has its own `ItemLocalId` namespace
/// starting at 0. This keeps the ID space compact (u32 per body, not per
/// crate) and enables efficient storage of per-node metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ItemLocalId(pub u32);

impl ItemLocalId {
    pub fn new(n: u32) -> Self {
        Self(n)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ItemLocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

/// Unique identifier for any HIR node (item or expression/statement/pattern
/// within a body).
///
/// Two `HirId`s are equal iff they refer to the same node. Use this as the
/// key for any per-node metadata (type, mutability, borrow info, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct HirId {
    /// The owner of the body this node belongs to. For nodes that ARE
    /// owners (top-level items), `owner` is the item's own DefId.
    pub owner: DefId,
    /// The local ID within the owner's body. For owner nodes themselves,
    /// this is `ItemLocalId(0)`.
    pub local_id: ItemLocalId,
}

impl HirId {
    pub fn new(owner: DefId, local_id: ItemLocalId) -> Self {
        Self { owner, local_id }
    }

    pub fn is_owner(self) -> bool {
        self.local_id == ItemLocalId(0)
    }
}

impl fmt::Display for HirId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HirId({}:{})", self.owner.0, self.local_id.0)
    }
}

/// Owner of a body — fns, consts, statics have bodies; types/items without
/// bodies (struct decls, enums, traits) are "owners" but don't have a body
/// in the HIR sense.
///
/// `OwnerId` is just a typed wrapper around `DefId` to make the intent
/// clear at API boundaries (e.g., `hir.owners.get(owner_id)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OwnerId(pub DefId);

impl OwnerId {
    pub fn new(d: DefId) -> Self {
        Self(d)
    }
}

impl fmt::Display for OwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OwnerId({})", self.0 .0)
    }
}

/// A counter for generating fresh DefIds. Per-crate.
#[derive(Debug, Default)]
pub struct DefIdCounter {
    next: u32,
}

impl DefIdCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next DefId.
    pub fn fresh(&mut self) -> DefId {
        let d = DefId(self.next);
        self.next += 1;
        d
    }

    /// Peek at the next DefId without allocating.
    pub fn peek_next(&self) -> DefId {
        DefId(self.next)
    }

    /// Total number of DefIds allocated so far.
    pub fn count(&self) -> usize {
        self.next as usize
    }
}

/// A counter for generating fresh ItemLocalIds within an owner's body.
#[derive(Debug, Default)]
pub struct ItemLocalIdCounter {
    next: u32,
}

impl ItemLocalIdCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next ItemLocalId. The first call returns `ItemLocalId(0)`
    /// which by convention is reserved for the owner node itself; callers
    /// should use `fresh_owner_local()` for the owner and `fresh()` for
    /// body-internal nodes.
    pub fn fresh(&mut self) -> ItemLocalId {
        let l = ItemLocalId(self.next);
        self.next += 1;
        l
    }

    /// Allocate the owner's local ID. Should be called exactly once per body,
    /// before any `fresh()` calls.
    pub fn fresh_owner_local(&mut self) -> ItemLocalId {
        debug_assert_eq!(self.next, 0, "fresh_owner_local called twice");
        let l = ItemLocalId(0);
        self.next = 1;
        l
    }

    /// Total number of ItemLocalIds allocated so far.
    pub fn count(&self) -> usize {
        self.next as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn def_id_basic() {
        let mut c = DefIdCounter::new();
        let a = c.fresh();
        let b = c.fresh();
        assert_ne!(a, b);
        assert_eq!(a.as_u32(), 0);
        assert_eq!(b.as_u32(), 1);
        assert!(a < b);
    }

    #[test]
    fn item_local_id_basic() {
        let mut c = ItemLocalIdCounter::new();
        let owner = c.fresh_owner_local();
        let n1 = c.fresh();
        let n2 = c.fresh();
        assert_eq!(owner.as_u32(), 0);
        assert_eq!(n1.as_u32(), 1);
        assert_eq!(n2.as_u32(), 2);
    }

    #[test]
    fn hir_id_equality() {
        let owner = DefId(5);
        let h1 = HirId::new(owner, ItemLocalId(3));
        let h2 = HirId::new(owner, ItemLocalId(3));
        let h3 = HirId::new(owner, ItemLocalId(4));
        let h4 = HirId::new(DefId(6), ItemLocalId(3));
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_ne!(h1, h4);
    }

    #[test]
    fn hir_id_is_owner() {
        let owner = DefId(5);
        let h_owner = HirId::new(owner, ItemLocalId(0));
        let h_inner = HirId::new(owner, ItemLocalId(1));
        assert!(h_owner.is_owner());
        assert!(!h_inner.is_owner());
    }

    #[test]
    fn display_formats() {
        let d = DefId(42);
        let l = ItemLocalId(7);
        let h = HirId::new(d, l);
        let o = OwnerId::new(d);
        assert_eq!(format!("{d}"), "DefId(42)");
        assert_eq!(format!("{l}"), "L7");
        assert_eq!(format!("{h}"), "HirId(42:7)");
        assert_eq!(format!("{o}"), "OwnerId(42)");
    }
}
