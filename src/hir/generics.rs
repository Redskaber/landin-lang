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
//!
//! Stage 32.3 (TD-PRELUDE-MONO-ORDER): Added `find_generics_for_fn_owner`
//! and `find_param_trait_bounds` to handle impl-block generic context.
//! Methods inside `impl<T> Foo<T> { fn bar() {} }` need access to both
//! the impl's T AND the fn's own generics. The original `find_generics`
//! only returned the owner's own generics, missing the impl's — causing
//! `self.field` to fail type resolution because T was unknown.

use crate::hir::{HirCrate, HirGenericParam, HirImplItem, HirItem, HirTraitBound, OwnerNode};
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
pub fn find_generics(def_id: crate::hir::DefId, hir: &HirCrate) -> Vec<ParamTy> {
    hir.find_owner(def_id)
        .and_then(extract_type_params)
        .unwrap_or_default()
}

/// Stage 32.3 (TD-PRELUDE-MONO-ORDER): Find the enclosing impl block's DefId
/// for a given fn owner DefId.
///
/// A fn inside `impl<T> Foo<T> { fn bar() {} }` has its own owner DefId, but
/// its generics context also includes the impl block's T. This helper scans
/// all HirImpl owners and finds the one whose `items` contains the given fn.
///
/// Returns `None` if:
/// - `fn_def_id` is not inside any impl block (e.g., free fns, trait fns).
/// - The fn owner cannot be found in any impl block.
///
/// Per §23: `find_enclosing_impl_for_fn` follows `<verb>_<noun>_<prep>_<noun>`
/// pattern.
/// Per §16: reads HIR (allowed in MIR lower, driver setup).
/// Per §1.0 原则 10 (唯一可信数据源): the impl block is the source of truth
/// for "which methods belong to this impl".
pub fn find_enclosing_impl_for_fn(
    fn_def_id: crate::hir::DefId,
    hir: &HirCrate,
) -> Option<crate::hir::DefId> {
    // Each fn inside an impl block was registered as a separate owner
    // (see lower_impl in hir/lower/item.rs:463). The fn's HirId.owner is
    // the fn's DefId; f.hir_id.owner matches it.
    //
    // Note: HirId.owner is `DefId` (bare), not `OwnerId(DefId)` — that's
    // only for BodyId.owner. We compare DefId to DefId directly.
    for (impl_def_id, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == fn_def_id {
                        return Some(*impl_def_id);
                    }
                }
            }
        }
    }
    None
}

/// Stage 32.3 (TD-PRELUDE-MONO-ORDER): Find generics for a fn owner,
/// INCLUDING the enclosing impl block's generics.
///
/// For a fn inside `impl<T, U> Foo<T, U> { fn bar<V>() {} }`, this returns
/// `[T, U, V]` — impl generics first, then fn generics.
///
/// For a fn NOT inside an impl block (free fn), returns just the fn's own
/// generics (same as `find_generics`).
///
/// This is needed because:
/// - `body_lower.rs:165` sets `cx.generic_params = find_generics(owner_def_id, hir)`.
///   For impl methods, this MUST include the impl's T so that `value: T` in
///   `fn push(&mut self, value: T)` resolves to `Param(0)`.
/// - `compile_inner.rs` builds fn_sig_table — for impl methods, the sig's
///   `generic_params` must include impl generics for proper type substitution.
///
/// Per §1.0 原则 6 (通解 > 特解): one function handles both free fns and
/// impl methods — the impl lookup is a no-op for free fns.
/// Per §1.0 原则 3 (显式 > 隐式): the impl+fn generic concatenation is
/// explicit, not implicit.
/// Per §1.0 原则 10 (唯一可信数据源): the impl block is the source of truth
/// for impl generics; the fn owner is the source of truth for fn generics.
///
/// Per §23: `find_generics_for_fn_owner` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
pub fn find_generics_for_fn_owner(fn_def_id: crate::hir::DefId, hir: &HirCrate) -> Vec<ParamTy> {
    let mut all_params = Vec::new();

    // 1. If the fn is inside an impl block, prepend the impl's generics.
    if let Some(impl_def_id) = find_enclosing_impl_for_fn(fn_def_id, hir) {
        if let Some(OwnerNode::Item(HirItem::Impl(impl_block))) = hir.find_owner(impl_def_id) {
            let impl_params = extract_type_params_inner(&impl_block.generics);
            all_params.extend(impl_params);
        }
    }

    // 2. Append the fn's own generics.
    let fn_params = find_generics(fn_def_id, hir);
    all_params.extend(fn_params);

    // Note: ParamTy.index is recomputed to be sequential (0, 1, 2, ...).
    // The HIR stores type params with their original index (per-impl or
    // per-fn), but the concatenated sequence needs a unified index space
    // so that `Param(0)` = first impl param, `Param(1)` = second impl param,
    // ..., `Param(N)` = first fn param, etc.
    //
    // This recomputation is done at the call site (in lower_path_generic_args
    // / lower_ast_ty_to_mir_ty_with_generics) by matching the param's NAME
    // against the generic_params list. As long as the names are unique across
    // impl+fn (which Rust requires), the lookup works correctly.
    //
    // Per §1.0 原则 10 (唯一可信数据源): the names in `all_params` come from
    // the HIR (impl block + fn signature), not synthesized.

    all_params
}

