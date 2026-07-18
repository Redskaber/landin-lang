//! Typed map/set collections keyed by `HirId`.
//!
//! For Stage 1 we use std `HashMap`/`HashSet`. If performance becomes an
//! issue, we can swap in `fxhash` or `rustc-hash` later without changing
//! the public API.

use std::collections::{HashMap, HashSet};

use crate::hir::id::HirId;

/// Map keyed by `HirId`. Used for per-node metadata: types, mutabilities,
/// borrow info, resolution results, etc.
pub type HirIdMap<V> = HashMap<HirId, V>;

/// Set of `HirId`s. Used for "visited" sets, liveness, etc.
pub type HirIdSet = HashSet<HirId>;

/// Map keyed by `DefId`. Used for per-definition metadata: HIR owner nodes,
/// bodies, item kinds, etc.
pub type DefIdMap<V> = HashMap<crate::hir::id::DefId, V>;

/// Set of `DefId`s.
pub type DefIdSet = HashSet<crate::hir::id::DefId>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::id::{DefId, ItemLocalId};

    #[test]
    fn hir_id_map_basic() {
        let mut m: HirIdMap<String> = HirIdMap::new();
        let h1 = HirId::new(DefId(0), ItemLocalId(1));
        let h2 = HirId::new(DefId(0), ItemLocalId(2));
        m.insert(h1, "first".to_string());
        m.insert(h2, "second".to_string());
        assert_eq!(m.get(&h1), Some(&"first".to_string()));
        assert_eq!(m.get(&h2), Some(&"second".to_string()));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn hir_id_set_basic() {
        let mut s: HirIdSet = HirIdSet::new();
        let h1 = HirId::new(DefId(0), ItemLocalId(1));
        let h2 = HirId::new(DefId(0), ItemLocalId(2));
        s.insert(h1);
        s.insert(h2);
        s.insert(h1); // duplicate
        assert_eq!(s.len(), 2);
        assert!(s.contains(&h1));
        assert!(s.contains(&h2));
    }

    #[test]
    fn def_id_map_basic() {
        let mut m: DefIdMap<String> = DefIdMap::new();
        m.insert(DefId(0), "main".to_string());
        m.insert(DefId(1), "helper".to_string());
        assert_eq!(m.get(&DefId(0)), Some(&"main".to_string()));
        assert_eq!(m.len(), 2);
    }
}
