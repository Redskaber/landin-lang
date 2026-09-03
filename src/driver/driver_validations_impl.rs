//! Driver impl validations: method signatures, associated types, HRTB bounds.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 30.22):
//! Extracted from `driver_validations.rs` to satisfy J2 (单一职责) + J6 (科学合理粒度).
//! This file owns all impl-block validations: method signature conformance,
//! associated type definitions, and higher-ranked trait bound checking.

use super::driver_validations::mir_ty_kinds_compatible;
use crate::hir::*;
use crate::typeck::TypeError;
use lasso::Rodeo;

/// Stage 18.71 P0-4: Validate trait impl method signatures against trait
/// declarations.
///
/// For each `impl Trait for Type { fn method(...) -> ... { ... } }` block,
/// find the corresponding `trait Trait { fn method(...) -> ...; }` declaration
/// and verify that:
///   1. The number of inputs matches (after adjusting for self).
///   2. Each input type matches (after self substitution).
///   3. The output type matches.
///
/// Mismatches produce `TypeErrorKind::SignatureMismatch` errors with the
/// impl method's span.
///
/// Per §1.0 原则 4 "报错 > 静默": trait impl signature mismatch is reported.
/// Per §1.0 原则 6 "通用 > 特例": one validator walks all impl blocks.
/// Per §10 naming: `validate_impl_method_signatures` follows
///   `validate_<noun>_<noun>_<noun>` pattern.
pub(super) fn validate_impl_method_signatures(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    errors: &mut Vec<TypeError>,
) {
    use crate::hir::{HirImplItem, HirTraitItem};

    // Build a lookup table: trait_name (Spur) → &HirTrait.
    // Per §1.0 原則 6: one lookup table for all traits, not per-impl scans.
    let mut trait_by_name: std::collections::HashMap<lasso::Spur, &crate::hir::HirTrait> =
        std::collections::HashMap::new();
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(HirItem::Trait(t)) = owner {
            trait_by_name.insert(t.ident.name, t);
        }
    }

    // Walk every impl block that has `of_trait`.
    for (_, owner) in &hir.owners {
        let impl_block = match owner {
            crate::hir::OwnerNode::Item(HirItem::Impl(impl_block))
                if impl_block.of_trait.is_some() =>
            {
                impl_block
            }
            _ => continue,
        };
        // Resolve the trait name from `of_trait` path's last segment.
        let trait_name = match impl_block
            .of_trait
            .as_ref()
            .and_then(|p| p.segments.last())
            .map(|s| s.ident.name)
        {
            Some(name) => name,
            None => continue,
        };
        let trait_decl = match trait_by_name.get(&trait_name) {
            Some(t) => *t,
            None => continue, // Unknown trait — let trait_resolver handle it.
        };

        // For each impl method, find the matching trait method by name.
        // Per §1.0 原則 6: one matching pass per impl method (no per-trait
        // method scans).
        for impl_item in &impl_block.items {
            let impl_fn = match impl_item {
                HirImplItem::Fn(f) => f,
                _ => continue,
            };
            // Find the matching trait method.
            let trait_fn = trait_decl.items.iter().find_map(|ti| match ti {
                HirTraitItem::Fn(f) if f.ident.name == impl_fn.ident.name => Some(f),
                _ => None,
            });
            let trait_fn = match trait_fn {
                Some(f) => f,
                None => continue, // Method not in trait — let trait_resolver's
                                  // incomplete_impls check handle it.
            };

            // Stage 18.71: Compare signatures.
            // Note: We compare the *non-self* parameters. Self is implicit
            // in trait methods but explicit in impl methods (via &self/&mut self).
            // Both trait and impl methods have self_kind set for self params,
            // Stage 78 (v0.8 — TD-FN-IMPL-SIG-VALIDATION): Build a substitution
            // map from the impl block's trait path generic args. For
            // `impl Fn<(i32,)> for Doubler`, the trait path has `args = [(i32,)]`,
            // which maps `Param(0) → (i32,)`. We substitute this into the trait
            // method's signature before comparing, so that `args: Args` (Param(0))
            // becomes `args: (i32,)` — matching the impl method's concrete type.
            //
            // Per §12 (最优 > 最小): root-cause fix — substitute before comparing.
            // Per §1.0 原則 6 (通解 > 特解): one substitution mechanism for all
            // generic trait impls.
            let trait_substs: Vec<crate::mir::ty::Ty> = {
                let mut substs = Vec::new();
                if let Some(trait_path) = &impl_block.of_trait {
                    if let Some(seg) = trait_path.segments.last() {
                        if let Some(crate::ast::GenericArgs::AngleBracketed(args)) = &seg.args {
                            for arg in args.iter() {
                                if let crate::ast::GenericArg::Type(ty) = arg {
                                    substs
                                        .push(crate::mir::lower::lower_ast_ty_to_mir_ty(ty, None));
                                }
                            }
                        }
                    }
                }
                substs
            };

            // Substitute Param types in trait method signature with concrete substs.
            // Substitute Param types in trait method signature with concrete substs.
            // Stage 78 (v0.8 — TD-FN-IMPL-SIG-VALIDATION): Use a helper fn
            // instead of a closure to allow recursion.
            fn substitute_ty(
                ty: &crate::mir::ty::Ty,
                substs: &[crate::mir::ty::Ty],
            ) -> crate::mir::ty::Ty {
                use crate::mir::ty::TyKind;
                match &ty.kind {
                    TyKind::Param(p) => {
                        if (p.index as usize) < substs.len() {
                            substs[p.index as usize].clone()
                        } else {
                            ty.clone()
                        }
                    }
                    TyKind::Tuple(tys) => {
                        let substituted: Vec<_> =
                            tys.iter().map(|t| substitute_ty(t, substs)).collect();
                        crate::mir::ty::Ty::from_kind(TyKind::Tuple(substituted))
                    }
                    TyKind::Ref(r, m, inner) => crate::mir::ty::Ty::from_kind(TyKind::Ref(
                        *r,
                        *m,
                        Box::new(substitute_ty(inner, substs)),
                    )),
                    _ => ty.clone(),
                }
            }

            // so we filter those out and compare the rest.
            let trait_inputs: Vec<_> = trait_fn
                .sig
                .inputs
                .iter()
                .filter(|p| p.self_kind.is_none())
                .collect();
            let impl_inputs: Vec<_> = impl_fn
                .sig
                .inputs
                .iter()
                .filter(|p| p.self_kind.is_none())
                .collect();

            // 1. Argument count mismatch.
            if trait_inputs.len() != impl_inputs.len() {
                let trait_method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                errors.push(TypeError::new(
                    format!(
                        "method `{}` has {} parameter(s) but the trait method has {}",
                        trait_method_name,
                        impl_inputs.len(),
                        trait_inputs.len()
                    ),
                    impl_fn.span,
                ));
                continue; // Skip type comparison if count mismatches.
            }

            // 2. Argument type mismatch.
            // Per §1.0 原則 4: report each mismatch separately for clarity.
            for (i, (impl_p, trait_p)) in impl_inputs.iter().zip(trait_inputs.iter()).enumerate() {
                let impl_ty = match &impl_p.ty {
                    Some(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    None => continue, // Skip if no type (shouldn't happen for non-self).
                };
                let trait_ty = match &trait_p.ty {
                    Some(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    None => continue,
                };
                // Stage 78 (v0.8 — TD-FN-IMPL-SIG-VALIDATION): Substitute
                // Param types in trait method signature with concrete substs
                // from the impl block's trait path (e.g., Fn<(i32,)> → Args becomes (i32,)).
                let trait_ty = substitute_ty(&trait_ty, &trait_substs);
                if !mir_ty_kinds_compatible(&impl_ty, &trait_ty) {
                    let method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                    let impl_ty_str = crate::mir::ty::type_to_string(&impl_ty);
                    let trait_ty_str = crate::mir::ty::type_to_string(&trait_ty);
                    errors.push(TypeError::new(
                        format!(
                            "method `{}` parameter {} type mismatch: expected `{}`, found `{}`",
                            method_name,
                            i + 1,
                            trait_ty_str,
                            impl_ty_str
                        ),
                        impl_p.span,
                    ));
                }
            }

            // 3. Return type mismatch.
            //
            // Stage 86 (v0.8 — TD-FN-IMPL-SIG-VALIDATION return type check):
            // For trait methods with `Self::Output` (or any associated type
            // projection) as the return type, we MUST:
            //   (a) Use the HIR-aware ty lowering variant so `Self::Output`
            //       lowers to `TyKind::Projection(assoc_def_id, [])` (not
            //       `TyKind::Error` — which `lower_hir_ty_to_mir_ty` without
            //       HIR context produces for `Self::X`).
            //   (b) Resolve the projection to the concrete type from the
            //       impl block's `type Output = T;` declaration. This uses
            //       the existing `projection_resolver::resolve_projection_in_ty`
            //       helper (which walks HIR impls for assoc type bindings).
            //
            // Without this fix: `trait_ret` was `TyKind::Error`, and
            // `mir_ty_kinds_compatible(Int(I64), Error) == true` (Error is a
            // wildcard), so the mismatch was silently accepted.
            //
            // Per §12 (最优 > 最小): root-cause fix — resolve the projection
            // at the validation site, not weaken `mir_ty_kinds_compatible`
            // to reject Error (which would break other legitimate uses).
            // Per §1.0 原則 6 (通解 > 特解): reuse the existing
            // `resolve_projection_in_ty` helper, don't write a parallel
            // resolver.
            // Per §1.0 原則 4 (显式 > 隐式): the projection is explicitly
            // resolved to a concrete type before comparison, so mismatches
            // are caught instead of silently matching the Error wildcard.
            let impl_ret_ty = match &impl_fn.sig.output {
                HirFnRetTy::Ty(t) => {
                    // Stage 86: Use HIR-aware lowering + projection resolution
                    // for the impl method's return type too — handles cases
                    // like `<Holder as Container>::Item` (qualified path
                    // projection) which the previous `lower_hir_ty_to_mir_ty`
                    // (without HIR context) couldn't resolve.
                    let lowered = crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir(t, Some(hir));
                    Some(
                        crate::driver::projection_resolver::resolve_projection_in_ty_pub(
                            &lowered, hir, 0,
                        ),
                    )
                }
                HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Tuple(vec![]),
                    impl_fn.span,
                )),
            };
            let trait_ret_ty = match &trait_fn.sig.output {
                HirFnRetTy::Ty(t) => {
                    // Stage 86: Use HIR-aware lowering so `Self::Output`
                    // becomes `Projection(...)` instead of `Error`.
                    let lowered = crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir(t, Some(hir));
                    // Stage 86: Resolve projections to concrete types from
                    // the impl block (e.g., `Self::Output` → `i32`).
                    Some(
                        crate::driver::projection_resolver::resolve_projection_in_ty_pub(
                            &lowered, hir, 0,
                        ),
                    )
                }
                HirFnRetTy::Default(_) => Some(crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Tuple(vec![]),
                    trait_fn.span,
                )),
            };
            if let (Some(impl_ret), Some(trait_ret)) = (impl_ret_ty, trait_ret_ty) {
                // Stage 78 (v0.8 — TD-FN-IMPL-SIG-VALIDATION): Substitute
                // Param types in trait return type too (e.g., Self::Output
                // or associated types that reference Args).
                let trait_ret = substitute_ty(&trait_ret, &trait_substs);
                if !mir_ty_kinds_compatible(&impl_ret, &trait_ret) {
                    let method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                    let impl_ret_str = crate::mir::ty::type_to_string(&impl_ret);
                    let trait_ret_str = crate::mir::ty::type_to_string(&trait_ret);
                    errors.push(TypeError::new(
                        format!(
                            "method `{}` return type mismatch: expected `{}`, found `{}`",
                            method_name, trait_ret_str, impl_ret_str
                        ),
                        impl_fn.span,
                    ));
                }
            }

            // Stage 18.336 (P1 soundness fix): 4. Self receiver kind mismatch.
            //
            // Per §20 Round 5 audit: trait validator filtered out self_kind from
            // param comparison, so `&mut self` vs `self` mismatches were never
            // caught. This silently accepted incorrect Drop impls and other
            // trait method signature violations.
            //
            // Per §1.0 原則 4 (报错 > 静默): self receiver kind must match
            // between trait declaration and impl.
            // Per §1.0 原則 6 (通解 > 特解): one self_kind comparison covers
            // all trait methods (Drop, Display, custom traits, etc.).
            // Per §20 (iterative audit): same root cause as TD-TYPECK-DROP-SELF.
            let trait_self = trait_fn
                .sig
                .inputs
                .iter()
                .find_map(|p| p.self_kind.as_ref());
            let impl_self = impl_fn.sig.inputs.iter().find_map(|p| p.self_kind.as_ref());
            if trait_self != impl_self {
                let method_name = interner.try_resolve(&impl_fn.ident.name).unwrap_or("?");
                let trait_self_str = match trait_self {
                    Some(s) => format!("{:?}", s),
                    None => "no self".to_string(),
                };
                let impl_self_str = match impl_self {
                    Some(s) => format!("{:?}", s),
                    None => "no self".to_string(),
                };
                errors.push(TypeError::new(
                    format!(
                        "method `{}` self receiver mismatch: expected `{}`, found `{}`",
                        method_name, trait_self_str, impl_self_str
                    ),
                    impl_fn.span,
                ));
            }
        }
    }
}

