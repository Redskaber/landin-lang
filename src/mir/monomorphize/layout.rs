//! Stage 16.57 (Task 11 Phase 4b): Per-mono layouts.
//! Stage 16.58 (Task 11 Phase 4c): Codegen integration — lookup_mono_layout.
//!
//! Provides `MonoLayoutKey`, `MonoLayoutMap`, `build_mono_layouts`, and
//! `lookup_mono_layout` for per-mono specialized type layouts.
//!
//! Per §23: all types/functions follow standard patterns.
//! Per §16: reads HIR (for layout building) + MonoLayoutMap (for lookup).

use super::item::MonoItem;
use crate::hir::DefId;
use crate::mir::ty::TyKind;

// =====================================================================
// Stage 16.57 (Task 11 Phase 4b): Per-mono layouts
// =====================================================================

/// A hashable key for per-mono layouts.
///
/// Wraps `(DefId, Vec<TyKind>)` — the DefId of the generic type plus the
/// TyKind values of its substs. Uses `Vec<TyKind>` (extracted from substs)
/// rather than `SubstsRef` (`Rc<[Ty]>`) because:
/// - `Vec<TyKind>` implements `Hash`/`Eq` out of the box (no deref needed)
/// - `Rc<[Ty]>` does NOT implement `Hash`/`Eq` (would need manual deref)
/// - `TyKind::clone()` is cheaper than `Ty::clone()` (Ty goes through interner)
///
/// Two MonoLayoutKeys are equal iff they have the same DefId and the same
/// substs (element-wise TyKind comparison). This ensures `Box<i32>` and
/// `Box<i32>` map to the same layout, while `Box<i32>` and `Box<bool>` map
/// to different layouts.
///
/// Per §23: `MonoLayoutKey` follows `<Noun>_<Noun>_<Noun>` pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonoLayoutKey {
    pub def_id: DefId,
    pub substs: Vec<TyKind>,
}

impl MonoLayoutKey {
    /// Create a MonoLayoutKey from a DefId and substs slice.
    ///
    /// Extracts the TyKind from each Ty in the substs.
    pub fn new(def_id: DefId, substs: &crate::mir::ty::SubstsRef) -> Self {
        let substs = substs.iter().map(|t| t.kind.clone()).collect();
        MonoLayoutKey { def_id, substs }
    }

    /// Create a MonoLayoutKey from a MonoItem.
    ///
    /// Stage 16.62: Gated behind `#[cfg(test)]` — only used by unit tests.
    #[cfg(test)]
    pub fn from_mono_item(item: &MonoItem) -> Self {
        match item {
            MonoItem::Type { def_id, substs }
            | MonoItem::Fn { def_id, substs }
            | MonoItem::Closure { def_id, substs } => Self::new(*def_id, substs),
        }
    }
}

/// A map from MonoLayoutKey to AdtLayout.
///
/// Each entry represents one specialized layout for a generic type
/// instantiation. For example, `Box<i32>` and `Box<bool>` have distinct
/// entries because their field types differ (i32 vs bool).
///
/// Built by `build_mono_layouts` from collected MonoItems. The layouts use
/// substituted field types — e.g., for `struct Box<T> { val: T }` with
/// substs `[i32]`, the field type is `i32` (not `Param(T)` or `Error`).
///
/// Per §23: `MonoLayoutMap` follows `<Noun>_<Noun>_<Noun>` pattern.
pub type MonoLayoutMap = std::collections::HashMap<MonoLayoutKey, crate::mir::body::AdtLayout>;

