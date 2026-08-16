//! Driver object safety checks: dyn Trait usage validation.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.134):
//! Extracted from `driver.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).

use lasso::Rodeo;

use super::CompileErrors;
// Stage 18.134: walk helpers from driver_scan.rs
use super::driver_scan::{walk_hir_ty, walk_hir_ty_in_body};

/// Stage 16.65 (Task 14 Phase 2): Check object safety for all `dyn Trait` usages.
///
/// Scans all HIR types for `HirTyKind::TraitObject`. For each, resolves the
/// trait DefId from the bound's path, looks up the `HirTrait` definition,
/// and calls `check_trait_object_safety`. If any violations are found, emits
/// typeck errors.
///
/// Per §23: `check_object_safety_for_dyn_trait_usage` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads HIR + TraitResolver (allowed during driver pre-computation).
pub(super) fn check_object_safety_for_dyn_trait_usage(
    hir: &crate::hir::HirCrate,
    resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    errors: &mut CompileErrors,
) {
    use crate::hir::{HirItem, HirTyKind, HirTypeBound, OwnerNode, Res};
    use crate::traits::object_safety::check_trait_object_safety;

    // Build a map from trait DefId → HirTrait for quick lookup.
    let mut trait_defs: std::collections::HashMap<crate::hir::DefId, &crate::hir::HirTrait> =
        std::collections::HashMap::new();
    for (def_id, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Trait(t)) = owner {
            trait_defs.insert(*def_id, t);
        }
    }

    // Walk all HIR bodies for TraitObject types.
    for (_body_id, body) in &hir.bodies {
        walk_hir_ty_in_body(&body.value, &mut |ty| {
            if let HirTyKind::TraitObject { bounds, .. } = &ty.kind {
                for bound in bounds {
                    if let HirTypeBound::Trait(tc) = bound {
                        if let Res::Def(trait_def_id, _) = tc.path.res {
                            if let Some(trait_def) = trait_defs.get(&trait_def_id) {
                                let violations =
                                    check_trait_object_safety(trait_def, &trait_defs, interner);
                                if !violations.is_empty() {
                                    let trait_name = interner
                                        .try_resolve(&trait_def.ident.name)
                                        .unwrap_or("<anonymous>");
                                    for v in &violations {
                                        errors.typeck.push(crate::typeck::TypeError::new(
                                            v.error_message(trait_name, interner),
                                            v.span(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    // Also walk fn signatures, struct fields, etc. for TraitObject types.
    for (_, owner) in &hir.owners {
        match owner {
            OwnerNode::Item(HirItem::Fn(f)) => {
                for param in &f.sig.inputs {
                    if let Some(ty) = &param.ty {
                        walk_hir_ty(ty, &mut |ty| {
                            check_trait_object_ty(ty, &trait_defs, resolver, interner, errors);
                        });
                    }
                }
                if let crate::hir::HirFnRetTy::Ty(ret_ty) = &f.sig.output {
                    walk_hir_ty(ret_ty, &mut |ty| {
                        check_trait_object_ty(ty, &trait_defs, resolver, interner, errors);
                    });
                }
            }
            OwnerNode::Item(HirItem::Struct(s)) => {
                for field in &s.fields {
                    walk_hir_ty(&field.ty, &mut |ty| {
                        check_trait_object_ty(ty, &trait_defs, resolver, interner, errors);
                    });
                }
            }
            OwnerNode::Item(HirItem::Enum(e)) => {
                for variant in &e.variants {
                    match &variant.data {
                        crate::hir::HirVariantData::Tuple(fields, _) => {
                            for f in fields {
                                walk_hir_ty(&f.ty, &mut |ty| {
                                    check_trait_object_ty(
                                        ty,
                                        &trait_defs,
                                        resolver,
                                        interner,
                                        errors,
                                    );
                                });
                            }
                        }
                        crate::hir::HirVariantData::Struct(fields, _) => {
                            for f in fields {
                                walk_hir_ty(&f.ty, &mut |ty| {
                                    check_trait_object_ty(
                                        ty,
                                        &trait_defs,
                                        resolver,
                                        interner,
                                        errors,
                                    );
                                });
                            }
                        }
                        _ => {} // Stage 18.60: skip unhandled variant (no Res::Def to check)
                    }
                }
            }
            _ => {} // Stage 18.60: skip unhandled HirStmt variant
        }
    }
}

/// Helper: check a single TraitObject type for object safety.
pub(super) fn check_trait_object_ty(
    ty: &crate::hir::HirTy,
    trait_defs: &std::collections::HashMap<crate::hir::DefId, &crate::hir::HirTrait>,
    _resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    errors: &mut CompileErrors,
) {
    use crate::hir::{HirTyKind, HirTypeBound, Res};
    use crate::traits::object_safety::check_trait_object_safety;

    if let HirTyKind::TraitObject { bounds, .. } = &ty.kind {
        for bound in bounds {
            if let HirTypeBound::Trait(tc) = bound {
                if let Res::Def(trait_def_id, _) = tc.path.res {
                    if let Some(trait_def) = trait_defs.get(&trait_def_id) {
                        let violations = check_trait_object_safety(trait_def, trait_defs, interner);
                        if !violations.is_empty() {
                            let trait_name = interner
                                .try_resolve(&trait_def.ident.name)
                                .unwrap_or("<anonymous>");
                            for v in &violations {
                                errors.typeck.push(crate::typeck::TypeError::new(
                                    v.error_message(trait_name, interner),
                                    v.span(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}