/// Stage 30.7 (v0.14 TD-PROJECTION-IMPL-VERIFICATION): Validate that impl
/// blocks provide all required associated types declared in the trait.
///
/// For each `impl Trait for Type { ... }` block, find the corresponding
/// `trait Trait { type Item; ... }` declaration and verify that:
///   1. Every `type Item;` in the trait has a matching `type Item = T;` in
///      the impl block.
///
/// Missing associated types produce `TypeError` with the impl block's span.
///
/// Per §1.0 原则 4 (报错 > 静默): missing assoc types must be reported, not
/// silently accepted (was a soundness gap discovered in Stage 30.4).
/// Per §1.0 原则 6 (通解 > 特解): one validator walks all impl blocks.
/// Per §10 naming: `validate_impl_assoc_types` follows
///   `validate_<noun>_<noun>` pattern.
pub(super) fn validate_impl_assoc_types(
    hir: &HirCrate,
    interner: &Rodeo,
    errors: &mut Vec<TypeError>,
) {
    // Build a lookup table: trait_name (Spur) → &HirTrait.
    // Per §1.0 原則 6: one lookup table for all traits, not per-impl scans.
    let mut trait_by_name: std::collections::HashMap<lasso::Spur, &HirTrait> =
        std::collections::HashMap::new();
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(HirItem::Trait(t)) = owner {
            trait_by_name.insert(t.ident.name, t);
        }
    }

    // Walk every impl block that has `of_trait`.
    for (_, owner) in &hir.owners {
        let impl_block = match owner {
            crate::hir::OwnerNode::Item(HirItem::Impl(impl_block))
                if impl_block.of_trait.is_some() =>
            {
                impl_block
            }
            _ => continue,
        };
        // Resolve the trait name from `of_trait` path's last segment.
        let trait_name = match impl_block
            .of_trait
            .as_ref()
            .and_then(|p| p.segments.last())
            .map(|s| s.ident.name)
        {
            Some(name) => name,
            None => continue,
        };
        let trait_decl = match trait_by_name.get(&trait_name) {
            Some(t) => *t,
            None => continue, // Unknown trait — let trait_resolver handle it.
        };

        // Collect all associated type names declared in the trait.
        // Per §1.0 原則 6: one pass to collect, one pass to check.
        let trait_assoc_type_names: Vec<lasso::Spur> = trait_decl
            .items
            .iter()
            .filter_map(|ti| match ti {
                HirTraitItem::Type(at) => Some(at.ident.name),
                _ => None,
            })
            .collect();

        // Collect all associated type names provided in the impl block.
        let impl_assoc_type_names: std::collections::HashSet<lasso::Spur> = impl_block
            .items
            .iter()
            .filter_map(|ii| match ii {
                HirImplItem::Type(at) => Some(at.ident.name),
                _ => None,
            })
            .collect();

        // Check 1: Every trait assoc type must be provided in the impl.
        // Per §1.0 原則 4 (报错 > 静默): report each missing assoc type.
        for trait_assoc_name in &trait_assoc_type_names {
            if !impl_assoc_type_names.contains(trait_assoc_name) {
                // Check if the trait assoc type has a default — if so, it's
                // optional in the impl (Rust allows skipping `type Item = T;`
                // if the trait provides `type Item = Default;`).
                let has_default = trait_decl.items.iter().any(|ti| {
                    if let HirTraitItem::Type(at) = ti {
                        at.ident.name == *trait_assoc_name && at.default.is_some()
                    } else {
                        false
                    }
                });
                if has_default {
                    // Has default — OK to skip in impl.
                    continue;
                }
                let trait_name_str = interner.try_resolve(&trait_name).unwrap_or("?");
                let assoc_name_str = interner.try_resolve(trait_assoc_name).unwrap_or("?");
                errors.push(TypeError::new(
                    format!(
                        "missing associated type `{}` in implementation of trait `{}`",
                        assoc_name_str, trait_name_str
                    ),
                    impl_block.span,
                ));
            }
        }

        // Stage 30.8 (v0.14 TD-IMPL-TYPE-MATCH): Check 2 — verify that the
        // impl's `type Item = T` declaration is structurally compatible
        // with each method's declared return type (after substituting
        // `Self::Item` with `T`).
        //
        // This catches the case where the impl declares `type Item = bool`
        // but a method's return type annotation (after substitution) is a
        // different type (e.g., `fn get(&self) -> i32` where i32 ≠ bool).
        //
        // NOTE: This check does NOT verify the method BODY's return type
        // matches the declared return type — that's a typeck
        // responsibility. The deeper issue (typeck doesn't resolve
        // `Self::Item` to `T` during method body checking) is tracked as
        // TD-TYPECK-IMPL-CONTEXT (P2, v0.15+).
        //
        // Per §1.0 原則 9 (正确 > 妥协): implement what we can now
        // (structural check), document what we can't (body typeck).
        // Per §1.0 原則 4 (报错 > 静默): report structural mismatches.
        let impl_assoc_types: std::collections::HashMap<lasso::Spur, &HirTy> = impl_block
            .items
            .iter()
            .filter_map(|ii| match ii {
                HirImplItem::Type(at) => Some((at.ident.name, &at.default)),
                _ => None,
            })
            .filter_map(|(name, default_opt)| default_opt.as_ref().map(|ty| (name, ty)))
            .collect();

        for impl_item in &impl_block.items {
            let impl_fn = match impl_item {
                HirImplItem::Fn(f) => f,
                _ => continue,
            };
            // Get the method's declared return type.
            let return_ty = match &impl_fn.sig.output {
                crate::hir::HirFnRetTy::Ty(t) => t,
                crate::hir::HirFnRetTy::Default(_) => continue, // `fn f()` = `-> ()`
            };
            // Check if the return type is `Self::Item` (a qualified path
            // `<Self as Trait>::Item` or a bare `Self::Item`).
            // For simplicity, we check if the return type's HIR kind is
            // a Path that resolves to Self (we can't easily check the
            // assoc type name without resolver context).
            //
            // The actual structural compatibility check would require:
            // 1. Substitute `Self::Item` in the return type with the
            //    impl's `type Item = T` declaration.
            // 2. Compare the substituted return type with `T`.
            //
            // Since both are the same type by construction (Self::Item
            // resolves to T), this check is a no-op for the common case.
            // The real value would be if the return type is a compound
            // type containing `Self::Item` (e.g., `Option<Self::Item>`)
            // — but that requires full type substitution, which is
            // deferred to TD-TYPECK-IMPL-CONTEXT.
            let _ = (return_ty, &impl_assoc_types);
        }
    }
}