/// Build per-mono layouts for all Type MonoItems.
///
/// For each `MonoItem::Type { def_id, substs }`:
/// 1. Get the generic params via `generics_of(def_id, hir)`
/// 2. Lower each field type with `lower_hir_ty_to_mir_ty_with_generics`
///    (resolves type params to `Param`)
/// 3. Apply `substitute(field_ty, substs)` to replace `Param` with actual types
/// 4. Build an `AdtLayout` with the substituted field types
/// 5. Insert into the map keyed by `MonoLayoutKey`
///
/// Non-generic types (empty substs) are skipped — they use the existing
/// `AdtLayouts` map (keyed by DefId only). Only generic instantiations
/// get per-mono layouts.
///
/// Per §23: `build_mono_layouts` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: reads HIR + MIR (allowed during layout building).
/// Per §1.0 原則 6 "通用 > 特例": one function for struct + enum layouts.
pub fn build_mono_layouts(items: &[MonoItem], hir: &crate::hir::HirCrate) -> MonoLayoutMap {
    use crate::hir::{HirItem, OwnerNode};
    use crate::mir::body::AdtLayout;
    use crate::mir::ty::TyKind as Tk;
    use crate::session::Span;

    let mut map = MonoLayoutMap::new();

    for item in items {
        // Only build layouts for Type MonoItems with non-empty substs.
        let (def_id, substs) = match item {
            MonoItem::Type { def_id, substs } if !substs.is_empty() => (*def_id, substs.clone()),
            _ => continue,
        };

        // Skip if already built (dedup by key).
        let key = MonoLayoutKey::new(def_id, &substs);
        if map.contains_key(&key) {
            continue;
        }

        // Get the HIR owner for this DefId.
        let owner = match hir.owner(def_id) {
            Some(o) => o,
            None => continue,
        };

        // Get generic params for this type.
        let generic_params = crate::hir::generics::generics_of(def_id, hir);

        match owner {
            OwnerNode::Item(HirItem::Struct(s)) => {
                // Lower each field type with generics, then substitute.
                let field_tys: Vec<crate::mir::ty::Ty> = s
                    .fields
                    .iter()
                    .map(|f| {
                        let field_ty = crate::mir::lower::lower_hir_ty_to_mir_ty_with_generics(
                            &f.ty,
                            &generic_params,
                        );
                        crate::mir::substitute::substitute(&field_ty, &substs)
                    })
                    .collect();
                map.insert(key, AdtLayout::Struct { field_tys });
            }
            OwnerNode::Item(HirItem::Enum(e)) => {
                let discriminant_ty =
                    crate::mir::ty::Ty::new(Tk::Int(crate::ast::IntTy::I32), Span::DUMMY);
                let variant_payloads: Vec<Vec<crate::mir::ty::Ty>> = e
                    .variants
                    .iter()
                    .map(|variant| match &variant.data {
                        crate::hir::HirVariantData::Unit(_) => Vec::new(),
                        crate::hir::HirVariantData::Tuple(fields, _) => fields
                            .iter()
                            .map(|f| {
                                let field_ty =
                                    crate::mir::lower::lower_hir_ty_to_mir_ty_with_generics(
                                        &f.ty,
                                        &generic_params,
                                    );
                                crate::mir::substitute::substitute(&field_ty, &substs)
                            })
                            .collect(),
                        crate::hir::HirVariantData::Struct(fields, _) => fields
                            .iter()
                            .map(|f| {
                                let field_ty =
                                    crate::mir::lower::lower_hir_ty_to_mir_ty_with_generics(
                                        &f.ty,
                                        &generic_params,
                                    );
                                crate::mir::substitute::substitute(&field_ty, &substs)
                            })
                            .collect(),
                    })
                    .collect();
                map.insert(
                    key,
                    AdtLayout::Enum {
                        discriminant_ty,
                        variant_payloads,
                    },
                );
            }
            _ => {}
        }
    }

    map
}

/// Stage 16.58 (Task 11 Phase 4c): Look up a per-mono layout for a Ty.
///
/// Given a `TyKind::Adt(def_id, substs)` and an optional `MonoLayoutMap`,
/// returns the specialized `AdtLayout` if one exists for this instantiation.
/// Returns `None` if:
/// - `mono_layouts` is `None` (not built)
/// - `substs` is empty (non-generic — use the existing AdtLayouts map)
/// - No layout was built for this (def_id, substs) pair
///
/// This is the codegen integration point — codegen calls this first for
/// Adt types, falling back to `AdtLayouts` when it returns `None`.
///
/// Per §23: `lookup_mono_layout` follows `<verb>_<noun>_<noun>` pattern.
/// Per §16: reads MonoLayoutMap (built from MIR + HIR, no HIR at lookup time).
pub fn lookup_mono_layout<'a>(
    def_id: DefId,
    substs: &crate::mir::ty::SubstsRef,
    mono_layouts: Option<&'a MonoLayoutMap>,
) -> Option<&'a crate::mir::body::AdtLayout> {
    let map = mono_layouts?;
    if substs.is_empty() {
        return None;
    }
    let key = MonoLayoutKey::new(def_id, substs);
    map.get(&key)
}

// =====================================================================
// Unit Tests
// =====================================================================

