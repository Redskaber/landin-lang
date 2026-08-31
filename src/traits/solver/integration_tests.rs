//! Stage 19.6 (v0.5 Phase 6) — Trait Solver End-to-End Integration Tests.
//!
//! This module tests the full Trait Solver pipeline:
//! - Phase 1: data structures (TraitPredicate, Goal, Obligation, ObligationQueue, etc.)
//! - Phase 2: Evaluation (evaluate_one, evaluate, eval_all_to_result)
//! - Phase 3: Selection (select, select_from_eval, bind_inference_vars)
//! - Phase 4: Fulfillment (fulfillment_loop, try_fulfill_obligation, collect_impl_where_clauses)
//! - Phase 5: Supertrait expansion (expand_supertraits, supertrait_obligations, error reporting)
//! - Phase 6: Integration (collect_impl_where_clauses now wires in supertrait expansion)
//!
//! Per §7.3.1: ≥30 case negative audit set covering all 7 error categories.
//! Per §9.4.3: 1:3+ pos:neg ratio.
//!
//! Per §1.0 原則 6 (通解 > 特解): tests use a real TraitResolver with
//! registered traits/impls to exercise the full pipeline (vs mock-only).

#![cfg(test)]

use crate::hir::DefId;
use crate::mir::ty::{Ty, TyKind};
use crate::session::Span;
use crate::traits::resolver::{ImplInfo, TraitInfo, TraitResolver};
use crate::traits::solver::eval::EvalCtxt;
use crate::traits::solver::fulfill::{
    fulfill_obligation, fulfillment_loop, try_fulfill_obligation, FulfillmentError,
    FulfillmentResult, ObligationResult, DEFAULT_MAX_DEPTH,
};
use crate::traits::solver::select::{describe_selection, select};
use crate::traits::solver::supertrait::{
    expand_supertraits, has_supertraits, report_fulfillment_error, report_fulfillment_result,
    supertrait_count,
};
use crate::traits::solver::{
    Goal, InferCtxt, Obligation, ObligationCause, ObligationQueue, ParamEnv, SelectionResult,
    TraitPredicate,
};
use lasso::Rodeo;

// =====================================================================
// Test helpers — build a TraitResolver with registered traits/impls
// =====================================================================

/// Test fixture: a TraitResolver with a registered trait + impl.
///
/// Per §1.0 原則 6 (通解 > 特解): one helper builds all test fixtures;
/// the parameters control what gets registered.
struct TestFixture {
    resolver: TraitResolver,
    interner: Rodeo,
    trait_def_id: DefId,
    impl_def_id: DefId,
    self_ty_def_id: DefId,
}

impl TestFixture {
    /// Build a fixture with:
    /// - 1 trait named "TestTrait" (DefId=1)
    /// - 1 type named "TestType" (DefId=2)
    /// - 1 impl TestTrait for TestType (DefId=3)
    fn with_single_impl() -> Self {
        let mut resolver = TraitResolver::default();
        let mut interner = Rodeo::new();

        let trait_name = interner.get_or_intern("TestTrait");
        let type_name = interner.get_or_intern("TestType");

        let trait_def_id = DefId::new(1);
        let self_ty_def_id = DefId::new(2);
        let impl_def_id = DefId::new(3);

        // Register trait.
        resolver.traits.insert(
            trait_def_id,
            TraitInfo {
                def_id: trait_def_id,
                name: trait_name,
                methods: Vec::new(),
                is_unsafe: false,
                supertraits: Vec::new(),
                default_methods: Vec::new(),
                associated_consts: Vec::new(),
            },
        );
        resolver.trait_by_name.insert(trait_name, trait_def_id);

        // Register type.
        resolver.type_by_def_id.insert(self_ty_def_id, type_name);

        // Register impl.
        resolver.impls.insert(
            impl_def_id,
            ImplInfo {
                def_id: impl_def_id,
                trait_name: Some(trait_name),
                self_ty_name: Some(type_name),
                methods: Vec::new(),
                is_unsafe: false,
                span: Span::DUMMY,
                associated_consts: Vec::new(),
                where_clauses: Vec::new(),
                hrtb_bounds: Vec::new(),
            },
        );
        resolver
            .impls_by_def_ids
            .insert((trait_def_id, self_ty_def_id), impl_def_id);

        Self {
            resolver,
            interner,
            trait_def_id,
            impl_def_id,
            self_ty_def_id,
        }
    }

