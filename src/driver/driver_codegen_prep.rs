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

/// Run post-typeck validations: trait impl coherence, method signatures,
/// struct literal fields, pattern arity, assignment targets, cast types,
/// and register builtin macro names in fn_name_by_def_id.
///
/// Per §13.4 J1-J6 (Stage 18.140): extracted from compile_inner.
pub(super) fn run_post_typeck_validations(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    errors: &mut super::CompileErrors,
    trait_resolver: &crate::traits::TraitResolver,
    fn_name_by_def_id: &mut std::collections::HashMap<crate::hir::DefId, String>,
) {
    use super::driver_validations::*;
    use crate::traits::TraitError;

    // Stage 5.22: Validate all trait impls (coherence + completeness).
    // Per deep review r70 action item: wire validate_impls() into driver.
    // Non-fatal — compilation continues, but errors are reported.
    //
    // Stage 5.80 (refactor): trait_resolver was built earlier (before the
    // per-body loop) so the DynTraitMIRPlan could be constructed from it.
    // Validation remains here — it doesn't affect lowering, only reports.
    let validation_report = trait_resolver.validate_impls();
    // Stage 15.9: Push typed TraitError values (was String). The structured
    // data (CoherenceError/IncompleteImpl) is preserved for downstream
    // consumers. format_for_user resolves the Spur symbols to &str.
    for ce in validation_report.coherence_errors {
        errors.trait_errors.push(TraitError::Coherence(ce));
    }
    for inc in validation_report.incomplete_impls {
        errors.trait_errors.push(TraitError::Incomplete(inc));
    }

    // Stage 18.71 P0-4: Validate trait impl method signatures against
    // trait declarations. Catches:
    //   - return type mismatch (trait: i32, impl: bool)
    //   - arg count mismatch (trait: 1 arg, impl: 2 args)
    //   - arg type mismatch (trait: i32, impl: bool)
    //
    // Per §1.0 原则 4 "报错 > 静默": signature mismatch must be reported.
    // Per §1.0 原则 6 "通用 > 特例": one validator covers all impl methods.
    // Per §10 naming: `validate_impl_method_signatures` follows
    //   `validate_<noun>_<noun>_<noun>` pattern.
    validate_impl_method_signatures(hir, interner, &mut errors.typeck);

    // Stage 18.72 P1-A: Validate struct literal field counts.
    // Catches:
    //   - missing field (`S { x: 1 }` where S has fields x, y)
    //   - extra field (`S { x: 1, y: 2 }` where S has only field x)
    //   - unknown field (`S { z: 1 }` where S has no field z)
    //   - duplicate field (`S { x: 1, x: 2 }`)
    //
    // Per §1.0 原则 4 "报错 > 静默": field count mismatch must be reported.
    // Per §1.0 原则 6 "通用 > 特例": one validator covers all struct literals.
    // Per §10 naming: `validate_struct_literal_fields` follows
    //   `validate_<noun>_<noun>_<noun>` pattern.
    validate_struct_literal_fields(hir, interner, &mut errors.typeck);

    // Stage 18.72 P1-C: Validate pattern arity in let bindings.
    // Catches `let (a, b, c) = (1, 2)` (3 patterns, 2 tuple elements).
    //
    // Per §1.0 原则 4 "报错 > 静默": arity mismatch must be reported.
    // Per §10 naming: `validate_pattern_arity` follows `validate_<noun>_<noun>`.
    validate_pattern_arity(hir, interner, &mut errors.typeck);

    // Stage 18.73 P1-G: Missing main check is inlined in `compile_binary`
    // (CLI path), not here in `compile` (test/library path). This avoids
    // false positives in test contexts where individual functions are
    // compiled without a `main`. See `compile_binary` at line ~1961.
    // Stage 18.78 P1-N7: `validate_main_exists` function was removed;
    // the check is now inlined in `compile_binary`.

    // Stage 18.73 P1-E: Validate assignment targets.
    // Per §1.0 原则 4 "报错 > 静默": invalid assignment target must be reported.
    // Per §10 naming: `validate_assignment_targets` follows `validate_<noun>_<noun>`.
    validate_assignment_targets(hir, interner, &mut errors.typeck);

    // Stage 18.73 P1-F: Validate cast types.
    // Per §1.0 原则 4 "报错 > 静默": invalid cast must be reported.
    // Per §10 naming: `validate_cast_types` follows `validate_<noun>_<noun>`.
    validate_cast_types(hir, interner, &mut errors.typeck);

    // Stage 18.21: Register __landin_println etc. in fn_name_by_def_id
    // so codegen can resolve the function name. The resolver returns a
    // synthetic DefId for __landin_ functions; we map each to its name.
    // Use DefId(u32::MAX - i) to avoid collisions with real DefIds.
    for (i, name) in crate::parser::macro_expand::BUILTIN_MACRO_NAMES
        .iter()
        .enumerate()
    {
        let landin_name = format!("__landin_{}", name);
        let synthetic_def_id = crate::hir::DefId::new(u32::MAX - i as u32);
        fn_name_by_def_id.insert(synthetic_def_id, landin_name);
    }
}

