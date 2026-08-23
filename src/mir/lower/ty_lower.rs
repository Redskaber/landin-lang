//! HIR/AST type → MIR type lowering.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.129):
//! Extracted from `mod.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all `lower_hir_ty_to_mir_ty*` and `lower_ast_ty_to_mir_ty*`
//! functions, plus their helpers (`lookup_type_def_id_by_name`,
//! `lower_qualified_path_to_projection`, etc.).
//!
//! ## Sub-responsibility
//! Type lowering: convert HIR `HirTy` and AST `ast::Ty` into MIR `Ty`,
//! handling generics, regions/lifetimes, qualified paths, and self param.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (type lowering)
//! - J3: no circular deps (functions called from mod.rs + siblings; ty_lower calls none back)
//! - J4: type lowering sub-responsibility is complete in this file
//! - J5: stays within mir::lower stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use crate::ast;
use crate::hir::*;
use crate::mir::ty::*;
use crate::session::Span;

/// Best-effort const-eval for array length expressions.
///
/// Stage 2.4c only handles literal integer expressions (e.g., `[T; 4]`).
/// Full const-eval (including const fns, paths, arithmetic) is Stage 3+.
/// If the expression is not a literal, falls back to `ConstVal::Uint(0)`
/// with `Ty::Error` to signal that the length couldn't be evaluated
/// (the type checker will flag the array as ill-typed).
fn const_eval_array_len(expr: &HirExpr, span: Span) -> Const {
    match &expr.kind {
        HirExprKind::Lit(HirLitKind::Int(n, _)) => Const {
            ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), span),
            val: ConstVal::Uint(*n),
        },
        HirExprKind::Lit(HirLitKind::Uint(n, _)) => Const {
            ty: Ty::new(TyKind::Uint(ast::UintTy::Usize), span),
            val: ConstVal::Uint(*n),
        },
        // Non-literal: emit an Error-typed const so typeck flags it.
        _ => Const {
            ty: Ty::new(TyKind::Error, span),
            val: ConstVal::Uint(0),
        },
    }
}

pub(crate) fn lower_hir_ty_to_mir_ty_with_lifetimes(
    ty: &HirTy,
    region_counter: &mut u32,
    lifetime_map: &mut std::collections::HashMap<crate::lexer::Symbol, crate::mir::ty::RegionVid>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    // Stage 18.57: Use the HIR Ty's span instead of Span::DUMMY.
    let span = ty.span;
    match &ty.kind {
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(lt) => {
                    // Explicit lifetime — look up or create vid.
                    let name = lt.ident.name;
                    if let Some(&existing_vid) = lifetime_map.get(&name) {
                        Region::Var(existing_vid)
                    } else {
                        let vid = *region_counter;
                        *region_counter += 1;
                        let rvid = RegionVid(vid);
                        lifetime_map.insert(name, rvid);
                        Region::Var(rvid)
                    }
                }
                None => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(
                    mir_region,
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_lifetimes(
                        inner,
                        region_counter,
                        lifetime_map,
                        generic_params,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| {
                        lower_hir_ty_to_mir_ty_with_lifetimes(
                            t,
                            region_counter,
                            lifetime_map,
                            generic_params,
                        )
                    })
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty_with_lifetimes(
                inner,
                region_counter,
                lifetime_map,
                generic_params,
            ))),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_lifetimes(
                        inner,
                        region_counter,
                        lifetime_map,
                        generic_params,
                    )),
                    Box::new(len_const),
                ),
                span,
            )
        }
        // Delegate to the non-lifetime variant for types without Ref.
        // Stage 18.105: Pass generic_params so bare type params resolve.
        _ => lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
            ty,
            region_counter,
            None,
            generic_params,
        ),
    }
}

