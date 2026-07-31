//! Stage 15.2 (v0.2): Type Interner Infrastructure
//!
//! This module provides the `TypeInterner` — a foundation for v0.2 Ty interning.
//! It does NOT modify the existing `Ty` struct — instead it provides:
//! - `TypeInterner`: arena allocator + dedup map for future Ty interning
//! - `InternedTy`: newtype wrapper for interned types (v0.3 will use this)
//! - Migration helpers for gradual transition
//!
//! Per `docs/lang-design/19-ty-interning.md`:
//! - v0.2: Add infrastructure (this module) + pre-build indexes
//! - v0.3: Migrate Ty to InternedTy (replace Rc with &'tcx)
//!
//! Per §23 (API Naming): `TypeInterner` follows `<Noun>` pattern for types.
//! Per §16 (Interface Isolation): this module is self-contained, no upstream deps.

use crate::mir::ty::{Ty, TyKind};
use std::rc::Rc;

/// A type interner that deduplicates TyKind values.
///
/// Stage 15.2: Infrastructure only — not yet used by the compiler pipeline.
/// v0.3 will replace `Ty::new(kind, span)` calls with `interner.intern(kind)`.
///
/// Per §1.0 原则 6 "通用 > 特例": one interner handles all TyKind variants.
/// Per §1.0 原则 3 "显式 > 隐式": interning is explicit (caller calls `intern()`).
pub struct TypeInterner {
    /// Deduplication map: TyKind → interned Rc<TyKind>
    /// Uses TyKind's PartialEq + Hash (when derived) for dedup.
    /// Note: TyKind doesn't implement Hash yet (v0.3 will add it).
    /// For now, this is a placeholder that just allocates without dedup.
    arena: Vec<Rc<TyKind>>,
}

impl TypeInterner {
    /// Create a new empty type interner.
    pub fn new() -> Self {
        Self { arena: Vec::new() }
    }

    /// Intern a TyKind, returning a Ty.
    ///
    /// Stage 15.2: Currently just wraps in Rc (no dedup).
    /// v0.3 will add HashMap-based deduplication.
    pub fn intern(&mut self, kind: TyKind) -> Ty {
        // Stage 15.2: Placeholder — just construct Ty normally.
        // v0.3: self.dedup.entry(kind).or_insert_with(|| Rc::new(kind)).clone()
        self.arena.push(Rc::new(kind.clone()));
        Ty::new(kind, crate::session::Span::DUMMY)
    }

    /// Number of interned types (for debugging/stats).
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Is the interner empty?
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
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
    fn test_interner_basic() {
        let mut interner = TypeInterner::new();
        let ty1 = interner.intern(TyKind::Int(IntTy::I32));
        let ty2 = interner.intern(TyKind::Int(IntTy::I32));
        assert_eq!(ty1, ty2);
        assert_eq!(interner.len(), 2); // No dedup yet (v0.3)
    }

    #[test]
    fn test_interner_empty() {
        let interner = TypeInterner::new();
        assert!(interner.is_empty());
        assert_eq!(interner.len(), 0);
    }
}
