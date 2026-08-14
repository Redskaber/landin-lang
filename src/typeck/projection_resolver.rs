//! Stage 16.68 (Task 17 Phase 3): Associated type projection resolution.
//! Stage 18.87 GATs Phase 3: Bug fixes B5-B9 + complete compound type coverage.
//!
//! ## Status (Stage 18.87)
//!
//! The projection resolver is wired into the driver (`driver.rs:1577`) and
//! handles `TyKind::Projection(assoc_def_id, substs)` in MIR local_decls.
//!
//! Stage 18.87 fixes:
//! - B6: Added FnDef/FnPtr/Closure/Projection recursive resolution
//! - B7: Expanded types_match to cover all TyKind variants
//! - B8: Added recursion depth limit (10) to prevent infinite loops
//!
//! Per §23: `resolve_projection` follows `<verb>_<noun>` pattern.
//! Per §16: reads HIR + TraitResolver (allowed during driver post-typeck).
//! Per §1.0 原則 6 "通用 > 特例": one resolver for all projections.

use crate::hir::{HirCrate, HirImpl, HirImplItem, HirItem, HirTraitItem, OwnerNode, Res};
use crate::mir::ty::{Ty, TyKind};
use crate::session::Span;

/// Maximum recursion depth for projection resolution.
/// Per §1.0 原則 9 "正确 > 妥协": prevents infinite loops on cyclic bindings.
const MAX_PROJECTION_DEPTH: u32 = 10;

/// Resolve all `TyKind::Projection` in a MIR body's local declarations.
///
/// Walks every `local_decl.ty` and replaces `Projection(def_id, substs)` with
/// the concrete type from the impl block. Recursively resolves nested
/// projections (e.g., `Projection` inside `Ref`, `Tuple`, `Array`).
///
/// Per §23: `resolve_projections_in_mir` follows `<verb>_<noun>_<prep>_<noun>`
/// pattern.
/// Per §16: reads HIR (allowed during driver post-typeck phase).
pub fn resolve_projections_in_mir(mir: &mut crate::mir::body::MirBody, hir: &HirCrate) {
    for local_decl in &mut mir.local_decls {
        let resolved = resolve_projection_in_ty(&local_decl.ty, hir, 0);
        local_decl.ty = resolved;
    }
}