/// Lower a HIR type to a MIR type.
/// Stage 16.51 (Task 11 Phase 1b): Lower generic args from a HIR path into
/// a MIR `SubstsRef`.
///
/// Walks `path.segments.last().args` (if any), extracts `GenericArg::Type`
/// args, lowers each to a MIR `Ty`, and collects into `SubstsRef`.
/// Lifetime and associated type args are skipped (not yet supported).
///
/// Returns an empty `SubstsRef` if the path has no generic args.
///
/// Stage 16.56 (Task 11 Phase 4b prerequisite): Now accepts `hir` parameter
/// to resolve nested generic type paths (e.g., `Box` in `Box<Box<i32>>`).
/// When `hir` is `Some`, AST paths in generic args are resolved to DefIds
/// by scanning HIR owners for matching type names. When `hir` is `None`,
/// unresolved paths produce `Error` (same as before).
///
/// Per §23: `lower_path_generic_args` follows `<verb>_<noun>_<adj>_<noun>`
/// pattern.
/// Per §16: reads HIR (path.args) during MIR lowering.
/// Stage 18.105 (S6 fix): Lower a HIR path's generic args to MIR substs,
/// with generics context for resolving bare type parameters.
///
/// When lowering a generic arg like `T` (a bare type parameter), this function
/// checks if `T` matches one of the `generic_params` (the type parameters of
/// the item being lowered). If so, it produces `Param(N)` instead of `Error`.
///
/// # Parameters
///
/// - `path`: the HIR path (e.g., `Box<T>` — path is `Box`, args contain `T`)
/// - `_region_counter`: unused (kept for API compat)
/// - `hir`: optional HIR crate for nested type resolution
/// - `generic_params`: the type parameters of the item being lowered
///   (e.g., for `fn make_box<T>(x: T) -> Box<T>`, this is `[ParamTy{T, index:0}]`)
///
/// Per §23: `lower_path_generic_args` follows `<verb>_<noun>_<adj>_<noun>`
/// pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all generic arg lowering.
/// Per §2.0 原則 9 "正确 > 妥协": bare type params now resolve correctly (S6 fix).
pub(crate) fn lower_path_generic_args(
    path: &crate::hir::HirPath,
    _region_counter: &mut u32,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> crate::mir::ty::SubstsRef {
    use crate::ast::GenericArg;

    // Get the last segment's generic args (e.g., `Vec<i32>` → args on "Vec")
    let args = match path.segments.last().and_then(|s| s.args.as_ref()) {
        Some(args) => args,
        None => return Vec::new().into(),
    };

    // Extract angle-bracketed args (e.g., `<i32, bool>`)
    let arg_list = match args {
        crate::ast::GenericArgs::AngleBracketed(args) => args,
        // Parenthesized args (fn trait syntax) not yet supported
        _ => return Vec::new().into(),
    };

    // Lower each Type arg to MIR Ty, skip Lifetime and Assoc args.
    // Stage 16.56: Pass HIR to lower_ast_ty_to_mir_ty so nested generic
    // paths can be resolved (e.g., Box<Box<i32>> → inner Box resolved).
    // Stage 18.105 (S6 fix): Pass generic_params so bare type parameters
    // (e.g., `T` in `Box<T>`) resolve to Param(N) instead of Error.
    let substs: Vec<crate::mir::ty::Ty> = arg_list
        .iter()
        .filter_map(|arg| match arg {
            GenericArg::Type(ty) => Some(lower_ast_ty_to_mir_ty_with_generics(
                ty,
                hir,
                generic_params,
            )),
            _ => None, // Skip Lifetime and Assoc args
        })
        .collect();

    substs.into()
}

/// Stage 16.56: Look up a type DefId by name from HIR owners.
///
/// Scans all HIR owners for a struct or enum with the given name.
/// Returns the first match (DefId). If multiple types share the same
/// name, the first one found is returned (this is a limitation — full
/// name resolution with module paths is future work).
///
/// Per §23: `lookup_type_def_id_by_name` follows `<verb>_<noun>_<noun>`
/// _<prep>_<noun>` pattern.
/// Per §16: reads HIR (allowed during MIR lowering).
fn lookup_type_def_id_by_name(
    hir: &HirCrate,
    name: crate::lexer::Symbol,
) -> Option<crate::hir::DefId> {
    for (def_id, owner) in &hir.owners {
        match owner {
            crate::hir::OwnerNode::Item(crate::hir::HirItem::Struct(s)) if s.ident.name == name => {
                return Some(*def_id);
            }
            crate::hir::OwnerNode::Item(crate::hir::HirItem::Enum(e)) if e.ident.name == name => {
                return Some(*def_id);
            }
            _ => {}
        }
    }
    None
}

/// Stage 16.51 (Task 11 Phase 1b): Lower an AST `Ty` to a MIR `Ty`.
///
/// This is a minimal lowerer for generic type arguments (e.g., `i32` in
/// `Vec<i32>`). It handles the common cases: primitives, paths (struct/enum
/// refs), tuples, arrays, references.
///
/// Stage 16.56 (Task 11 Phase 4b prerequisite): Now accepts `hir` parameter.
/// When `hir` is `Some`, AST paths are resolved to DefIds by scanning HIR
/// owners for matching type names. This enables nested generic types like
/// `Box<Box<i32>>` where the inner `Box<i32>` is an AST path that needs
/// resolution.
///
/// Per §23: `lower_ast_ty_to_mir_ty` follows `<verb>_<noun>_<noun>_<noun>`
/// pattern.
pub(crate) fn lower_ast_ty_to_mir_ty(
    ty: &crate::ast::Ty,
    hir: Option<&HirCrate>,
) -> crate::mir::ty::Ty {
    lower_ast_ty_to_mir_ty_with_generics(ty, hir, &[])
}

/// Stage 18.105 (S6 fix): Lower an AST type to MIR type with generics context.
///
/// This is the same as `lower_ast_ty_to_mir_ty` but additionally checks if
/// a bare path name matches one of the `generic_params`. If so, it produces
/// `Param(N)` instead of `Error`.
///
/// This fixes S6: generic function return types like `Box<T>` now correctly
/// produce `Adt(Box, [Param(0)])` instead of `Adt(Box, [Error])`.
///
/// Per §23: `lower_ast_ty_to_mir_ty_with_generics` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §1.0 原則 6 "通用 > 特例": one function for all AST type lowering.
pub(crate) fn lower_ast_ty_to_mir_ty_with_generics(
    ty: &crate::ast::Ty,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> crate::mir::ty::Ty {
    use crate::ast::Ty as ATy;
    let span = crate::session::Span::DUMMY;
    match ty {
        ATy::Bool(_) => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Bool, span),
        ATy::Char(_) => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Char, span),
        ATy::Int(int_ty, _) => crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Int(*int_ty), span),
        ATy::Uint(uint_ty, _) => {
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Uint(*uint_ty), span)
        }
        ATy::Float(float_ty, _) => {
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Float(*float_ty), span)
        }
        ATy::Tuple(tys, _) => {
            let mir_tys: Vec<_> = tys
                .iter()
                .map(|t| lower_ast_ty_to_mir_ty_with_generics(t, hir, generic_params))
                .collect();
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Tuple(mir_tys), span)
        }
        ATy::Path(_, path, _) => {
            // Stage 18.105 (S6 fix): First, check if the path is a bare type
            // parameter (single-segment, name matches a generic param).
            // Per §1.0 原則 6 "通用 > 特例": check generic params before
            // falling back to struct/enum lookup.
            if path.segments.len() == 1 {
                if let Some(last_seg) = path.segments.last() {
                    let name = last_seg.ident.name;
                    // Check if this name matches a generic type parameter.
                    for param in generic_params {
                        if param.name == name {
                            return crate::mir::ty::Ty::new(
                                crate::mir::ty::TyKind::Param(*param),
                                span,
                            );
                        }
                    }
                }
            }

            // Stage 16.56: When HIR is available, try to resolve the AST path
            // to a DefId by looking up the type name in HIR owners.
            if let Some(hir_crate) = hir {
                if let Some(last_seg) = path.segments.last() {
                    if let Some(def_id) = lookup_type_def_id_by_name(hir_crate, last_seg.ident.name)
                    {
                        // Resolve the path's generic args recursively.
                        // Stage 18.105: Pass generic_params for nested bare params.
                        let inner_substs: Vec<_> = last_seg
                            .args
                            .as_ref()
                            .map(|args| match args {
                                crate::ast::GenericArgs::AngleBracketed(args) => args
                                    .iter()
                                    .filter_map(|a| match a {
                                        crate::ast::GenericArg::Type(t) => {
                                            Some(lower_ast_ty_to_mir_ty_with_generics(
                                                t,
                                                hir,
                                                generic_params,
                                            ))
                                        }
                                        _ => None,
                                    })
                                    .collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();
                        return crate::mir::ty::Ty::new(
                            crate::mir::ty::TyKind::Adt(def_id, inner_substs.into()),
                            span,
                        );
                    }
                }
            }
            // HIR not available or name not found → Error.
            crate::mir::ty::Ty::new(crate::mir::ty::TyKind::Error, span)
        }
        // For unsupported types, return Infer (will be resolved by typeck)
        _ => crate::mir::ty::Ty::new(
            crate::mir::ty::TyKind::Infer(crate::mir::ty::InferVar::TyVar(crate::mir::ty::TyVid(
                u32::MAX,
            ))),
            span,
        ),
    }
}