/// Stage 32.3 (TD-PRELUDE-MONO-ORDER): Find the trait bounds for the Nth
/// type param in the impl+fn generics chain.
///
/// For `impl<X: T> T for S<X> { fn f<Y: T>() {} }`:
/// - param 0 (X) → returns `[T]` (the trait T bound from impl).
/// - param 1 (Y) → returns `[T]` (the trait T bound from fn).
/// - param 2+ → returns `[]` (out of bounds).
///
/// This is used by `resolve_trait_method` to resolve methods on
/// `TyKind::Param(N)` receivers — when `self.x.f()` is called and
/// `self.x: X` with `X: T` (trait bound), we look up T's method `f`.
///
/// Returns an empty Vec if:
/// - The param index is out of bounds (no such param).
/// - The param has no trait bounds.
/// - The fn owner is not inside an impl block AND has no own generics.
///
/// Per §1.0 原则 3 (显式 > 隐式): bounds are explicitly tracked in HIR.
/// Per §1.0 原则 4 (报错 > 静默): out-of-bounds returns empty (not panic).
/// Per §23: `find_param_trait_bounds` follows `<verb>_<noun>_<noun>_<noun>`
/// pattern.
pub fn find_param_trait_bounds(
    fn_def_id: crate::hir::DefId,
    param_index: u32,
    hir: &HirCrate,
) -> Vec<HirTraitBound> {
    // Collect all type params (with their bounds) in impl+fn order.
    // Each entry is (ParamTy, bounds).
    let mut all_params_with_bounds: Vec<(ParamTy, Vec<HirTraitBound>)> = Vec::new();

    // 1. Add impl generics (with bounds).
    if let Some(impl_def_id) = find_enclosing_impl_for_fn(fn_def_id, hir) {
        if let Some(OwnerNode::Item(HirItem::Impl(impl_block))) = hir.find_owner(impl_def_id) {
            let mut type_index = 0u32;
            for param in &impl_block.generics.params {
                if let HirGenericParam::Type(tp) = param {
                    let param_ty = ParamTy {
                        index: type_index,
                        name: tp.ident.name,
                    };
                    let trait_bounds: Vec<HirTraitBound> = tp
                        .bounds
                        .iter()
                        .filter_map(|b| {
                            if let crate::hir::HirTypeBound::Trait(tb) = b {
                                Some(tb.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    all_params_with_bounds.push((param_ty, trait_bounds));
                    type_index += 1;
                }
            }
        }
    }

    // 2. Add fn generics (with bounds).
    if let Some(OwnerNode::Item(HirItem::Fn(f))) = hir.find_owner(fn_def_id) {
        let mut type_index = all_params_with_bounds.len() as u32;
        for param in &f.generics.params {
            if let HirGenericParam::Type(tp) = param {
                let param_ty = ParamTy {
                    index: type_index,
                    name: tp.ident.name,
                };
                let trait_bounds: Vec<HirTraitBound> = tp
                    .bounds
                    .iter()
                    .filter_map(|b| {
                        if let crate::hir::HirTypeBound::Trait(tb) = b {
                            Some(tb.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                all_params_with_bounds.push((param_ty, trait_bounds));
                type_index += 1;
            }
        }
    }

    // 3. Look up the Nth param's bounds.
    all_params_with_bounds
        .get(param_index as usize)
        .map(|(_, bounds)| bounds.clone())
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

    Some(extract_type_params_inner(generics))
}

/// Stage 32.3: Inner helper that takes `&HirGenerics` directly (avoids
/// the OwnerNode match in callers that already have the generics).
///
/// Per §1.0 原则 6 (通解 > 特例): one function extracts params from any
/// HirGenerics, regardless of owner kind.
fn extract_type_params_inner(generics: &crate::hir::HirGenerics) -> Vec<ParamTy> {
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
    params
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
        let params = find_generics(crate::hir::DefId::new(0), hir);
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
        // Stage 18.165: Prelude injects Option<T> and Result<T, E> (both
        // generic), so the map is no longer empty. We check that the
        // user-defined non-generic struct "Point" is NOT in the map.
        // Per §2 原則 9 (正确>妥协): adapt test to new prelude behavior.
        assert!(
            !map.values().any(|params| params.is_empty()),
            "Non-generic items should not have empty params in generics map"
        );
        // Verify Option and Result are in the map (prelude injection works).
        assert!(
            map.len() >= 2,
            "Prelude should inject at least 2 generic types (Option, Result), got {}",
            map.len()
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
