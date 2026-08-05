//! Stage 16.50 (Task 11 Phase 1a): `generics_of` query — DefId → Vec<ParamTy>.
//!
//! This module provides the `generics_of` query that maps a DefId to its
//! type parameters. It walks the HIR crate's owner nodes, extracts the
//! `HirGenerics` from each item, and converts the type params to `ParamTy`.
//!
//! Per §16: reads HIR (allowed during MIR lowering + typeck setup).
//! Per §23: `generics_of` follows `<noun>_<prep>` pattern (query function).
//!
//! This is the foundation for Task 11 (monomorphization). The query is used
//! by `lower_hir_ty_to_mir_ty` to know how many type params a generic type
//! has, and by `substitute` to replace `TyKind::Param` with actual types.

use crate::hir::{HirCrate, HirGenericParam, HirItem, OwnerNode};
use crate::mir::ty::ParamTy;

/// Build a map from DefId → Vec<ParamTy> for all generic items in the crate.
///
/// For each item that has `HirGenerics` (fn, struct, enum, trait, impl,
/// type alias), extract the type parameters and convert them to `ParamTy`.
/// Lifetime parameters are skipped (they're not type params).
///
/// The `index` field of `ParamTy` is the position of the type param in
/// the generic params list (counting only type params, not lifetimes).
///
/// Stage 16.62: Gated behind `#[cfg(test)]` — only used by unit tests.
/// Per §1.0 原則 5 "去除兼容思维": test-only code shouldn't be in the
/// public production API.
///
/// Per §23: `build_generics_map` follows `<verb>_<noun>_<noun>` pattern.
#[cfg(test)]
pub fn build_generics_map(
    hir: &HirCrate,
) -> std::collections::HashMap<crate::hir::DefId, Vec<ParamTy>> {
    let mut map = std::collections::HashMap::new();
    for (def_id, owner) in &hir.owners {
        if let Some(params) = extract_type_params(owner) {
            if !params.is_empty() {
                map.insert(*def_id, params);
            }
        }
    }
    map
}

/// Query: get the type parameters for a given DefId.
///
/// Returns an empty slice if the item has no type parameters.
/// Per §23: `generics_of` follows `<noun>_<prep>` pattern (query function).
pub fn generics_of(def_id: crate::hir::DefId, hir: &HirCrate) -> Vec<ParamTy> {
    hir.owner(def_id)
        .and_then(extract_type_params)
        .unwrap_or_default()
}

/// Extract type parameters from an owner node's generics.
///
/// Walks the `HirGenerics.params` list, filters for `HirGenericParam::Type`,
/// and converts each to `ParamTy` with the correct index.
fn extract_type_params(owner: &OwnerNode) -> Option<Vec<ParamTy>> {
    let generics = match owner {
        OwnerNode::Item(HirItem::Fn(f)) => &f.generics,
        OwnerNode::Item(HirItem::Struct(s)) => &s.generics,
        OwnerNode::Item(HirItem::Enum(e)) => &e.generics,
        OwnerNode::Item(HirItem::Trait(t)) => &t.generics,
        OwnerNode::Item(HirItem::Impl(i)) => &i.generics,
        OwnerNode::Item(HirItem::TypeAlias(ta)) => &ta.generics,
        _ => return None,
    };

    let mut params = Vec::new();
    let mut type_index = 0u32;
    for param in &generics.params {
        if let HirGenericParam::Type(tp) = param {
            params.push(ParamTy {
                index: type_index,
                name: tp.ident.name,
            });
            type_index += 1;
        }
    }
    Some(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    /// Stage 16.50 test 1: generics_of returns empty for non-generic fn.
    #[test]
    fn stage16_50_generics_of_non_generic_fn() {
        let result = compile("fn main() -> i32 { 42 }");
        assert!(!result.has_errors());
        let hir = result.hir.as_ref().expect("HIR should be available");
        let params = generics_of(crate::hir::DefId::new(0), hir);
        assert!(params.is_empty());
    }

    /// Stage 16.50 test 2: generics_of returns type params for generic struct.
    #[test]
    fn stage16_50_generics_of_generic_struct() {
        let result = compile("struct Pair<A, B> { a: A, b: B } fn main() {}");
        assert!(!result.has_errors());
        let hir = result.hir.as_ref().expect("HIR should be available");
        let map = build_generics_map(hir);
        // Find the struct with 2 type params
        let two_param_items: Vec<_> = map.values().filter(|v| v.len() == 2).collect();
        assert!(
            !two_param_items.is_empty(),
            "Should find a struct with 2 type params"
        );
    }

    /// Stage 16.50 test 3: build_generics_map collects all generic items.
    #[test]
    fn stage16_50_build_generics_map() {
        let result =
            compile("struct Pair<A, B> { a: A, b: B } struct Single<T> { x: T } fn main() {}");
        assert!(!result.has_errors());
        let hir = result.hir.as_ref().expect("HIR should be available");
        let map = build_generics_map(hir);
        let total_generic_items = map.values().filter(|v| !v.is_empty()).count();
        assert!(total_generic_items >= 2);
    }

    /// Stage 16.50 test 4: generics_of returns empty for non-generic struct.
    #[test]
    fn stage16_50_generics_of_non_generic_struct() {
        let result = compile("struct Point { x: i32, y: i32 } fn main() {}");
        assert!(!result.has_errors());
        let hir = result.hir.as_ref().expect("HIR should be available");
        let map = build_generics_map(hir);
        // Non-generic struct should NOT be in the map (empty params are excluded)
        assert!(
            map.is_empty(),
            "Non-generic items should not be in generics map"
        );
    }

    /// Stage 16.50 test 5: generics_of works for generic fn.
    /// Note: Generic fn bodies don't fully compile yet (type param resolution
    /// is Phase 1b), but the HIR is still built. We check the map directly.
    #[test]
    fn stage16_50_generics_of_generic_fn() {
        let result = compile("fn id<T>(x: T) -> T { x } fn main() {}");
        // Generic fn has typeck errors (expected — Phase 1b will fix)
        let hir = result.hir.as_ref().expect("HIR should be available");
        let map = build_generics_map(hir);
        // Should find a fn with 1 type param
        let one_param_items: Vec<_> = map.values().filter(|v| v.len() == 1).collect();
        assert!(
            !one_param_items.is_empty(),
            "Should find a fn with 1 type param"
        );
    }

    /// Stage 16.50 test 6: generics_of skips lifetime params.
    #[test]
    fn stage16_50_generics_of_skips_lifetimes() {
        let result = compile("struct Foo<'a, T> { x: &'a T } fn main() {}");
        assert!(!result.has_errors());
        let hir = result.hir.as_ref().expect("HIR should be available");
        let map = build_generics_map(hir);
        // Foo has 1 lifetime + 1 type param. Should only have 1 type param.
        let one_param_items: Vec<_> = map.values().filter(|v| v.len() == 1).collect();
        assert!(
            !one_param_items.is_empty(),
            "Should find Foo with 1 type param (skipping lifetime)"
        );
    }
}