pub(crate) fn lower_hir_ty_to_mir_ty(ty: &HirTy) -> Ty {
    lower_hir_ty_to_mir_ty_with_hir(ty, None)
}

/// Stage 18.105 (S6 fix): Lower a HIR type to MIR type with generics context + HIR.
///
/// This is the same as `lower_hir_ty_to_mir_ty` but additionally passes
/// `generic_params` to `lower_path_generic_args` so bare type parameters
/// (e.g., `T` in `Box<T>`) resolve to `Param(N)` instead of `Error`.
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_hir_and_generics` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_hir_ty_to_mir_ty_with_hir_and_generics(
    ty: &HirTy,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
        ty,
        &mut region_counter,
        hir,
        generic_params,
    )
}

/// Stage 16.56: Lower a HIR type to MIR type with optional HIR access.
///
/// This is the preferred entry point for callers that have HIR access.
/// When `hir` is `Some`, nested generic type paths are resolved correctly
/// (e.g., `Box<Box<i32>>` → inner `Box` resolved to its DefId).
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_hir` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_hir_ty_to_mir_ty_with_hir(ty: &HirTy, hir: Option<&HirCrate>) -> Ty {
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(ty, &mut region_counter, hir, &[])
}

/// Stage 15.49 (HP-5 step 2): Lower a HIR type to MIR type with proper
/// region assignment.
///
/// Unlike `lower_hir_ty_to_mir_ty`, this function assigns a fresh
/// `Region::Var(RegionVid(n))` to each elided reference lifetime, where
/// `n` is obtained from `region_counter` (incremented per allocation).
/// This gives the region inference infrastructure real region variables
/// to work with, instead of `Region::Erased` (which maps to `'static`).
///
/// Per §23: function name follows `<verb>_<noun>_<noun>_<prep>_<noun>`
/// pattern with `_with_regions` suffix.
/// Per §1.0 原則 3 "显式 > 隐式": regions are explicit in the MIR.
pub(crate) fn lower_hir_ty_to_mir_ty_with_regions(ty: &HirTy, region_counter: &mut u32) -> Ty {
    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(ty, region_counter, None, &[])
}