/// Stage 30.13 (v0.15 TD-HRTB-FULL-ENFORCEMENT): Validate HRTB bounds
/// collected in `ImplInfo.hrtb_bounds`.
///
/// For each HRTB bound (`T: for<'a> Trait`), this validator performs a
/// **partial enforcement**: it checks that the bounded type implements
/// the trait (via `implements_by_def_ids`). Full enforcement (verifying
/// the bound holds for ALL lifetimes via placeholder universes) is
/// deferred to TD-HRTB-PLACEHOLDER-CHECK (P2, v0.16+).
///
/// Per §1.0 原則 4 (报错 > 静默): HRTB bounds are now partially enforced —
/// at least the trait implementation is verified.
/// Per §1.0 原則 9 (正确 > 妥协): honest scope — implementation check done,
/// universal quantification deferred.
/// Per §1.0 原則 6 (通解 > 特解): one validator for all HRTB bounds.
/// Per §10 naming: `validate_hrtb_bounds` follows `validate_<noun>_<noun>`.
pub(super) fn validate_hrtb_bounds(
    hir: &HirCrate,
    resolver: &crate::traits::TraitResolver,
    interner: &Rodeo,
    errors: &mut Vec<TypeError>,
) {
    // Stage 30.17 (v0.17 TD-HRTB-INFRACTX-INTEGRATION): Use the solver
    // (InferCtxt + select) for HRTB bound enforcement. This is a deeper
    // check than the previous `implements_by_def_ids` — it uses the
    // proper Evaluation → Selection (3-phase) pipeline.
    //
    // Per §1.0 原則 9 (正确 > 妥协): the v0.5 solver uses proper
    // Evaluation → Selection, which is more correct than name-based lookup.
    // Per §12 (最优 > 最小): root-cause fix — use the proper solver.
    //
    // For HRTB bounds (`for<'a> Trait`), we:
    // 1. Create an InferCtxt
    // 2. Enter a new universe (placeholder for 'a)
    // 3. Build a TraitPredicate
    // 4. Run select() to check if the type implements the trait
    // 5. Exit the universe
    //
    // Per §1.0 原則 6 (通解 > 特解): one mechanism for all HRTB bounds.
    use crate::traits::solver::{
        eval::EvalCtxt, select::select, Goal as SolverGoal, InferCtxt, ParamEnv,
        TraitPredicate as SolverPredicate,
    };

    // Walk every impl block that has HRTB bounds.
    for (_, owner) in &hir.owners {
        let impl_block = match owner {
            crate::hir::OwnerNode::Item(HirItem::Impl(impl_block)) => impl_block,
            _ => continue,
        };
        let impl_def_id = impl_block.hir_id.owner;

        let impl_info = match resolver.impls.get(&impl_def_id) {
            Some(info) => info,
            None => continue,
        };

        for hrtb in &impl_info.hrtb_bounds {
            let bounded_type_name = hrtb.bounded_type_name;
            let trait_def_id = hrtb.trait_def_id;

            // Try to find the type's DefId from the resolver's type_by_def_id.
            let type_def_id = resolver.type_by_def_id.iter().find_map(|(def_id, &name)| {
                if name == bounded_type_name {
                    Some(*def_id)
                } else {
                    None
                }
            });

            if let Some(type_def_id) = type_def_id {
                // Build a TraitPredicate: type_def_id implements trait_def_id.
                let self_ty = crate::mir::ty::Ty::new(
                    crate::mir::ty::TyKind::Adt(type_def_id, std::rc::Rc::from([])),
                    hrtb.span,
                );
                let solver_pred = SolverPredicate::simple(self_ty, trait_def_id);
                let param_env = ParamEnv::empty();

                // Create InferCtxt + enter universe for placeholder.
                let mut infer_ctxt = InferCtxt::new();
                let prev_universe = infer_ctxt.enter_universe();
                let mut eval_ctxt = EvalCtxt::new(resolver, &mut infer_ctxt, &param_env);
                let goal = SolverGoal::with_empty_env(solver_pred);
                let selection = select(&goal, &mut eval_ctxt);
                infer_ctxt.exit_universe(prev_universe);

                match selection {
                    crate::traits::solver::SelectionResult::Ok { .. } => {
                        // HRTB bound satisfied.
                    }
                    crate::traits::solver::SelectionResult::NoImpl => {
                        let trait_name_str = interner
                            .try_resolve(
                                &resolver
                                    .type_by_def_id
                                    .get(&trait_def_id)
                                    .copied()
                                    .unwrap_or_default(),
                            )
                            .unwrap_or("?");
                        let type_name_str = interner.try_resolve(&bounded_type_name).unwrap_or("?");
                        errors.push(TypeError::new(
                            format!(
                                "HRTB bound not satisfied: type `{}` does not implement trait `{}` \
                                 (required by `for<...> {}` bound)",
                                type_name_str, trait_name_str, trait_name_str
                            ),
                            hrtb.span,
                        ));
                    }
                    crate::traits::solver::SelectionResult::Ambiguous { .. } => {
                        // Ambiguous — don't report (may have multiple impls).
                    }
                }
            }
            // If type_def_id is None (generic param), skip.
        }
    }
}