/// Pre-intern built-in macro names and runtime function symbols.
///
/// Per §13.4 J1-J6 (Stage 18.141): extracted from compile_inner.
pub(super) fn pre_intern_macro_symbols(interner: &mut lasso::Rodeo) {
    for name in crate::parser::macro_expand::BUILTIN_MACRO_NAMES {
        interner.get_or_intern(name);
    }
    for name in crate::parser::macro_expand::BUILTIN_MACRO_NAMES {
        interner.get_or_intern(format!("__landin_{}", name));
    }
    interner.get_or_intern("args");
    interner.get_or_intern("tt");
    interner.get_or_intern("cond");
    interner.get_or_intern("msg");
    interner.get_or_intern("x");
    interner.get_or_intern("dst");
    interner.get_or_intern("__landin_assert");
    interner.get_or_intern("__landin_panic_msg");
    interner.get_or_intern("__landin_format");
    interner.get_or_intern("__landin_dbg");
    interner.get_or_intern("__landin_write");
    interner.get_or_intern("__landin_stringify");
    interner.get_or_intern("__landin_concat");
    interner.get_or_intern("__landin_env");
    interner.get_or_intern("path");
    interner.get_or_intern("__landin_file");
    interner.get_or_intern("__landin_line");
    interner.get_or_intern("__landin_module_path");
    interner.get_or_intern("__landin_include_str");
    interner.get_or_intern("pat");
    interner.get_or_intern("cfg");
    interner.get_or_intern("__landin_matches");
    interner.get_or_intern("__landin_cfg");
    interner.get_or_intern("__landin_option_env");
    interner.get_or_intern("attr");
    interner.get_or_intern("__landin_asm");
    interner.get_or_intern("__landin_compile_error");
    interner.get_or_intern("__landin_cfg_attr");
    interner.get_or_intern("mode");
    interner.get_or_intern("__landin_unreachable");
    interner.get_or_intern("__landin_trace_macros");
    interner.get_or_intern("__landin_format_args");
}

/// Build generics_map from HIR: maps DefId to Vec<ParamTy>.
///
/// Per §13.4 J1-J6 (Stage 18.142): extracted from compile_inner.
pub(super) fn build_generics_map(
    hir: &HirCrate,
) -> std::collections::HashMap<crate::hir::DefId, Vec<crate::mir::ty::ParamTy>> {
    let mut map = std::collections::HashMap::new();
    for (def_id, _) in &hir.owners {
        let params = crate::hir::generics::find_generics(*def_id, hir);
        if !params.is_empty() {
            map.insert(*def_id, params);
        }
    }
    map
}

/// Build TraitResolver, run trait validations, and build DynTraitMIRPlan.
///
/// Per §13.4 J1-J6 (Stage 18.143): extracted from compile_inner.
/// This function:
/// 1. Creates TraitResolver and registers builtin traits + stdlib types
/// 2. Runs object safety check + where clause check (pushes errors)
/// 3. Builds DynTraitMIRPlan from the resolver
pub(super) fn build_trait_resolver_and_plan(
    hir: &HirCrate,
    interner: &mut lasso::Rodeo,
    errors: &mut super::CompileErrors,
) -> (
    crate::traits::TraitResolver,
    crate::mir::dyn_trait::DynTraitMIRPlan,
) {
    use super::driver_object_safety::check_object_safety_for_dyn_trait_usage;

    let mut trait_resolver = crate::traits::TraitResolver::new();
    trait_resolver.register_builtin_traits(interner);
    crate::stdlib::register_stdlib(interner);
    trait_resolver.collect(hir, interner);

    check_object_safety_for_dyn_trait_usage(hir, &trait_resolver, interner, errors);

    let where_errors =
        crate::typeck::where_clause::check_where_clauses(hir, &trait_resolver, interner);
    errors.typeck.extend(where_errors);

    let dyn_trait_plan =
        crate::mir::dyn_trait::build_dyn_trait_mir_plan_from_resolver(&trait_resolver, interner);
    (trait_resolver, dyn_trait_plan)
}