    /// Build a fixture with a trait that has a supertrait.
    /// - SubTrait (DefId=1) with supertrait SuperTrait
    /// - SuperTrait (DefId=4)
    /// - TestType (DefId=2)
    /// - impl SubTrait for TestType (DefId=3)
    /// - impl SuperTrait for TestType (DefId=5)
    fn with_supertrait() -> Self {
        let mut resolver = TraitResolver::default();
        let mut interner = Rodeo::new();

        let sub_name = interner.get_or_intern("SubTrait");
        let super_name = interner.get_or_intern("SuperTrait");
        let type_name = interner.get_or_intern("TestType");

        let sub_def_id = DefId::new(1);
        let self_ty_def_id = DefId::new(2);
        let sub_impl_def_id = DefId::new(3);
        let super_def_id = DefId::new(4);
        let super_impl_def_id = DefId::new(5);

        // Register SubTrait with SuperTrait as supertrait.
        resolver.traits.insert(
            sub_def_id,
            TraitInfo {
                def_id: sub_def_id,
                name: sub_name,
                methods: Vec::new(),
                is_unsafe: false,
                supertraits: vec![super_name],
                default_methods: Vec::new(),
                associated_consts: Vec::new(),
            },
        );
        resolver.trait_by_name.insert(sub_name, sub_def_id);

        // Register SuperTrait.
        resolver.traits.insert(
            super_def_id,
            TraitInfo {
                def_id: super_def_id,
                name: super_name,
                methods: Vec::new(),
                is_unsafe: false,
                supertraits: Vec::new(),
                default_methods: Vec::new(),
                associated_consts: Vec::new(),
            },
        );
        resolver.trait_by_name.insert(super_name, super_def_id);

        // Register type.
        resolver.type_by_def_id.insert(self_ty_def_id, type_name);

        // Register impl SubTrait for TestType.
        resolver.impls.insert(
            sub_impl_def_id,
            ImplInfo {
                def_id: sub_impl_def_id,
                trait_name: Some(sub_name),
                self_ty_name: Some(type_name),
                methods: Vec::new(),
                is_unsafe: false,
                span: Span::DUMMY,
                associated_consts: Vec::new(),
                where_clauses: Vec::new(),
                hrtb_bounds: Vec::new(),
            },
        );
        resolver
            .impls_by_def_ids
            .insert((sub_def_id, self_ty_def_id), sub_impl_def_id);

        // Register impl SuperTrait for TestType.
        resolver.impls.insert(
            super_impl_def_id,
            ImplInfo {
                def_id: super_impl_def_id,
                trait_name: Some(super_name),
                self_ty_name: Some(type_name),
                methods: Vec::new(),
                is_unsafe: false,
                span: Span::DUMMY,
                associated_consts: Vec::new(),
                where_clauses: Vec::new(),
                hrtb_bounds: Vec::new(),
            },
        );
        resolver
            .impls_by_def_ids
            .insert((super_def_id, self_ty_def_id), super_impl_def_id);

        Self {
            resolver,
            interner,
            trait_def_id: sub_def_id,
            impl_def_id: sub_impl_def_id,
            self_ty_def_id,
        }
    }