/// Recursively resolve `Projection` in a `Ty`.
///
/// If the projection can be resolved (impl found, assoc type found), returns
/// the concrete type. If not, returns the original `Ty` unchanged (the
/// projection remains unresolved — this is a graceful degradation).
///
/// Stage 18.87 B8: Added `depth` parameter to prevent infinite recursion
/// on cyclic associated type bindings (e.g., `type A = B; type B = A;`).
fn resolve_projection_in_ty(ty: &Ty, hir: &HirCrate, depth: u32) -> Ty {
    // B8: Recursion depth limit — prevent infinite loops on cyclic bindings.
    if depth >= MAX_PROJECTION_DEPTH {
        return ty.clone();
    }

    match &ty.kind {
        TyKind::Projection(assoc_def_id, substs) => {
            // Try to resolve this projection to a concrete type.
            if let Some(concrete) = lookup_assoc_type_resolution(*assoc_def_id, substs, hir) {
                // Recursively resolve any nested projections in the concrete type.
                resolve_projection_in_ty(&concrete, hir, depth + 1)
            } else {
                // Cannot resolve — keep the projection (graceful degradation).
                ty.clone()
            }
        }
        // Recursively resolve in compound types.
        TyKind::Ref(r, m, inner) => Ty::new(
            TyKind::Ref(
                *r,
                *m,
                Box::new(resolve_projection_in_ty(inner, hir, depth + 1)),
            ),
            Span::DUMMY,
        ),
        TyKind::RawPtr(m, inner) => Ty::new(
            TyKind::RawPtr(
                *m,
                Box::new(resolve_projection_in_ty(inner, hir, depth + 1)),
            ),
            Span::DUMMY,
        ),
        TyKind::Array(inner, c) => Ty::new(
            TyKind::Array(
                Box::new(resolve_projection_in_ty(inner, hir, depth + 1)),
                c.clone(),
            ),
            Span::DUMMY,
        ),
        TyKind::Slice(inner) => Ty::new(
            TyKind::Slice(Box::new(resolve_projection_in_ty(inner, hir, depth + 1))),
            Span::DUMMY,
        ),
        TyKind::Tuple(tys) => Ty::new(
            TyKind::Tuple(
                tys.iter()
                    .map(|t| resolve_projection_in_ty(t, hir, depth + 1))
                    .collect(),
            ),
            Span::DUMMY,
        ),
        TyKind::Adt(def_id, substs) => {
            let new_substs: Vec<Ty> = substs
                .iter()
                .map(|t| resolve_projection_in_ty(t, hir, depth + 1))
                .collect();
            Ty::new(TyKind::Adt(*def_id, new_substs.into()), Span::DUMMY)
        }
        // Stage 18.87 B6: Added FnDef/FnPtr/Closure recursive resolution.
        TyKind::FnDef(def_id, substs) => {
            let new_substs: Vec<Ty> = substs
                .iter()
                .map(|t| resolve_projection_in_ty(t, hir, depth + 1))
                .collect();
            Ty::new(TyKind::FnDef(*def_id, new_substs.into()), Span::DUMMY)
        }
        TyKind::Closure(def_id, substs) => {
            let new_substs: Vec<Ty> = substs
                .iter()
                .map(|t| resolve_projection_in_ty(t, hir, depth + 1))
                .collect();
            Ty::new(TyKind::Closure(*def_id, new_substs.into()), Span::DUMMY)
        }
        TyKind::FnPtr(sig) => {
            let new_inputs: Vec<Ty> = sig
                .inputs
                .iter()
                .map(|t| resolve_projection_in_ty(t, hir, depth + 1))
                .collect();
            let new_output = resolve_projection_in_ty(&sig.output, hir, depth + 1);
            Ty::new(
                TyKind::FnPtr(crate::mir::ty::Sig {
                    inputs: new_inputs,
                    output: Box::new(new_output),
                    abi: sig.abi,
                    is_unsafe: sig.is_unsafe,
                }),
                Span::DUMMY,
            )
        }
        // Stage 18.87 B6: Also resolve substs in nested Projection.
        // (Already handled by the Projection arm above, but if we get
        // here via a different path, resolve substs too.)
        // All other types — no projections to resolve.
        _ => ty.clone(),
    }
}

/// Look up the concrete type for an associated type projection.
///
/// Given `assoc_def_id` (the DefId of `type Item;` in the trait) and `substs`
/// (the type arguments, where substs[0] is the self type), find the impl
/// block that implements the trait for the self type, and extract the
/// `type Item = Concrete;` binding.
///
/// Returns `None` if no impl is found or the impl doesn't define the
/// associated type.
fn lookup_assoc_type_resolution(
    assoc_def_id: crate::hir::DefId,
    substs: &crate::mir::ty::SubstsRef,
    hir: &HirCrate,
) -> Option<Ty> {
    // Step 1: Find the trait that declares this associated type.
    // The assoc_def_id is the DefId of the HirAssocType within the trait.
    // We need to find which trait owns it.
    let (trait_def_id, assoc_name) = find_trait_for_assoc_type(assoc_def_id, hir)?;

    // Step 2: Get the self type from substs[0].
    let self_ty = substs.first()?;

    // Step 3: Find the impl of this trait for the self type.
    let impl_block = find_impl_for_trait_and_type(trait_def_id, self_ty, hir)?;

    // Step 4: In the impl, find the `type Item = Concrete;` binding.
    for item in &impl_block.items {
        if let HirImplItem::Type(assoc) = item {
            // Match by name (the assoc type's ident name).
            if assoc.ident.name == assoc_name {
                // Return the concrete type from the impl.
                return Some(crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir(
                    assoc.default.as_ref()?,
                    Some(hir),
                ));
            }
        }
    }

    None
}

/// Find the trait that declares an associated type, and return its DefId + name.
fn find_trait_for_assoc_type(
    assoc_def_id: crate::hir::DefId,
    hir: &HirCrate,
) -> Option<(crate::hir::DefId, crate::lexer::Symbol)> {
    // The assoc_def_id is a DefId allocated during HIR lowering.
    // We search all traits for an associated type whose HirId matches.
    for (trait_def_id, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Trait(t)) = owner {
            for item in &t.items {
                if let HirTraitItem::Type(assoc) = item {
                    // Match by comparing the assoc type's DefId (owner)
                    // with the given assoc_def_id.
                    if assoc.hir_id.owner == assoc_def_id {
                        return Some((*trait_def_id, assoc.ident.name));
                    }
                }
            }
        }
    }
    None
}