/// Stage 16.56: Region-aware HIR→MIR type lowering with optional HIR access.
///
/// This is the main implementation. When `hir` is `Some`, nested generic
/// type paths are resolved correctly (e.g., `Box<Box<i32>>` → inner `Box`
/// resolved to its DefId).
///
/// Stage 18.105 (S6 fix): Added `generic_params` parameter so bare type
/// parameters (e.g., `T` in `Box<T>`) resolve to `Param(N)`.
///
/// Per §23: function name follows `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>`
/// pattern with `_with_regions_and_hir_and_generics` suffix.
fn lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
    ty: &HirTy,
    region_counter: &mut u32,
    hir: Option<&HirCrate>,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    // Stage 18.57: Use the HIR Ty's span instead of Span::DUMMY.
    // Per §1.0 原則 3 "显式 > 隐式": span is explicitly propagated from HIR.
    // Per §1.0 原則 4 "报错 > 静默": accurate spans improve diagnostics.
    let span = ty.span;
    match &ty.kind {
        HirTyKind::Bool => Ty::new(TyKind::Bool, span),
        HirTyKind::Char => Ty::new(TyKind::Char, span),
        HirTyKind::Int(int_ty) => Ty::new(TyKind::Int(*int_ty), span),
        HirTyKind::Uint(uint_ty) => Ty::new(TyKind::Uint(*uint_ty), span),
        HirTyKind::Float(float_ty) => Ty::new(TyKind::Float(*float_ty), span),
        HirTyKind::Never => Ty::new(TyKind::Never, span),
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| {
                        lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                            t,
                            region_counter,
                            hir,
                            generic_params,
                        )
                    })
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(_) => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
                None => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    Region::Var(RegionVid(vid))
                }
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(
                    mir_region,
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        inner,
                        region_counter,
                        hir,
                        generic_params,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Ptr(mutability, inner) => {
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::RawPtr(
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        inner,
                        region_counter,
                        hir,
                        generic_params,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(
                lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                    inner,
                    region_counter,
                    hir,
                    generic_params,
                ),
            )),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        inner,
                        region_counter,
                        hir,
                        generic_params,
                    )),
                    Box::new(len_const),
                ),
                span,
            )
        }
        HirTyKind::Infer => Ty::new(TyKind::Infer(InferVar::TyVar(TyVid(u32::MAX))), span),
        HirTyKind::Path(qself, path) => {
            // Stage 18.53 GATs Phase 2: If `qself.ty` is `Some`, this is a
            // qualified path `<T as Trait>::Item` — lower to `TyKind::Projection`
            // so `projection_resolver` can resolve it to the concrete type
            // from the impl block.
            //
            // Per §1.0 原則 3 "显式 > 隐式": projection is explicitly
            // represented as `TyKind::Projection(assoc_def_id, substs)`,
            // not implicitly folded into `TyKind::Adt`.
            // Per §1.0 原則 5 "去除兼容思维": the old code ignored qself
            // via `_` — that path is removed; qualified paths now produce
            // projections, plain paths produce Adt.
            if let Some(inner_ty) = &qself.ty {
                lower_qualified_path_to_projection(inner_ty, path, region_counter, hir, span)
            } else {
                // Plain path: existing behavior.
                match path.res {
                    Res::Def(def_id, _) => {
                        // Stage 16.56: Pass HIR to lower_path_generic_args so
                        // nested generic paths can be resolved.
                        // Stage 18.105 (S6 fix): Pass generic_params for bare type params.
                        let substs =
                            lower_path_generic_args(path, region_counter, hir, generic_params);
                        // Stage 18.221 (TD-GENERIC-PARAM-CHECK full fix):
                        // If the type has generic params but the user didn't
                        // supply any (substs is empty AND path has no args),
                        // this is a missing type argument (e.g., `let b: Box`).
                        // Per §1.0 原則 4 (报错>静默): must report, not silently
                        // accept.
                        //
                        // Key insight: `lower_path_generic_args` returns empty
                        // when `path.segments.last().args` is `None` (no `<>`
                        // on the type). We can check if the path had explicit
                        // generic args by examining the segment directly.
                        //
                        // Per §1.0 原則 6 (通解>特例): one check for all generic
                        // types (Box, Vec, Option, Result, etc.).
                        // Per §1.0 原則 9 (正确>妥协): only error when the type
                        // actually has generic params (non-generic types like
                        // `struct Foo` are fine without args).
                        let path_has_args = path
                            .segments
                            .last()
                            .and_then(|s| s.args.as_ref())
                            .is_some();
                        if !path_has_args && substs.is_empty() {
                            if let Some(hir_crate) = hir {
                                let expected_params =
                                    crate::hir::find_generics(def_id, hir_crate);
                                if !expected_params.is_empty() {
                                    // The type has generic params but none were
                                    // provided. This is a type error.
                                    // Per §1.0 原則 4 (报错>静默).
                                    // We return Error type so typeck catches it.
                                    // Per §1.0 原則 9 (正确>妥协): return Error,
                                    // not a silently-wrong Adt with empty substs.
                                    return Ty::new(TyKind::Error, span);
                                }
                            }
                        }
                        Ty::new(TyKind::Adt(def_id, substs), span)
                    }
                    Res::PrimTy(PrimTy::Str) => Ty::new(TyKind::Str, span),
                    // Stage 18.54: Generic type parameter (e.g., `T` in `fn f<T>(x: T)`).
                    // Lower to TyKind::Param so monomorphization can substitute it.
                    // Per §1.0 原則 6 "通用 > 特例": reuse existing ParamTy.
                    Res::GenericParam(name, idx) => {
                        let param = crate::mir::ty::ParamTy {
                            index: idx as u32,
                            name,
                        };
                        Ty::new(TyKind::Param(param), span)
                    }
                    // Stage 18.62: Res::Err/Res::Unknown/Res::Local/Res::SelfTy
                    // reaching here means the resolver couldn't resolve the type path.
                    // The resolver may have already pushed a ResolveError, but if not
                    // (e.g. Res::Unknown for body-local types), we return Error.
                    // Per §1.0 原則 4 "报错 > 静默": TyKind::Error is the fallback,
                    // and the resolver's scan_for_unresolved_paths will report it.
                    _ => Ty::new(TyKind::Error, span),
                }
            }
        }
        HirTyKind::FnPtr {
            inputs,
            output,
            abi,
            is_unsafe,
        } => {
            let mir_inputs: Vec<Ty> = inputs
                .iter()
                .map(|t| {
                    lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                        t,
                        region_counter,
                        hir,
                        generic_params,
                    )
                })
                .collect();
            let mir_output = Box::new(lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
                output,
                region_counter,
                hir,
                generic_params,
            ));
            Ty::new(
                TyKind::FnPtr(crate::mir::ty::Sig {
                    inputs: mir_inputs,
                    output: mir_output,
                    abi: *abi,
                    is_unsafe: *is_unsafe,
                }),
                span,
            )
        }
        // Stage 18.62: Unsupported HirTyKind — return Error.
        _ => Ty::new(TyKind::Error, span),
    }
}