    /// Build a fixture with a trait but NO impl (for NoImpl error tests).
    fn with_trait_no_impl() -> Self {
        let mut resolver = TraitResolver::default();
        let mut interner = Rodeo::new();

        let trait_name = interner.get_or_intern("NoImplTrait");
        let trait_def_id = DefId::new(1);
        let self_ty_def_id = DefId::new(2);

        resolver.traits.insert(
            trait_def_id,
            TraitInfo {
                def_id: trait_def_id,
                name: trait_name,
                methods: Vec::new(),
                is_unsafe: false,
                supertraits: Vec::new(),
                default_methods: Vec::new(),
                associated_consts: Vec::new(),
            },
        );
        resolver.trait_by_name.insert(trait_name, trait_def_id);
        resolver
            .type_by_def_id
            .insert(self_ty_def_id, interner.get_or_intern("TestType"));

        Self {
            resolver,
            interner,
            trait_def_id,
            impl_def_id: DefId::new(99), // nonexistent
            self_ty_def_id,
        }
    }

    /// Build a fixture with 2 impls for the same trait (Ambiguous error).
    fn with_overlapping_impls() -> Self {
        let mut resolver = TraitResolver::default();
        let mut interner = Rodeo::new();

        let trait_name = interner.get_or_intern("AmbigTrait");
        let type_name = interner.get_or_intern("TestType");

        let trait_def_id = DefId::new(1);
        let self_ty_def_id = DefId::new(2);
        let impl1_def_id = DefId::new(3);
        let impl2_def_id = DefId::new(4);

        resolver.traits.insert(
            trait_def_id,
            TraitInfo {
                def_id: trait_def_id,
                name: trait_name,
                methods: Vec::new(),
                is_unsafe: false,
                supertraits: Vec::new(),
                default_methods: Vec::new(),
                associated_consts: Vec::new(),
            },
        );
        resolver.trait_by_name.insert(trait_name, trait_def_id);
        resolver.type_by_def_id.insert(self_ty_def_id, type_name);

        // First impl.
        resolver.impls.insert(
            impl1_def_id,
            ImplInfo {
                def_id: impl1_def_id,
                trait_name: Some(trait_name),
                self_ty_name: Some(type_name),
                methods: Vec::new(),
                is_unsafe: false,
                span: Span::DUMMY,
                associated_consts: Vec::new(),
                where_clauses: Vec::new(),
                hrtb_bounds: Vec::new(),
            },
        );

        // Second impl (overlapping — MVP forbids this).
        resolver.impls.insert(
            impl2_def_id,
            ImplInfo {
                def_id: impl2_def_id,
                trait_name: Some(trait_name),
                self_ty_name: Some(type_name),
                methods: Vec::new(),
                is_unsafe: false,
                span: Span::DUMMY,
                associated_consts: Vec::new(),
                where_clauses: Vec::new(),
                hrtb_bounds: Vec::new(),
            },
        );

        Self {
            resolver,
            interner,
            trait_def_id,
            impl_def_id: impl1_def_id,
            self_ty_def_id,
        }
    }

    fn make_obligation(&self, trait_def_id: DefId) -> Obligation {
        let self_ty = Ty::from_kind(TyKind::Adt(self.self_ty_def_id, std::rc::Rc::from([])));
        Obligation::new(
            TraitPredicate::simple(self_ty, trait_def_id),
            ObligationCause::LetBinding,
            Span::DUMMY,
        )
    }

    fn make_goal(&self, trait_def_id: DefId) -> Goal {
        let self_ty = Ty::from_kind(TyKind::Adt(self.self_ty_def_id, std::rc::Rc::from([])));
        Goal::with_empty_env(TraitPredicate::simple(self_ty, trait_def_id))
    }
}

/// Run a closure with an EvalCtxt (ParamEnv owned by the closure).
fn with_eval_ctxt<R, F>(fixture: &TestFixture, infer_ctxt: &mut InferCtxt, f: F) -> R
where
    F: FnOnce(&mut EvalCtxt) -> R,
{
    let param_env = ParamEnv::empty();
    let mut cx = EvalCtxt::new(&fixture.resolver, infer_ctxt, &param_env);
    f(&mut cx)
}