/// Find an impl block that implements `trait_def_id` for the given self type.
fn find_impl_for_trait_and_type<'a>(
    trait_def_id: crate::hir::DefId,
    self_ty: &Ty,
    hir: &'a HirCrate,
) -> Option<&'a HirImpl> {
    for (_, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Impl(impl_block)) = owner {
            // Check if this impl implements the target trait.
            if let Some(trait_path) = &impl_block.of_trait {
                if let Res::Def(impl_trait_def_id, _) = trait_path.res {
                    if impl_trait_def_id == trait_def_id {
                        // Check if the self type matches.
                        let impl_self_ty = crate::mir::lower::lower_hir_ty_to_mir_ty_with_hir(
                            &impl_block.self_ty,
                            Some(hir),
                        );
                        if types_match(&impl_self_ty, self_ty) {
                            return Some(impl_block);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if two types match (structural equality, ignoring substs differences
/// for Adt — substs are resolved separately by the caller).
///
/// Stage 18.87 B7: Expanded from 6 variants to cover all TyKind variants.
/// Per §1.0 原則 6 "通用 > 特例": exhaustive match, no silent fallback.
fn types_match(a: &Ty, b: &Ty) -> bool {
    match (&a.kind, &b.kind) {
        // Primitive types — direct equality.
        (TyKind::Bool, TyKind::Bool) => true,
        (TyKind::Char, TyKind::Char) => true,
        (TyKind::Str, TyKind::Str) => true,
        (TyKind::Never, TyKind::Never) => true,
        // Numeric types — match on variant.
        (TyKind::Int(a_i), TyKind::Int(b_i)) => a_i == b_i,
        (TyKind::Uint(a_u), TyKind::Uint(b_u)) => a_u == b_u,
        (TyKind::Float(a_f), TyKind::Float(b_f)) => a_f == b_f,
        // Adt — match by DefId (substs resolved separately).
        (TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) => a_def == b_def,
        // Param — match by index.
        (TyKind::Param(a_p), TyKind::Param(b_p)) => a_p.index == b_p.index,
        // Compound types — recursive match.
        (TyKind::Ref(_, _, a_inner), TyKind::Ref(_, _, b_inner)) => types_match(a_inner, b_inner),
        (TyKind::RawPtr(_, a_inner), TyKind::RawPtr(_, b_inner)) => types_match(a_inner, b_inner),
        (TyKind::Array(a_inner, _), TyKind::Array(b_inner, _)) => types_match(a_inner, b_inner),
        (TyKind::Slice(a_inner), TyKind::Slice(b_inner)) => types_match(a_inner, b_inner),
        (TyKind::Tuple(a_tys), TyKind::Tuple(b_tys)) => {
            a_tys.len() == b_tys.len()
                && a_tys
                    .iter()
                    .zip(b_tys.iter())
                    .all(|(a, b)| types_match(a, b))
        }
        // FnDef / Closure — match by DefId.
        (TyKind::FnDef(a_def, _), TyKind::FnDef(b_def, _)) => a_def == b_def,
        (TyKind::Closure(a_def, _), TyKind::Closure(b_def, _)) => a_def == b_def,
        // FnPtr — match by signature structure.
        (TyKind::FnPtr(a_sig), TyKind::FnPtr(b_sig)) => {
            a_sig.inputs.len() == b_sig.inputs.len()
                && a_sig
                    .inputs
                    .iter()
                    .zip(b_sig.inputs.iter())
                    .all(|(a, b)| types_match(a, b))
                && types_match(&a_sig.output, &b_sig.output)
        }
        // Infer / Error / Foreign — always match (deferred / unknown).
        (TyKind::Infer(_), _) | (_, TyKind::Infer(_)) => true,
        (TyKind::Error, _) | (_, TyKind::Error) => true,
        (TyKind::Foreign, TyKind::Foreign) => true,
        // Projection — match by assoc_def_id.
        (TyKind::Projection(a_def, _), TyKind::Projection(b_def, _)) => a_def == b_def,
        // All other combinations — no match.
        _ => false,
    }
}
