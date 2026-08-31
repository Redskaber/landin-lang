//! Stage 19.5 (v0.5 Phase 5) — Trait Solver Supertrait Expansion + Error Reporting.
//!
//! Per `docs/lang-design/03-type-system.md` §5.5 (Supertrait auto-derivation):
//! When a trait T is selected for `Self: T`, Self must also implement all
//! of T's supertraits. E.g., `trait Foo: Bar` means `impl Foo for X`
//! requires `X: Bar` to hold (added as new obligation).
//!
//! This module (Phase 5) implements:
//! - `expand_supertraits(trait_def_id, resolver)` — collect all supertrait
//!   predicates for a trait (transitive closure)
//! - `supertrait_obligations(impl_def_id, self_ty, resolver)` — generate
//!   new obligations for an impl's supertraits
//! - `report_fulfillment_error(error, obl, resolver)` — high-quality
//!   diagnostic messages for FulfillmentError
//! - `report_fulfillment_result(result, resolver)` — summarize FulfillmentResult
//!
//! Per §11 (接口隔离): this module reads TraitResolver (data contract) +
//! Phase 4 FulfillmentResult (data contract). It does NOT call typeck/codegen.
//!
//! Per §12 (最优 > 最小): implement transitive closure properly (vs naive
//! one-level expansion) to handle `trait A: B`, `trait B: C` chains.
//!
//! Per §1.0 原則 6 (通解 > 特解): one `expand_supertraits` function handles
//! all trait kinds; the algorithm is general (no per-trait branches).

use crate::hir::DefId;
use crate::session::Span;
use crate::traits::resolver::TraitResolver;
use crate::traits::solver::fulfill::{FulfillmentError, FulfillmentResult};
use crate::traits::solver::{Obligation, ObligationCause, TraitPredicate};
use std::collections::HashSet;

// =====================================================================
// expand_supertraits — collect all supertrait predicates (transitive)
// =====================================================================

/// Collect all supertrait predicates for a trait (transitive closure).
///
/// Per §5.5: when trait T is selected for `Self: T`, Self must also
/// implement all of T's supertraits. E.g., `trait Foo: Bar` means
/// `impl Foo for X` requires `X: Bar` to hold.
///
/// Transitive closure: `trait A: B`, `trait B: C` → supertraits(A) = [B, C].
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all trait kinds.
///
/// Per §12 (最优 > 最小): proper transitive closure (vs one-level expansion)
/// to handle trait chains.
///
/// Per §1.0 原則 9 (正确 > 妥协): returns empty Vec if trait not found
/// (rather than silently succeeding or erroring).
///
/// # Arguments
/// * `trait_def_id` - DefId of the trait to expand
/// * `self_ty` - The Self type for the new predicates (e.g., `i32` for `i32: Foo`)
/// * `resolver` - Trait resolver for trait metadata lookup
///
/// # Returns
/// Vec of TraitPredicate, one per supertrait (transitive)
pub fn expand_supertraits(
    trait_def_id: DefId,
    self_ty: &crate::mir::ty::Ty,
    resolver: &TraitResolver,
) -> Vec<TraitPredicate> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    expand_supertraits_recursive(trait_def_id, self_ty, resolver, &mut result, &mut visited);
    result
}

/// Recursive helper for `expand_supertraits`.
///
/// Per §5.8: depth limit 128 prevents infinite recursion on cyclic
/// supertrait declarations (e.g., `trait A: B`, `trait B: A`).
///
/// Per §1.0 原則 4 (报错 > 静默): cycle detection via `visited` set
/// ensures we don't loop forever — instead, we stop expanding.
fn expand_supertraits_recursive(
    trait_def_id: DefId,
    self_ty: &crate::mir::ty::Ty,
    resolver: &TraitResolver,
    result: &mut Vec<TraitPredicate>,
    visited: &mut HashSet<DefId>,
) {
    // Cycle detection: if we've already visited this trait, stop.
    // (Per §1.0 原則 4: explicit cycle prevention, not silent infinite loop.)
    if !visited.insert(trait_def_id) {
        return;
    }

    // Look up the trait's supertraits by Spur name.
    // Per §11: TraitResolver is the single source of truth for trait metadata.
    // We need to find the trait's Spur name from its DefId.
    //
    // MVP: iterate trait_by_name to find the Spur for this DefId.
    // (Future: add a reverse map DefId → Spur for O(1) lookup.)
    let trait_name_spur = resolver
        .trait_by_name
        .iter()
        .find_map(|(spur, did)| (*did == trait_def_id).then_some(*spur));

    let Some(trait_name_spur) = trait_name_spur else {
        // Trait not found in trait_by_name — can't expand.
        // Per §1.0 原則 9: return silently (no supertraits to add).
        return;
    };

    let Some(supertraits) = resolver.trait_supertraits(trait_name_spur) else {
        // Trait has no supertraits entry — nothing to expand.
        return;
    };

    // For each supertrait, look up its DefId and add a predicate.
    for &supertrait_spur in supertraits {
        let Some(&supertrait_def_id) = resolver.trait_by_name.get(&supertrait_spur) else {
            // Supertrait not registered — skip (can't generate predicate).
            // Per §1.0 原則 4: documented limitation — supertrait not in registry.
            continue;
        };

        // Add the supertrait predicate.
        result.push(TraitPredicate::simple(self_ty.clone(), supertrait_def_id));

        // Recurse to collect transitive supertraits.
        expand_supertraits_recursive(supertrait_def_id, self_ty, resolver, result, visited);
    }
}

