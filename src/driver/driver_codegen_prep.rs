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
            // Stage 18.178 (TD-HEAP-ALLOC bug fix): Preserve extern fn names
            // AS-IS (no `landin_` prefix mangling). Extern fns are linked
            // against external C symbols (e.g., `__landin_alloc`, `printf`,
            // `malloc`) whose names must match exactly. Mangling them to
            // `landin___landin_alloc` would break the linker.
            //
            // Root cause: latent bug since Stage 10.3 (extern block support).
            // Extern fns were registered in the resolver (via the HirItem::Fn
            // owner path) but the codegen prep applied Landin-style name
            // mangling, producing symbols that don't exist in the C runtime.
            // Existing tests only verified compilation, not linking/calling.
            //
            // Per §1.0 原則 3 (显式>隐式): extern fn names are explicit
            // contracts with external code — preserve them literally.
            // Per §1.0 原則 6 (通解>特例): one rule for ALL extern fns (any
            // non-Landin ABI), not a special-case list of known names.
            // Per §2 原則 9 (正确>妥协): fix the root cause (preserve name
            // based on ABI), not the symptom (register __landin_* names in
            // the synthetic list).
            if f.sig.abi != crate::ast::Abi::Landin {
                // Extern fn (C ABI, System ABI, etc.) — preserve name as-is.
                fn_name_by_def_id.insert(*def_id, name.to_string());
            } else {
                // Landin ABI fn — apply standard `landin_<name>` mangling.
                let stripped = name.strip_prefix("landin_").unwrap_or(name);
                fn_name_by_def_id.insert(*def_id, format!("landin_{}", stripped));
            }
        }
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = owner {
            for impl_item in &impl_block.items {
                if let crate::hir::HirImplItem::Fn(f) = impl_item {
                    let method = interner.try_resolve(&f.ident.name).unwrap_or("fn");
                    // Stage 18.287 (TD-NEGOVERFLOW-I32 fix continuation): Handle
                    // primitive variant self_tys (`impl i32`, `impl bool`, etc.)
                    // by using `name_of_primitive_hir_ty` to get the source name.
                    // Previously, non-Path self_tys defaulted to "Self", causing
                    // `impl i32 { fn abs }` and `impl i64 { fn abs }` to both
                    // resolve to `landin_Self_abs` — a duplicate symbol crash.
                    //
                    // Per §1.0 原則 6 (通解 > 特解): one name-resolution path
                    // for both Path and primitive-variant self_tys.
                    // Per §12 (最优 > 最小): reuse `name_of_primitive_hir_ty`
                    // (already exists for method resolution).
                    let self_ty_name =
                        if let crate::hir::HirTyKind::Path(_, p) = &impl_block.self_ty.kind {
                            if let Some(seg) = p.segments.last() {
                                interner.try_resolve(&seg.ident.name).unwrap_or("Self")
                            } else {
                                "Self"
                            }
                        } else {
                            // Primitive variant (Int/Uint/Bool/Char/Float).
                            crate::mir::lower::name_of_primitive_hir_ty(&impl_block.self_ty.kind)
                                .unwrap_or("Self")
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
    user_item_count: usize,
) -> (
    crate::traits::TraitResolver,
    crate::mir::dyn_trait::DynTraitMIRPlan,
) {
    let mut trait_resolver = crate::traits::TraitResolver::new();
    trait_resolver.register_builtin_traits(interner);
    crate::stdlib::register_stdlib(interner);
    trait_resolver.collect(hir, interner, user_item_count);

    super::driver_validations_trait_object::check_object_safety_for_dyn_trait_usage(
        hir,
        &trait_resolver,
        interner,
        errors,
    );

    let where_errors =
        crate::typeck::where_clause::check_where_clauses(hir, &trait_resolver, interner);
    errors.typeck.extend(where_errors);

    let dyn_trait_plan =
        crate::mir::dyn_trait::build_dyn_trait_mir_plan_from_resolver(&trait_resolver, interner);
    (trait_resolver, dyn_trait_plan)
}

/// Build fn_sig_table entries for trait default body methods.
///
/// Per §13.4 J1-J6 (Stage 18.144): extracted from compile_inner.
pub(super) fn populate_trait_default_fn_sigs(
    hir: &HirCrate,
    interner: &lasso::Rodeo,
    fn_sig_table: &mut crate::typeck::FnSigTable,
    errors: &mut super::CompileErrors,
) {
    // Stage 14.97 (Bug Y1 fix): Also build fn_sig_table entries for trait
    // DEFAULT BODY methods. A trait default body is a method declared inside
    // a `trait T { fn f(&self) -> i32 { ... } }` block that has a body. When
    // called via static dispatch (e.g., `p.f()` where p: Pair and Pair: T),
    // codegen needs the function signature to emit the correct call.
    //
    // Strategy: For each trait method with a body, find the unique impl of
    // that trait (if any). Use the impl's self_ty as the self parameter type.
    // If multiple impls exist, use the first impl's self_ty (v0.1 limitation
    // — full monomorphization is v0.2+ work).
    //
    // Stage 14.99 (Bug Z7 fix): Emit a warning when 2+ impls exist for a trait
    // with default bodies. Per §1.0 原则 5 "报错 > 静默": the user should know
    // that the default body will be specialized for only the first impl.
    for (_, owner) in &hir.owners {
        if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Trait(t)) = owner {
            let trait_name = t.ident.name;
            // Find all impls of this trait.
            let impls: Vec<_> = hir
                .owners
                .iter()
                .filter_map(|(_, o)| {
                    if let crate::hir::OwnerNode::Item(crate::hir::HirItem::Impl(impl_block)) = o {
                        if impl_block
                            .of_trait
                            .as_ref()
                            .and_then(|p| p.segments.last().map(|s| s.ident.name))
                            == Some(trait_name)
                        {
                            return Some(impl_block);
                        }
                    }
                    None
                })
                .collect();
            // Stage 14.99 (Bug Z7 fix): Check if this trait has any default body methods.
            // If so, and if there are 2+ impls, emit a warning per §1.0 原则 5.
            //
            // Stage 14.100 (Bug AA6 fix): Refine the check — only emit the error
            // if at least one impl does NOT override the default body method.
            // If all impls override the default body, the default is never used,
            // so no specialization issue can occur.
            if impls.len() >= 2 {
                // For each trait method with a body, check if any impl doesn't override it.
                let mut any_unoverridden_default = false;
                for trait_item in &t.items {
                    if let crate::hir::HirTraitItem::Fn(default_fn) = trait_item {
                        if default_fn.body.is_none() {
                            continue;
                        }
                        // Check if every impl overrides this method.
                        let all_override = impls.iter().all(|impl_block| {
                            impl_block.items.iter().any(|impl_item| {
                                if let crate::hir::HirImplItem::Fn(impl_fn) = impl_item {
                                    impl_fn.ident.name == default_fn.ident.name
                                } else {
                                    false
                                }
                            })
                        });
                        if !all_override {
                            any_unoverridden_default = true;
                            break;
                        }
                    }
                }
                if any_unoverridden_default {
                    let trait_name_str = interner.try_resolve(&trait_name).unwrap_or("?");
                    errors.typeck.push(crate::typeck::TypeError::new(
                        format!(
                            "trait `{}` has default body methods and {} impls — \
                         v0.1 will specialize the default body using the first impl's \
                         self_ty only. Other impls will use incorrect specialization. \
                         This is a v0.1 limitation; full monomorphization is v0.2+ work. \
                         Workaround: override the default body in each impl.",
                            trait_name_str,
                            impls.len()
                        ),
                        t.span,
                    ));
                }
            }
            for trait_item in &t.items {
                if let crate::hir::HirTraitItem::Fn(f) = trait_item {
                    if f.body.is_none() {
                        continue; // No body — no fn_sig needed (it's just a declaration).
                    }
                    let method_def_id = f.hir_id.owner;
                    if fn_sig_table.sigs.contains_key(&method_def_id) {
                        continue; // Already registered (e.g., overridden in an impl).
                    }
                    // Use the first impl's self_ty as the specialization type.
                    let self_ty_opt = impls.first().map(|impl_block| {
                        crate::mir::lower::lower_hir_ty_to_mir_ty(&impl_block.self_ty)
                    });
                    let inputs: Vec<crate::mir::ty::Ty> = f
                        .sig
                        .inputs
                        .iter()
                        .map(|p| {
                            if p.self_kind.is_some() {
                                if let Some(ref self_ty) = self_ty_opt {
                                    match p.self_kind {
                                        Some(crate::ast::SelfKind::Ref(mutability)) => {
                                            let mir_mut = match mutability {
                                                crate::ast::Mutability::Mutable => {
                                                    crate::mir::ty::Mutability::Mutable
                                                }
                                                crate::ast::Mutability::Immutable => {
                                                    crate::mir::ty::Mutability::Immutable
                                                }
                                            };
                                            crate::mir::ty::Ty::new(
                                                crate::mir::ty::TyKind::Ref(
                                                    crate::mir::ty::Region::Erased,
                                                    mir_mut,
                                                    Box::new(self_ty.clone()),
                                                ),
                                                p.span,
                                            )
                                        }
                                        _ => self_ty.clone(),
                                    }
                                } else {
                                    crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, p.span)
                                }
                            } else if let Some(ty) = &p.ty {
                                crate::mir::lower::lower_hir_ty_to_mir_ty(ty)
                            } else {
                                crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, p.span)
                            }
                        })
                        .collect();
                    let output = match &f.sig.output {
                        HirFnRetTy::Default(_) => crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Tuple(Vec::new()),
                            f.span,
                        ),
                        HirFnRetTy::Ty(t) => crate::mir::lower::lower_hir_ty_to_mir_ty(t),
                    };
                    fn_sig_table.sigs.insert(
                        method_def_id,
                        crate::mir::ty::Sig {
                            inputs,
                            output: Box::new(output),
                            abi: f.sig.abi,
                            is_unsafe: f.sig.is_unsafe,
                        },
                    );
                    if crate::session::debug_codegen_enabled() {
                        let name = interner.try_resolve(&f.ident.name).unwrap_or("?");
                        eprintln!(
                        "[DRIVER] fn_sig_table: inserted trait default method_def_id={:?} name={} inputs_len={}",
                        method_def_id,
                        name,
                        fn_sig_table
                            .sigs
                            .get(&method_def_id)
                            .map(|s| s.inputs.len())
                            .unwrap_or(0)
                    );
                    }
                }
            }
        }
    }
}