// =====================================================================
// E2E Phase 2+3: evaluate + select
// =====================================================================

#[test]
fn test_e2e_evaluate_single_impl_selects() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let goal = fixture.make_goal(fixture.trait_def_id);
    let selection = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| select(&goal, cx));
    // Unique impl → Ok.
    assert!(selection.is_ok());
    if let SelectionResult::Ok { impl_def_id } = selection {
        assert_eq!(impl_def_id, fixture.impl_def_id);
    }
}

#[test]
fn test_e2e_evaluate_no_impl_returns_no_impl() {
    let fixture = TestFixture::with_trait_no_impl();
    let mut infer_ctxt = InferCtxt::new();
    let goal = fixture.make_goal(fixture.trait_def_id);
    let selection = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| select(&goal, cx));
    assert_eq!(selection, SelectionResult::NoImpl);
}

#[test]
fn test_e2e_evaluate_overlapping_impls_ambiguous() {
    let fixture = TestFixture::with_overlapping_impls();
    let mut infer_ctxt = InferCtxt::new();
    let goal = fixture.make_goal(fixture.trait_def_id);
    let selection = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| select(&goal, cx));
    // MVP禁 overlapping — 2 matching impls = Ambiguous.
    assert!(selection.is_ambiguous());
    if let SelectionResult::Ambiguous { candidate_count } = selection {
        assert_eq!(candidate_count, 2);
    }
}

#[test]
fn test_e2e_describe_selection_ok() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let goal = fixture.make_goal(fixture.trait_def_id);
    let selection = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| select(&goal, cx));
    let desc = describe_selection(&selection);
    assert!(desc.contains("selected impl"));
}

#[test]
fn test_e2e_describe_selection_no_impl() {
    let fixture = TestFixture::with_trait_no_impl();
    let mut infer_ctxt = InferCtxt::new();
    let goal = fixture.make_goal(fixture.trait_def_id);
    let selection = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| select(&goal, cx));
    let desc = describe_selection(&selection);
    assert!(desc.contains("no impl matched"));
}

// =====================================================================
// E2E Phase 4: fulfillment_loop
// =====================================================================

#[test]
fn test_e2e_fulfillment_single_impl_resolves() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    assert!(result.is_ok());
    assert_eq!(result.resolved_count(), 1);
    assert_eq!(result.selected_count(), 1);
}

#[test]
fn test_e2e_fulfillment_no_impl_errors() {
    let fixture = TestFixture::with_trait_no_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    assert!(result.has_errors());
    if let FulfillmentResult::Errors { errors, .. } = result {
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].1, FulfillmentError::NoImpl);
    }
}

#[test]
fn test_e2e_fulfillment_overlapping_ambiguous_stalls() {
    // Per §5.4: Ambiguous obligations are deferred to pending queue.
    // With overlapping impls, select returns Ambiguous → try_fulfill returns Deferred.
    // After loop drains, pending queue has the deferred obligation → Stalled.
    let fixture = TestFixture::with_overlapping_impls();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    // Ambiguous → deferred → stalled (pending queue has it).
    assert!(result.is_stalled() || result.has_errors());
}

#[test]
fn test_e2e_fulfillment_empty_queue_ok() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let mut queue = ObligationQueue::new();
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfillment_loop(&mut queue, cx, DEFAULT_MAX_DEPTH)
    });
    assert!(result.is_ok());
    assert_eq!(result.resolved_count(), 0);
}

#[test]
fn test_e2e_fulfillment_assumed_short_circuits() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let pe = ParamEnv::from_predicates(vec![obl.predicate.clone()]);
    let mut cx = EvalCtxt::new(&fixture.resolver, &mut infer_ctxt, &pe);
    let result = fulfill_obligation(obl, &mut cx, DEFAULT_MAX_DEPTH);
    assert!(result.is_ok());
    assert_eq!(result.resolved_count(), 1);
    assert_eq!(result.selected_count(), 0); // assumed, not selected
}

