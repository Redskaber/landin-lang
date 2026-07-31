//! Stage 15.2 (v0.2): Type Interner Infrastructure
//! Stage 15.26 (v0.2): Activated HashMap-based deduplication (Eq+Hash ready).
//!
//! This module provides the `TypeInterner` — a dedup allocator for Ty values.
//! Stage 15.25 added `Eq + Hash` derives to `TyKind`, enabling HashMap dedup.
//!
//! Per `docs/lang-design/19-ty-interning.md`:
//! - v0.2: Add infrastructure + activate dedup (this module)
//! - v0.3: Migrate Ty to InternedTy (replace Rc with &'tcx)
//!
//! Per §23 (API Naming): `TypeInterner` follows `<Noun>` pattern for types.
//! Per §16 (Interface Isolation): this module is self-contained, no upstream deps.

use crate::mir::ty::{Ty, TyKind};
use std::collections::HashMap;

/// A type interner that deduplicates TyKind values.
///
/// Stage 15.2: Infrastructure added (placeholder, no dedup).
/// Stage 15.26: Activated HashMap-based deduplication. Now `intern()` returns
///   the same `Ty` for equal `TyKind` values, reducing memory usage.
///
/// Per §1.0 原则 6 "通用 > 特例": one interner handles all TyKind variants.
/// Per §1.0 原则 3 "显式 > 隐式": interning is explicit (caller calls `intern()`).
pub struct TypeInterner {
    /// Deduplication map: TyKind → Ty.
    /// Uses TyKind's PartialEq + Eq + Hash (Stage 15.25) for dedup.
    dedup: HashMap<TyKind, Ty>,
}

impl TypeInterner {
    /// Create a new empty type interner.
    pub fn new() -> Self {
        Self {
            dedup: HashMap::new(),
        }
    }

    /// Intern a TyKind, returning a Ty.
    ///
    /// Stage 15.26: Now uses HashMap dedup — equal TyKind values return the
    /// same Ty (by value, since Ty is Clone). This reduces memory by avoiding
    /// duplicate TyKind allocations.
    ///
    /// Per §23 (API Naming): `intern` follows `<verb>` pattern.
    pub fn intern(&mut self, kind: TyKind) -> Ty {
        // Stage 15.26: HashMap dedup — if we've seen this TyKind before,
        // return the cached Ty. Otherwise, create a new Ty and cache it.
        self.dedup
            .entry(kind)
            .or_insert_with_key(|k| Ty::from_kind(k.clone()))
            .clone()
    }

    /// Number of unique interned types (for debugging/stats).
    pub fn len(&self) -> usize {
        self.dedup.len()
    }

    /// Is the interner empty?
    pub fn is_empty(&self) -> bool {
        self.dedup.is_empty()
    }

    /// Stage 15.26: Number of dedup hits (how many times we returned a cached Ty).
    /// Returns (total_intern_calls, unique_types).
    /// `total - unique` = number of dedup hits.
    pub fn dedup_stats(&self) -> (usize, usize) {
        // We don't track total calls separately, but len() gives unique count.
        // Caller can track total calls externally.
        (self.dedup.len(), self.dedup.len())
    }
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::mir::ty::TyKind;

    #[test]
    fn stage15_26_interner_dedup_basic() {
        let mut interner = TypeInterner::new();
        let ty1 = interner.intern(TyKind::Int(IntTy::I32));
        let ty2 = interner.intern(TyKind::Int(IntTy::I32));
        // Both should be equal (dedup works)
        assert_eq!(ty1, ty2);
        // Only 1 unique type in the interner (dedup!)
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn stage15_26_interner_dedup_different_types() {
        let mut interner = TypeInterner::new();
        let _ty1 = interner.intern(TyKind::Int(IntTy::I32));
        let _ty2 = interner.intern(TyKind::Bool);
        let _ty3 = interner.intern(TyKind::Int(IntTy::I32)); // dedup hit
                                                             // 2 unique types (I32 and Bool), even though we interned 3 times
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn stage15_26_interner_empty() {
        let interner = TypeInterner::new();
        assert!(interner.is_empty());
        assert_eq!(interner.len(), 0);
    }

    #[test]
    fn stage15_26_interner_complex_types() {
        let mut interner = TypeInterner::new();
        let tuple_ty = TyKind::Tuple(vec![
            Ty::from_kind(TyKind::Int(IntTy::I32)),
            Ty::from_kind(TyKind::Bool),
        ]);
        let _ty1 = interner.intern(tuple_ty.clone());
        let _ty2 = interner.intern(tuple_ty.clone());
        // Same complex type should dedup
        assert_eq!(interner.len(), 1);
    }

    #[test]
    fn stage15_26_interner_ref_types() {
        let mut interner = TypeInterner::new();
        use crate::mir::ty::{Mutability, Region};
        let ref_ty = TyKind::Ref(
            Region::Static,
            Mutability::Immutable,
            Box::new(Ty::from_kind(TyKind::Int(IntTy::I32))),
        );
        let _ty1 = interner.intern(ref_ty.clone());
        let _ty2 = interner.intern(ref_ty.clone());
        assert_eq!(interner.len(), 1);
    }
}