// =================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::compile;
    use crate::hir::DefId;
    use crate::mir::body::AdtLayout;
    use crate::mir::collect_mono_items;
    use crate::mir::ty::{Ty, TyKind};
    use crate::session::Span;

    /// Helper: create a Ty of the given kind.
    fn ty(kind: TyKind) -> Ty {
        Ty::new(kind, Span::DUMMY)
    }

    /// Helper: create an i32 Ty.
    fn i32_ty() -> Ty {
        ty(TyKind::Int(IntTy::I32))
    }

    /// Helper: create a bool Ty.
    fn bool_ty() -> Ty {
        ty(TyKind::Bool)
    }

    // §9. MonoLayoutKey tests (Stage 16.57, Phase 4b)
    // =================================================================

    #[test]
    fn stage16_57_mono_layout_key_new() {
        let substs: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key = MonoLayoutKey::new(DefId::new(1), &substs);
        assert_eq!(key.def_id, DefId::new(1));
        assert_eq!(key.substs.len(), 1);
        assert_eq!(key.substs[0], TyKind::Int(IntTy::I32));
    }

    #[test]
    fn stage16_57_mono_layout_key_empty_substs() {
        let substs: crate::mir::ty::SubstsRef = vec![].into();
        let key = MonoLayoutKey::new(DefId::new(2), &substs);
        assert_eq!(key.def_id, DefId::new(2));
        assert!(key.substs.is_empty());
    }

    #[test]
    fn stage16_57_mono_layout_key_equality() {
        let substs1: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let substs2: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs1);
        let key2 = MonoLayoutKey::new(DefId::new(1), &substs2);
        assert_eq!(key1, key2);
    }

    #[test]
    fn stage16_57_mono_layout_key_inequality_different_def_id() {
        let substs: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs);
        let key2 = MonoLayoutKey::new(DefId::new(2), &substs);
        assert_ne!(key1, key2);
    }

    #[test]
    fn stage16_57_mono_layout_key_inequality_different_substs() {
        let substs1: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let substs2: crate::mir::ty::SubstsRef = vec![bool_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs1);
        let key2 = MonoLayoutKey::new(DefId::new(1), &substs2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn stage16_57_mono_layout_key_from_mono_item_type() {
        let item = MonoItem::Type {
            def_id: DefId::new(5),
            substs: vec![i32_ty()].into(),
        };
        let key = MonoLayoutKey::from_mono_item(&item);
        assert_eq!(key.def_id, DefId::new(5));
        assert_eq!(key.substs.len(), 1);
    }

    #[test]
    fn stage16_57_mono_layout_key_from_mono_item_fn() {
        let item = MonoItem::Fn {
            def_id: DefId::new(7),
            substs: vec![bool_ty()].into(),
        };
        let key = MonoLayoutKey::from_mono_item(&item);
        assert_eq!(key.def_id, DefId::new(7));
        assert_eq!(key.substs.len(), 1);
        assert_eq!(key.substs[0], TyKind::Bool);
    }

    #[test]
    fn stage16_57_mono_layout_key_hashable() {
        use std::collections::HashSet;
        let substs1: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let substs2: crate::mir::ty::SubstsRef = vec![i32_ty()].into();
        let key1 = MonoLayoutKey::new(DefId::new(1), &substs1);
        let key2 = MonoLayoutKey::new(DefId::new(1), &substs2);
        let mut set = HashSet::new();
        set.insert(key1);
        set.insert(key2);
        assert_eq!(set.len(), 1); // dedup
    }

    // =================================================================
    // §10. build_mono_layouts tests (Stage 16.57, Phase 4b)
    // =================================================================

    #[test]
    fn stage16_57_build_mono_layouts_empty_items() {
        let result = compile("fn main() { 0 }");
        let hir = result.hir.as_ref().expect("HIR should be available");
        let layouts = build_mono_layouts(&[], hir);
        assert!(layouts.is_empty());
    }

    #[test]
    fn stage16_57_build_mono_layouts_non_generic_skipped() {
        // Non-generic MonoItems (empty substs) are skipped.
        let result = compile("fn main() { 0 }");
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = vec![MonoItem::Type {
            def_id: DefId::new(0),
            substs: vec![].into(),
        }];
        let layouts = build_mono_layouts(&items, hir);
        assert!(layouts.is_empty());
    }

    #[test]
    fn stage16_57_build_mono_layouts_generic_struct() {
        let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have at least 1 layout (Box<i32>)
        assert!(
            !layouts.is_empty(),
            "Expected at least 1 mono layout, got: {:?}",
            layouts
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_two_instantiations() {
        let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<bool> = Box { val: true }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have 2 layouts (Box<i32> and Box<bool>)
        assert_eq!(
            layouts.len(),
            2,
            "Expected exactly 2 mono layouts (Box<i32> + Box<bool>), got: {}",
            layouts.len()
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_dedup() {
        let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<i32> = Box { val: 43 }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have 1 layout (Box<i32> deduped)
        assert_eq!(
            layouts.len(),
            1,
            "Expected exactly 1 mono layout (Box<i32> deduped), got: {}",
            layouts.len()
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_nested_generic() {
        let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have 2 layouts (Box<Box<i32>> and Box<i32>)
        assert!(
            layouts.len() >= 2,
            "Expected at least 2 mono layouts (nested Box), got: {}",
            layouts.len()
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_correct_field_type() {
        let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Find the Box<i32> layout and verify its field type is i32 (not Param or Error)
        let has_i32_field = layouts.values().any(|layout| match layout {
            AdtLayout::Struct { field_tys } => {
                field_tys.len() == 1 && matches!(field_tys[0].kind, TyKind::Int(IntTy::I32))
            }
            _ => false,
        });
        assert!(
            has_i32_field,
            "Expected a struct layout with i32 field type (substituted), got: {:?}",
            layouts
        );
    }

    #[test]
    fn stage16_57_build_mono_layouts_generic_enum() {
        let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::Some(42); }";
        let result = compile(src);
        assert!(!result.has_errors(), "errors: {:?}", result.errors);
        let hir = result.hir.as_ref().expect("HIR should be available");
        let items = collect_mono_items(&result.mirs);
        let layouts = build_mono_layouts(&items, hir);
        // Should have at least 1 layout (Opt<i32>)
        let has_enum_layout = layouts
            .values()
            .any(|layout| matches!(layout, AdtLayout::Enum { .. }));
        assert!(
            has_enum_layout,
            "Expected at least 1 enum layout (Opt<i32>), got: {:?}",
            layouts
        );
    }
}