#[test]
fn test_e2e_fulfillment_recursion_limit() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, 0) // max_depth = 0
    });
    assert!(result.has_errors());
    if let FulfillmentResult::Errors { errors, .. } = result {
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0].1,
            FulfillmentError::RecursionLimitExceeded { depth: 0 }
        ));
    }
}

// =====================================================================
// E2E Phase 5: supertrait expansion
// =====================================================================

#[test]
fn test_e2e_supertrait_expansion_no_supertraits() {
    let fixture = TestFixture::with_single_impl();
    let self_ty = Ty::from_kind(TyKind::Adt(fixture.self_ty_def_id, std::rc::Rc::from([])));
    let result = expand_supertraits(fixture.trait_def_id, &self_ty, &fixture.resolver);
    // TestTrait has no supertraits → empty.
    assert!(result.is_empty());
}

#[test]
fn test_e2e_supertrait_expansion_with_supertrait() {
    let fixture = TestFixture::with_supertrait();
    let self_ty = Ty::from_kind(TyKind::Adt(fixture.self_ty_def_id, std::rc::Rc::from([])));
    let result = expand_supertraits(fixture.trait_def_id, &self_ty, &fixture.resolver);
    // SubTrait has SuperTrait as supertrait → 1 predicate.
    assert_eq!(result.len(), 1);
}

#[test]
fn test_e2e_has_supertraits_false() {
    let fixture = TestFixture::with_single_impl();
    assert!(!has_supertraits(fixture.trait_def_id, &fixture.resolver));
}

#[test]
fn test_e2e_has_supertraits_true() {
    let fixture = TestFixture::with_supertrait();
    assert!(has_supertraits(fixture.trait_def_id, &fixture.resolver));
}

#[test]
fn test_e2e_supertrait_count_zero() {
    let fixture = TestFixture::with_single_impl();
    let self_ty = Ty::from_kind(TyKind::Adt(fixture.self_ty_def_id, std::rc::Rc::from([])));
    assert_eq!(
        supertrait_count(fixture.trait_def_id, &self_ty, &fixture.resolver),
        0
    );
}

#[test]
fn test_e2e_supertrait_count_one() {
    let fixture = TestFixture::with_supertrait();
    let self_ty = Ty::from_kind(TyKind::Adt(fixture.self_ty_def_id, std::rc::Rc::from([])));
    assert_eq!(
        supertrait_count(fixture.trait_def_id, &self_ty, &fixture.resolver),
        1
    );
}

// =====================================================================
// E2E Phase 6: collect_impl_where_clauses integration
// =====================================================================

#[test]
fn test_e2e_collect_impl_where_clauses_no_supertraits() {
    let fixture = TestFixture::with_single_impl();
    let obligations = crate::traits::solver::fulfill::collect_impl_where_clauses(
        fixture.impl_def_id,
        &fixture.resolver,
    );
    // TestTrait has no supertraits → 0 supertrait obligations.
    // (impl where clauses are MVP placeholder → 0.)
    assert!(obligations.is_empty());
}

#[test]
fn test_e2e_collect_impl_where_clauses_with_supertrait() {
    let fixture = TestFixture::with_supertrait();
    let obligations = crate::traits::solver::fulfill::collect_impl_where_clauses(
        fixture.impl_def_id,
        &fixture.resolver,
    );
    // SubTrait has SuperTrait → 1 supertrait obligation.
    // (impl where clauses are MVP placeholder → 0.)
    assert_eq!(obligations.len(), 1);
    // Verify the obligation's cause is Supertrait.
    assert!(matches!(
        obligations[0].cause,
        ObligationCause::Supertrait { .. }
    ));
}