/// Stage 18.53 GATs Phase 2: Lower a qualified path `<T as Trait>::Item` to
/// `TyKind::Projection(assoc_def_id, substs)`.
///
/// ## Algorithm
///
/// 1. Lower the inner type `T` to MIR `Ty` — this becomes `substs[0]` (self type).
/// 2. Extract the trait path from `path.segments[..qself.position]` and the
///    assoc item name from `path.segments[qself.position]` (the segment after
///    the trait).
/// 3. Look up the assoc type's `DefId` by searching traits for a matching
///    `HirAssocType`. If not found, return `TyKind::Error` (graceful
///    degradation — Phase 3 will improve this).
/// 4. Lower the path's generic args (if any) to `substs[1..]`.
/// 5. Return `TyKind::Projection(assoc_def_id, substs)`.
///
/// Per §1.0 原則 3 "显式 > 隐式": projection is explicit.
/// Per §1.0 原則 4 "报错 > 静默": if assoc type not found, return `TyKind::Error`
/// (which surfaces in typeck as an error), not a silent fallback.
/// Per §10 naming: `lower_qualified_path_to_projection` follows
/// `<verb>_<noun>_<prep>_<noun>` pattern.
pub(crate) fn lower_qualified_path_to_projection(
    inner_ty: &HirTy,
    path: &crate::hir::HirPath,
    region_counter: &mut u32,
    hir: Option<&HirCrate>,
    span: Span,
) -> Ty {
    // Step 1: Lower the inner self type T.
    let self_ty = lower_hir_ty_to_mir_ty_with_regions_and_hir_and_generics(
        inner_ty,
        region_counter,
        hir,
        &[],
    );

    // Step 2: The last segment of the path is the assoc item name.
    let assoc_segment = path.segments.last().expect(
        "qualified path must have at least one segment after `>::` — \
         parser guarantees this",
    );

    // Step 3: Stage 18.56 — Use path.res (set by resolver) as the trait DefId.
    // The resolver now validates that the assoc type exists in the trait
    // (per §1.0 原則 4 "报错 > 静默"). If res is Res::Def, the trait is valid.
    // If res is Res::Err, the resolver already emitted an error.
    let trait_def_id = match path.res {
        crate::hir::Res::Def(def_id, _) => Some(def_id),
        _ => None,
    };

    // Step 4: Lower generic args from the assoc segment (e.g., `Item<'a, T>`).
    // Per §1.0 原則 6 "通用 > 特例": reuse `lower_ast_ty_to_mir_ty` rather
    // than duplicating AST→MIR lowering.
    let mut substs: Vec<Ty> = Vec::new();
    substs.push(self_ty);
    if let Some(crate::ast::GenericArgs::AngleBracketed(arg_list)) = &assoc_segment.args {
        for arg in arg_list {
            if let crate::ast::GenericArg::Type(ty) = arg {
                substs.push(lower_ast_ty_to_mir_ty(ty, hir));
            }
            // Lifetimes in GAT projections are erased for Stage 18.55.
            // Phase 4 will handle region-aware monomorphization.
        }
    }

    match trait_def_id {
        // Stage 18.56: Use trait_def_id from resolver (soundness fix).
        // Per §1.0 原則 9 "正确 > 妥协": trait qualifier is now respected.
        Some(def_id) => Ty::new(TyKind::Projection(def_id, substs.into()), span),
        None => {
            // Resolver already emitted an error for this case.
            // Return Error so downstream typeck doesn't crash.
            Ty::new(TyKind::Error, span)
        }
    }
}