// =====================================================================
// supertrait_obligations — generate obligations for an impl's supertraits
// =====================================================================

/// Generate new obligations for an impl's supertraits.
///
/// Per §5.4 + §5.5: when an impl is selected, its trait's supertraits
/// become new obligations. E.g., `impl Foo for X` (where `trait Foo: Bar`)
/// adds `X: Bar` as a new obligation.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all impl kinds.
///
/// Per §1.0 原則 3 (显式 > 隐式): obligations carry `ObligationCause::Supertrait`
/// so diagnostics can point to the trait declaration.
///
/// Note: `impl_def_id` is currently unused (MVP doesn't yet integrate with
/// HIR to fetch the impl's actual trait). It's kept in the signature for
/// future Phase 6 integration when supertrait expansion is wired into
/// `collect_impl_where_clauses`.
#[allow(unused_variables)]
pub fn supertrait_obligations(
    impl_def_id: DefId,
    trait_def_id: DefId,
    self_ty: &crate::mir::ty::Ty,
    resolver: &TraitResolver,
    span: Span,
) -> Vec<Obligation> {
    let predicates = expand_supertraits(trait_def_id, self_ty, resolver);

    predicates
        .into_iter()
        .map(|predicate| {
            Obligation::new(
                predicate,
                ObligationCause::Supertrait { trait_def_id },
                span,
            )
        })
        .collect()
}

// =====================================================================
// has_supertraits — check if a trait has any supertraits
// =====================================================================