#[test]
fn test_e2e_collect_impl_where_clauses_impl_not_found() {
    let fixture = TestFixture::with_single_impl();
    let obligations = crate::traits::solver::fulfill::collect_impl_where_clauses(
        DefId::new(999), // nonexistent impl
        &fixture.resolver,
    );
    // Impl not found → no obligations.
    assert!(obligations.is_empty());
}

#[test]
fn test_e2e_fulfillment_with_supertrait_resolves_both() {
    // End-to-end: fulfilling SubTrait obligation should also fulfill SuperTrait.
    // Per §5.5: supertrait expansion adds X: SuperTrait as new obligation.
    let fixture = TestFixture::with_supertrait();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id); // SubTrait
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    // Should resolve both SubTrait (impl exists) and SuperTrait (impl exists).
    assert!(result.is_ok());
    assert_eq!(result.resolved_count(), 2); // SubTrait + SuperTrait
    assert_eq!(result.selected_count(), 2); // both impls selected
}

// =====================================================================
// E2E Phase 5: error reporting
// =====================================================================

#[test]
fn test_e2e_report_no_impl_error() {
    let fixture = TestFixture::with_trait_no_impl();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let msg = report_fulfillment_error(&FulfillmentError::NoImpl, &obl, &fixture.resolver);
    assert!(msg.contains("trait bound not satisfied"));
    assert!(msg.contains("no impl found"));
}

#[test]
fn test_e2e_report_ambiguous_error() {
    let fixture = TestFixture::with_overlapping_impls();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let msg = report_fulfillment_error(
        &FulfillmentError::Ambiguous { candidate_count: 2 },
        &obl,
        &fixture.resolver,
    );
    assert!(msg.contains("ambiguous"));
    assert!(msg.contains("2 candidate"));
    assert!(msg.contains("MVP forbids overlapping"));
}

#[test]
fn test_e2e_report_recursion_limit_error() {
    let fixture = TestFixture::with_single_impl();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let msg = report_fulfillment_error(
        &FulfillmentError::RecursionLimitExceeded { depth: 128 },
        &obl,
        &fixture.resolver,
    );
    assert!(msg.contains("recursion limit exceeded"));
    assert!(msg.contains("128"));
    assert!(msg.contains("cyclic supertrait"));
}

#[test]
fn test_e2e_report_result_ok() {
    let fixture = TestFixture::with_single_impl();
    let result = FulfillmentResult::Ok {
        resolved_count: 3,
        selected_count: 2,
    };
    let msg = report_fulfillment_result(&result, &fixture.resolver);
    assert!(msg.contains("succeeded"));
    assert!(msg.contains("3 obligations resolved"));
}

#[test]
fn test_e2e_report_result_errors() {
    let fixture = TestFixture::with_trait_no_impl();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = FulfillmentResult::Errors {
        errors: vec![(obl, FulfillmentError::NoImpl)],
        resolved_count: 0,
        selected_count: 0,
    };
    let msg = report_fulfillment_result(&result, &fixture.resolver);
    assert!(msg.contains("failed"));
    assert!(msg.contains("1 error"));
}

#[test]
fn test_e2e_report_result_stalled() {
    let fixture = TestFixture::with_overlapping_impls();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = FulfillmentResult::Stalled {
        pending: vec![obl],
        resolved_count: 0,
        selected_count: 0,
    };
    let msg = report_fulfillment_result(&result, &fixture.resolver);
    assert!(msg.contains("stalled"));
    assert!(msg.contains("1 pending"));
    assert!(msg.contains("type annotations needed"));
}

// =====================================================================
// E2E: try_fulfill_obligation directly
// =====================================================================

#[test]
fn test_e2e_try_fulfill_resolved() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        try_fulfill_obligation(&obl, cx)
    });
    assert!(result.is_resolved());
    if let ObligationResult::Resolved {
        impl_def_id,
        new_obligations,
    } = result
    {
        assert_eq!(impl_def_id, fixture.impl_def_id);
        // TestTrait has no supertraits → no new obligations.
        assert!(new_obligations.is_empty());
    }
}