/// Stage 16.53 (Task 11 Phase 2): Lower a HIR type to MIR type with generic
/// type parameter resolution.
///
/// This is an extension of `lower_hir_ty_to_mir_ty_with_regions` that
/// resolves generic type parameters (e.g., `T` in `struct Box<T> { val: T }`)
/// to `TyKind::Param(ParamTy { index, name })`.
///
/// ## Generic Param Resolution
///
/// When the HIR path's `Res` is `Res::Err` (unresolved by the name resolver),
/// we check if the path's single segment name matches one of the `generic_params`.
/// If it matches, we produce `TyKind::Param(ParamTy { index, name })` instead
/// of `TyKind::Error`. This is the key step that makes `substitute` useful —
/// without it, generic field types would be `Error` and substitution would
/// be a no-op.
///
/// ## When to Use
///
/// Use this function when lowering types inside a generic context (e.g.,
/// struct/enum field types, generic fn signatures). Use the plain
/// `lower_hir_ty_to_mir_ty` for non-generic contexts.
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_generics` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>` pattern.
/// Per §16: reads HIR (allowed during MIR lowering).
/// Per §1.0 原則 6 "通用 > 特例": one function for all generic type lowering.
pub(crate) fn lower_hir_ty_to_mir_ty_with_generics(
    ty: &HirTy,
    generic_params: &[crate::mir::ty::ParamTy],
) -> Ty {
    let mut region_counter = 0u32;
    lower_hir_ty_to_mir_ty_with_generics_and_regions(ty, generic_params, &mut region_counter)
}