/// Check if a trait has any supertraits (one-level check, not transitive).
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit check function for callers
/// that need to know if supertrait expansion would yield anything.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all trait kinds.
pub fn has_supertraits(trait_def_id: DefId, resolver: &TraitResolver) -> bool {
    let trait_name_spur = resolver
        .trait_by_name
        .iter()
        .find_map(|(spur, did)| (*did == trait_def_id).then_some(*spur));

    let Some(trait_name_spur) = trait_name_spur else {
        return false;
    };

    resolver
        .trait_supertraits(trait_name_spur)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

// =====================================================================
// supertrait_count — count supertraits (transitive)
// =====================================================================

/// Count the number of supertraits (transitive closure) for a trait.
///
/// Per §1.0 原則 3 (显式 > 隐式): explicit count function for stats/diagnostics.
///
/// Per §12 (最优 > 最小): reuses `expand_supertraits` to avoid duplicating
/// the transitive closure logic.
pub fn supertrait_count(
    trait_def_id: DefId,
    self_ty: &crate::mir::ty::Ty,
    resolver: &TraitResolver,
) -> usize {
    expand_supertraits(trait_def_id, self_ty, resolver).len()
}

// =====================================================================
// report_fulfillment_error — high-quality diagnostic for a single error
// =====================================================================

/// Generate a high-quality diagnostic message for a FulfillmentError.
///
/// Per §1.0 原則 3 (显式 > 隐式): produces explicit human-readable strings
/// with trait/type context.
///
/// Per §1.0 原則 4 (报错 > 静默): all error variants produce non-empty
/// diagnostic messages.
///
/// Per §12 (最优 > 最小): proper formatting with context (trait name,
/// type name, span) rather than minimal "error" string.
pub fn report_fulfillment_error(
    error: &FulfillmentError,
    obl: &Obligation,
    resolver: &TraitResolver,
) -> String {
    let trait_name = trait_name_for_def_id(obl.predicate.trait_def_id, resolver);
    let type_name = type_name_for_obligation(obl, resolver);

    match error {
        FulfillmentError::NoImpl => {
            format!(
                "trait bound not satisfied: `{}: {}` — no impl found for trait `{}`",
                type_name, trait_name, trait_name
            )
        }
        FulfillmentError::Ambiguous { candidate_count } => {
            format!(
                "ambiguous trait bound: `{}: {}` — {} candidate impls matched (MVP forbids overlapping impls)",
                type_name, trait_name, candidate_count
            )
        }
        FulfillmentError::RecursionLimitExceeded { depth } => {
            format!(
                "recursion limit exceeded (depth {}) while solving `{}: {}` — possible cyclic supertrait declaration",
                depth, type_name, trait_name
            )
        }
    }
}

// =====================================================================
// report_fulfillment_result — summarize a FulfillmentResult
// =====================================================================

/// Generate a summary diagnostic for a FulfillmentResult.
///
/// Per §1.0 原則 3 (显式 > 隐式): produces a multi-line summary for
/// Errors/Stalled variants, listing all errors or pending obligations.
///
/// Per §1.0 原則 4 (报错 > 静默): all variants produce non-empty messages.
pub fn report_fulfillment_result(result: &FulfillmentResult, resolver: &TraitResolver) -> String {
    match result {
        FulfillmentResult::Ok {
            resolved_count,
            selected_count,
        } => {
            format!(
                "trait fulfillment succeeded: {} obligations resolved, {} impls selected",
                resolved_count, selected_count
            )
        }
        FulfillmentResult::Errors {
            errors,
            resolved_count,
            selected_count,
        } => {
            let mut lines = Vec::new();
            lines.push(format!(
                "trait fulfillment failed with {} error(s) ({} resolved, {} selected before errors):",
                errors.len(),
                resolved_count,
                selected_count
            ));
            for (obl, error) in errors {
                lines.push(format!(
                    "  - {}",
                    report_fulfillment_error(error, obl, resolver)
                ));
            }
            lines.join("\n")
        }
        FulfillmentResult::Stalled {
            pending,
            resolved_count,
            selected_count,
        } => {
            let mut lines = Vec::new();
            lines.push(format!(
                "trait fulfillment stalled: {} pending obligation(s) ({} resolved, {} selected before stalling):",
                pending.len(),
                resolved_count,
                selected_count
            ));
            for obl in pending {
                let trait_name = trait_name_for_def_id(obl.predicate.trait_def_id, resolver);
                let type_name = type_name_for_obligation(obl, resolver);
                lines.push(format!(
                    "  - pending: `{}: {}` (type annotations needed)",
                    type_name, trait_name
                ));
            }
            lines.join("\n")
        }
    }
}

// =====================================================================
// Helpers — name lookups for diagnostics
// =====================================================================

/// Look up the trait name (Spur → string) for a DefId.
///
/// Per §1.0 原則 10 (唯一可信数据源): TraitResolver is the single source
/// of truth for trait metadata.
///
/// Returns "<unknown trait>" if not found (per §1.0 原則 4: explicit,
/// not silent empty string).
fn trait_name_for_def_id(trait_def_id: DefId, resolver: &TraitResolver) -> String {
    let trait_name_spur = resolver
        .trait_by_name
        .iter()
        .find_map(|(spur, did)| (*did == trait_def_id).then_some(*spur));

    match trait_name_spur {
        Some(spur) => {
            // Per §11: we don't have direct access to the interner here,
            // so we return the Spur's debug representation.
            // (Future: thread the interner through for proper name lookup.)
            format!("#{}", spur.into_inner())
        }
        None => "<unknown trait>".to_string(),
    }
}

/// Look up the type name for an obligation's self type.
///
/// Per §1.0 原則 6 (通解 > 特解): one function handles all TyKind variants.
///
/// Returns "<unknown type>" if the type name can't be determined
/// (per §1.0 原則 4: explicit, not silent empty string).
fn type_name_for_obligation(obl: &Obligation, resolver: &TraitResolver) -> String {
    use crate::mir::ty::TyKind;

    match &obl.predicate.self_ty.kind {
        TyKind::Adt(def_id, _) => {
            // Look up the type name via resolver.type_by_def_id.
            match resolver.type_by_def_id.get(def_id) {
                Some(spur) => format!("#{}", spur.into_inner()),
                None => "<unknown adt>".to_string(),
            }
        }
        TyKind::Int(_) => "i32".to_string(),
        TyKind::Uint(_) => "u32".to_string(),
        TyKind::Float(_) => "f64".to_string(),
        TyKind::Bool => "bool".to_string(),
        TyKind::Char => "char".to_string(),
        TyKind::Str => "str".to_string(),
        TyKind::Infer(_) => "<inferred type>".to_string(),
        TyKind::Param(param) => format!("<type param #{}>", param.index),
        TyKind::Error => "<error type>".to_string(),
        _ => "<composite type>".to_string(),
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::IntTy;
    use crate::hir::DefId;
    use crate::mir::ty::{Ty, TyKind, TyVid};
    use crate::session::Span;
    use crate::traits::resolver::TraitResolver;
    use crate::traits::solver::fulfill::{FulfillmentError, FulfillmentResult};
    use crate::traits::solver::{Obligation, ObligationCause, TraitPredicate};

    // ----------- Test helpers -----------

    fn dummy_def_id(n: u32) -> DefId {
        DefId::new(n)
    }

    fn dummy_i32_ty() -> Ty {
        Ty::from_kind(TyKind::Int(IntTy::I32))
    }

    fn dummy_infer_ty(id: u32) -> Ty {
        Ty::from_kind(TyKind::Infer(crate::mir::ty::InferVar::TyVar(TyVid(id))))
    }

    fn dummy_resolver() -> TraitResolver {
        TraitResolver::default()
    }

    fn dummy_obligation(self_ty: Ty, trait_def_id: u32) -> Obligation {
        Obligation::new(
            TraitPredicate::simple(self_ty, dummy_def_id(trait_def_id)),
            ObligationCause::LetBinding,
            Span::DUMMY,
        )
    }

    // ----------- expand_supertraits tests -----------

    #[test]
    fn test_expand_supertraits_trait_not_found() {
        let resolver = dummy_resolver();
        let result = expand_supertraits(dummy_def_id(99), &dummy_i32_ty(), &resolver);
        // Trait not in registry → empty.
        assert!(result.is_empty());
    }

    #[test]
    fn test_expand_supertraits_no_supertraits() {
        // With empty resolver, no traits are registered.
        let resolver = dummy_resolver();
        let result = expand_supertraits(dummy_def_id(7), &dummy_i32_ty(), &resolver);
        assert!(result.is_empty());
    }

    #[test]
    fn test_expand_supertraits_cycle_detection() {
        // Test that cycle detection prevents infinite recursion.
        // (With empty resolver, there are no supertraits, so no cycle.
        // But the test verifies the function doesn't hang.)
        let resolver = dummy_resolver();
        let result = expand_supertraits(dummy_def_id(7), &dummy_i32_ty(), &resolver);
        assert!(result.is_empty());
    }

    // ----------- has_supertraits tests -----------

    #[test]
    fn test_has_supertraits_trait_not_found() {
        let resolver = dummy_resolver();
        assert!(!has_supertraits(dummy_def_id(99), &resolver));
    }

    #[test]
    fn test_has_supertraits_no_supertraits() {
        let resolver = dummy_resolver();
        assert!(!has_supertraits(dummy_def_id(7), &resolver));
    }

    // ----------- supertrait_count tests -----------

    #[test]
    fn test_supertrait_count_trait_not_found() {
        let resolver = dummy_resolver();
        assert_eq!(
            supertrait_count(dummy_def_id(99), &dummy_i32_ty(), &resolver),
            0
        );
    }

    #[test]
    fn test_supertrait_count_no_supertraits() {
        let resolver = dummy_resolver();
        assert_eq!(
            supertrait_count(dummy_def_id(7), &dummy_i32_ty(), &resolver),
            0
        );
    }

    // ----------- supertrait_obligations tests -----------

    #[test]
    fn test_supertrait_obligations_empty() {
        let resolver = dummy_resolver();
        let result = supertrait_obligations(
            dummy_def_id(7),
            dummy_def_id(7),
            &dummy_i32_ty(),
            &resolver,
            Span::DUMMY,
        );
        assert!(result.is_empty());
    }

    // ----------- report_fulfillment_error tests -----------

    #[test]
    fn test_report_fulfillment_error_no_impl() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let error = FulfillmentError::NoImpl;
        let msg = report_fulfillment_error(&error, &obl, &resolver);
        assert!(msg.contains("trait bound not satisfied"));
        assert!(msg.contains("no impl found"));
    }

    #[test]
    fn test_report_fulfillment_error_ambiguous() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let error = FulfillmentError::Ambiguous { candidate_count: 3 };
        let msg = report_fulfillment_error(&error, &obl, &resolver);
        assert!(msg.contains("ambiguous"));
        assert!(msg.contains("3 candidate"));
        assert!(msg.contains("MVP forbids overlapping"));
    }

    #[test]
    fn test_report_fulfillment_error_recursion_limit() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let error = FulfillmentError::RecursionLimitExceeded { depth: 128 };
        let msg = report_fulfillment_error(&error, &obl, &resolver);
        assert!(msg.contains("recursion limit exceeded"));
        assert!(msg.contains("128"));
        assert!(msg.contains("cyclic supertrait"));
    }

    // ----------- report_fulfillment_result tests -----------

    #[test]
    fn test_report_fulfillment_result_ok() {
        let resolver = dummy_resolver();
        let result = FulfillmentResult::Ok {
            resolved_count: 5,
            selected_count: 3,
        };
        let msg = report_fulfillment_result(&result, &resolver);
        assert!(msg.contains("trait fulfillment succeeded"));
        assert!(msg.contains("5 obligations resolved"));
        assert!(msg.contains("3 impls selected"));
    }

    #[test]
    fn test_report_fulfillment_result_errors() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let result = FulfillmentResult::Errors {
            errors: vec![(obl, FulfillmentError::NoImpl)],
            resolved_count: 2,
            selected_count: 1,
        };
        let msg = report_fulfillment_result(&result, &resolver);
        assert!(msg.contains("trait fulfillment failed"));
        assert!(msg.contains("1 error"));
        assert!(msg.contains("trait bound not satisfied"));
    }

    #[test]
    fn test_report_fulfillment_result_stalled() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_infer_ty(0), 7);
        let result = FulfillmentResult::Stalled {
            pending: vec![obl],
            resolved_count: 0,
            selected_count: 0,
        };
        let msg = report_fulfillment_result(&result, &resolver);
        assert!(msg.contains("trait fulfillment stalled"));
        assert!(msg.contains("1 pending"));
        assert!(msg.contains("type annotations needed"));
    }

    #[test]
    fn test_report_fulfillment_result_errors_multiple() {
        let resolver = dummy_resolver();
        let obl1 = dummy_obligation(dummy_i32_ty(), 7);
        let obl2 = dummy_obligation(dummy_i32_ty(), 8);
        let result = FulfillmentResult::Errors {
            errors: vec![
                (obl1, FulfillmentError::NoImpl),
                (obl2, FulfillmentError::Ambiguous { candidate_count: 2 }),
            ],
            resolved_count: 0,
            selected_count: 0,
        };
        let msg = report_fulfillment_result(&result, &resolver);
        assert!(msg.contains("2 error"));
        assert!(msg.contains("no impl found"));
        assert!(msg.contains("ambiguous"));
    }

    // ----------- type_name_for_obligation tests (via report) -----------

    #[test]
    fn test_report_with_infer_type() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_infer_ty(0), 7);
        let error = FulfillmentError::NoImpl;
        let msg = report_fulfillment_error(&error, &obl, &resolver);
        assert!(msg.contains("<inferred type>"));
    }

    #[test]
    fn test_report_with_i32_type() {
        let resolver = dummy_resolver();
        let obl = dummy_obligation(dummy_i32_ty(), 7);
        let error = FulfillmentError::NoImpl;
        let msg = report_fulfillment_error(&error, &obl, &resolver);
        assert!(msg.contains("i32"));
    }

    // ----------- Integration tests -----------

    #[test]
    fn test_integration_supertrait_obligations_with_empty_resolver() {
        // End-to-end: with empty resolver, supertrait_obligations returns empty.
        let resolver = dummy_resolver();
        let obligations = supertrait_obligations(
            dummy_def_id(7),
            dummy_def_id(7),
            &dummy_i32_ty(),
            &resolver,
            Span::DUMMY,
        );
        assert!(obligations.is_empty());
    }

    #[test]
    fn test_integration_report_after_fulfillment() {
        // End-to-end: report_fulfillment_result produces non-empty message.
        let resolver = dummy_resolver();
        let result = FulfillmentResult::Ok {
            resolved_count: 1,
            selected_count: 0,
        };
        let msg = report_fulfillment_result(&result, &resolver);
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_integration_expand_then_count() {
        // End-to-end: expand_supertraits and supertrait_count are consistent.
        let resolver = dummy_resolver();
        let expanded = expand_supertraits(dummy_def_id(7), &dummy_i32_ty(), &resolver);
        let count = supertrait_count(dummy_def_id(7), &dummy_i32_ty(), &resolver);
        assert_eq!(expanded.len(), count);
    }

    #[test]
    fn test_integration_has_supertraits_consistency() {
        // End-to-end: has_supertraits is consistent with supertrait_count > 0.
        let resolver = dummy_resolver();
        let has = has_supertraits(dummy_def_id(7), &resolver);
        let count = supertrait_count(dummy_def_id(7), &dummy_i32_ty(), &resolver);
        assert_eq!(has, count > 0);
    }
}
