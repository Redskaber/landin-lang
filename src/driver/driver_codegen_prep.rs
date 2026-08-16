//! Driver codegen preparation: fn_name_by_def_id + type_name_by_def_id.
//!
//! Per §13.4 J1-J6 (Stage 18.138): extracted from driver/mod.rs.

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