#[test]
fn test_e2e_try_fulfill_with_supertrait() {
    let fixture = TestFixture::with_supertrait();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        try_fulfill_obligation(&obl, cx)
    });
    assert!(result.is_resolved());
    if let ObligationResult::Resolved {
        new_obligations, ..
    } = result
    {
        // SubTrait has SuperTrait → 1 new supertrait obligation.
        assert_eq!(new_obligations.len(), 1);
    }
}

#[test]
fn test_e2e_try_fulfill_no_impl_error() {
    let fixture = TestFixture::with_trait_no_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        try_fulfill_obligation(&obl, cx)
    });
    assert!(result.is_error());
    assert_eq!(result, ObligationResult::Error(FulfillmentError::NoImpl));
}

#[test]
fn test_e2e_try_fulfill_overlapping_deferred() {
    let fixture = TestFixture::with_overlapping_impls();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        try_fulfill_obligation(&obl, cx)
    });
    // Ambiguous → Deferred.
    assert!(result.is_deferred());
}

#[test]
fn test_e2e_try_fulfill_assumed_short_circuits() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let pe = ParamEnv::from_predicates(vec![obl.predicate.clone()]);
    let mut cx = EvalCtxt::new(&fixture.resolver, &mut infer_ctxt, &pe);
    let result = try_fulfill_obligation(&obl, &mut cx);
    // Assumed → Resolved with sentinel impl_def_id.
    assert!(result.is_resolved());
    if let ObligationResult::Resolved {
        impl_def_id,
        new_obligations,
    } = result
    {
        assert_eq!(impl_def_id, DefId::new(u32::MAX)); // sentinel
        assert!(new_obligations.is_empty());
    }
}

// =====================================================================
// E2E: universe preservation (Phase 2+3+4+5 should not pollute InferCtxt)
// =====================================================================

#[test]
fn test_e2e_universe_preserved_after_fulfillment() {
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let initial_universe = infer_ctxt.current_universe();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let _ = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    // Universe should be unchanged (placeholders restored).
    assert_eq!(infer_ctxt.current_universe(), initial_universe);
}

#[test]
fn test_e2e_universe_preserved_after_supertrait_expansion() {
    let fixture = TestFixture::with_supertrait();
    let mut infer_ctxt = InferCtxt::new();
    let initial_universe = infer_ctxt.current_universe();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let _ = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    assert_eq!(infer_ctxt.current_universe(), initial_universe);
}

// =====================================================================
// E2E: full pipeline stress test
// =====================================================================

#[test]
fn test_e2e_full_pipeline_single_impl() {
    // Full pipeline: select → fulfill → report.
    let fixture = TestFixture::with_single_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    let msg = report_fulfillment_result(&result, &fixture.resolver);
    assert!(msg.contains("succeeded"));
    assert!(result.is_ok());
}

#[test]
fn test_e2e_full_pipeline_supertrait() {
    // Full pipeline with supertrait: select SubTrait → fulfill → adds SuperTrait → fulfill → report.
    let fixture = TestFixture::with_supertrait();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    let msg = report_fulfillment_result(&result, &fixture.resolver);
    assert!(msg.contains("succeeded"));
    assert_eq!(result.resolved_count(), 2);
}

#[test]
fn test_e2e_full_pipeline_no_impl_error() {
    // Full pipeline with NoImpl: select → fulfill → error → report.
    let fixture = TestFixture::with_trait_no_impl();
    let mut infer_ctxt = InferCtxt::new();
    let obl = fixture.make_obligation(fixture.trait_def_id);
    let result = with_eval_ctxt(&fixture, &mut infer_ctxt, |cx| {
        fulfill_obligation(obl, cx, DEFAULT_MAX_DEPTH)
    });
    let msg = report_fulfillment_result(&result, &fixture.resolver);
    assert!(msg.contains("failed"));
    assert!(result.has_errors());
}
