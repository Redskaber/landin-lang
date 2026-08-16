//! Driver codegen preparation: fn_name_by_def_id + type_name_by_def_id.
//!
//! Per §13.4 J1-J6 (Stage 18.138): extracted from driver/mod.rs.

use super::driver_validations::owner_return_ty;
use super::BodyMeta;
use crate::hir::*;

/// Populate fn_name_by_def_id with top-level fn names, impl method names,
/// and trait default method names.
///
/// Per §13.4 J1-J6 (Stage 18.138): extracted from compile_inner.
/// The HashMap is pre-populated with closure function names during the
/// per-body loop; this function adds the remaining entries.
pub(super) fn populate_fn_name_by_def_id(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    fn_name_by_def_id: &mut std::collections::HashMap<crate::hir::DefId, String>,
) {
    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) = owner {
            let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
            let stripped = name.strip_prefix("landin_").unwrap_or(name);
            fn_name_by_def_id.insert(*def_id, format!("landin_{}", stripped));
        }
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    let method = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                    let self_ty_name =
                        if let crate::hir::HirTyKind::Path(_, p) = &impl_block.self_ty.kind {
                            if let Some(seg) = p.segments.last() {
                                interner.try_resolve(&seg.ident.name).unwrap_or("Self")
                            } else {
                                "Self"
                            }
                        } else {
                            "Self"
                        };
                    let self_stripped =
                        self_ty_name.strip_prefix("landin_").unwrap_or(self_ty_name);
                    let method_stripped = method.strip_prefix("landin_").unwrap_or(method);
                    fn_name_by_def_id.insert(
                        f.hir_id.owner,
                        format!("landin_{}_{}", self_stripped, method_stripped),
                    );
                }
            }
        }
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.body.is_some() {
                        let method = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                        let trait_name = interner.try_resolve(&t.ident.name).unwrap_or("Trait");
                        let trait_stripped =
                            trait_name.strip_prefix("landin_").unwrap_or(trait_name);
                        let method_stripped = method.strip_prefix("landin_").unwrap_or(method);
                        fn_name_by_def_id.insert(
                            f.hir_id.owner,
                            format!("landin_{}_default_{}", trait_stripped, method_stripped),
                        );
                    }
                }
            }
        }
    }
}

/// Build type_name_by_def_id: maps struct/enum DefId to their name Symbol.
///
/// Per §13.4 J1-J6 (Stage 18.138): extracted from compile_inner.
pub(super) fn build_type_name_by_def_id(
    hir: &HirCrate,
) -> std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol> {
    let mut map = std::collections::HashMap::new();
    for (def_id, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(item) = owner {
            let name = match item {
                crate::hir::HirItem::Struct(s) => s.ident.name,
                crate::hir::HirItem::Enum(e) => e.ident.name,
                _ => continue,
            };
            map.insert(*def_id, name);
        }
    }
    map
}

/// Build body_metas: per-body metadata for codegen.
///
/// Per §13.4 J1-J6 (Stage 18.139): extracted from compile_inner.
pub(super) fn build_body_metas(
    interner: &lasso::Rodeo,
    hir: &HirCrate,
    lowered_body_owners: &std::collections::HashSet<crate::hir::DefId>,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
) -> Vec<super::BodyMeta> {
    hir.bodies
        .iter()
        .filter_map(|(body_id, body)| {
            // Stage 14.100 (Bug AA5 fix): Skip body_metas for bodies that
            // were skipped during MIR lowering (trait default bodies with
            // zero impls). Without this filter, codegen would try to emit
            // functions for bodies that have no MIR, producing invalid LLVM
            // IR like `void %(void %arg0)`.
            if !lowered_body_owners.contains(&body_id.owner.0) {
                return None;
            }
            // Stage 14.72: Use fn_name_by_def_id for name resolution.
            //
            // Previously, body_metas recomputed the fn name by iterating
            // hir.owners. But impl methods are stored as HirItem::Fn owners
            // (not HirItem::Impl), so the Impl branch was never matched.
            // This caused all impl methods with the same name (e.g.,
            // Inner::new and Outer::new) to resolve to `landin_new`,
            // producing duplicate function definitions → segfault.
            //
            // Fix: look up the name from fn_name_by_def_id, which was
            // built earlier with proper type-qualified names for impl
            // methods (landin_<Type>_<method>).
            let owner_def_id = body_id.owner.0;
            let fn_name = if let Some(name) = fn_name_by_def_id.get(&owner_def_id) {
                name.clone()
            } else {
                // Fallback: recompute from HirItem::Fn owner.
                hir.owners
                    .iter()
                    .find_map(|(_, owner)| match owner {
                        crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f))
                            if f.body == Some(*body_id) =>
                        {
                            let name = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                            let stripped = name.strip_prefix("landin_").unwrap_or(name);
                            Some(format!("landin_{}", stripped))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("fn_{}", body_id.owner.0.as_u32()))
            };
            // Check if void (no return type).
            let return_ty = hir.find_owner(body_id.owner.0).and_then(owner_return_ty);
            let is_void = return_ty.is_none();
            // Stage 13.22: Force `main`/`landin_main` to return i32 (not void).
            // The C wrapper declares `extern int landin_main(void)` and reads
            // the return value. If the LLVM function is void, the return
            // register contains garbage → undefined exit code (e.g., 219).
            // For void main, codegen emits `ret i32 0` instead of `ret void`.
            let is_void = is_void && fn_name != "landin_main";
            // Stage 8.3: Get the ABI from the function owner.
            let abi = hir
                .find_owner(body_id.owner.0)
                .and_then(|owner| match owner {
                    crate::hir::OwnerNode::Item(crate::hir::HirItem::Fn(f)) => Some(f.sig.abi),
                    _ => None,
                })
                .unwrap_or(crate::ast::Abi::Landin);
            Some(BodyMeta {
                fn_name,
                is_void,
                param_count: body.params.len(),
                abi,
            })
        })
        .collect()
}