/// Stage 16.53: Region-aware variant of `lower_hir_ty_to_mir_ty_with_generics`.
///
/// Per §23: `lower_hir_ty_to_mir_ty_with_generics_and_regions` follows
/// `<verb>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` pattern.
fn lower_hir_ty_to_mir_ty_with_generics_and_regions(
    ty: &HirTy,
    generic_params: &[crate::mir::ty::ParamTy],
    region_counter: &mut u32,
) -> Ty {
    // Stage 18.57: Use the HIR Ty's span instead of Span::DUMMY.
    let span = ty.span;
    match &ty.kind {
        // For Path types, check if it's a generic type param first.
        HirTyKind::Path(_, path) => {
            // Single-segment path with unresolved Res → might be a type param.
            if path.segments.len() == 1 && matches!(path.res, Res::Err | Res::Unknown) {
                let seg_name = path.segments[0].ident.name;
                for param in generic_params {
                    if param.name == seg_name {
                        return Ty::new(TyKind::Param(*param), span);
                    }
                }
            }
            // Not a type param — delegate to the standard lowerer.
            lower_hir_ty_to_mir_ty_with_regions(ty, region_counter)
        }
        // For recursive types (Tuple, Ref, Array, etc.), recurse with generics.
        HirTyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| {
                        lower_hir_ty_to_mir_ty_with_generics_and_regions(
                            t,
                            generic_params,
                            region_counter,
                        )
                    })
                    .collect(),
            ),
            span,
        ),
        HirTyKind::Ref(region, mutability, inner) => {
            let mir_region = match region {
                Some(_) => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    crate::mir::ty::Region::Var(crate::mir::ty::RegionVid(vid))
                }
                None => {
                    let vid = *region_counter;
                    *region_counter += 1;
                    crate::mir::ty::Region::Var(crate::mir::ty::RegionVid(vid))
                }
            };
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::Ref(
                    mir_region,
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                        inner,
                        generic_params,
                        region_counter,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Ptr(mutability, inner) => {
            let mir_mut = match mutability {
                ast::Mutability::Mutable => crate::mir::ty::Mutability::Mutable,
                ast::Mutability::Immutable => crate::mir::ty::Mutability::Immutable,
            };
            Ty::new(
                TyKind::RawPtr(
                    mir_mut,
                    Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                        inner,
                        generic_params,
                        region_counter,
                    )),
                ),
                span,
            )
        }
        HirTyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                inner,
                generic_params,
                region_counter,
            ))),
            span,
        ),
        HirTyKind::Array(inner, count_expr) => {
            let len_const = const_eval_array_len(count_expr, span);
            Ty::new(
                TyKind::Array(
                    Box::new(lower_hir_ty_to_mir_ty_with_generics_and_regions(
                        inner,
                        generic_params,
                        region_counter,
                    )),
                    Box::new(len_const),
                ),
                span,
            )
        }
        // All other kinds delegate to the standard lowerer (no generics needed).
        _ => lower_hir_ty_to_mir_ty_with_regions(ty, region_counter),
    }
}
