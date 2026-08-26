use super::*;

#[test]
fn test_new_context_has_static() {
    let ctx = RegionInferenceContext::new();
    // `'static` is universal region 0
    assert_eq!(ctx.num_regions(), 1);
    assert_eq!(ctx.universal_regions().len(), 1);
    assert_eq!(ctx.universal_regions()[0], RegionVid(0));
    // RegionInfo for vid 0 should be Universal { name: "'static" }
    let info = ctx.region_info(RegionVid(0)).unwrap();
    assert!(info.is_universal());
    match info {
        RegionInfo::Universal { name, vid } => {
            assert_eq!(*name, "'static");
            assert_eq!(*vid, RegionVid(0));
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_add_universal_region() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_universal_region("'a");
    assert_eq!(vid_a, RegionVid(1)); // 0 is `'static`
    assert_eq!(ctx.num_regions(), 2);
    assert_eq!(ctx.universal_regions().len(), 2);
    let info = ctx.region_info(vid_a).unwrap();
    assert!(info.is_universal());
}

#[test]
fn test_add_inference_region() {
    let mut ctx = RegionInferenceContext::new();
    let vid = ctx.add_inference_region(UniverseId::ROOT);
    assert_eq!(vid, RegionVid(1)); // 0 is `'static`
    assert_eq!(ctx.num_regions(), 2);
    let info = ctx.region_info(vid).unwrap();
    assert!(info.is_inference());
    match info {
        RegionInfo::Inference { vid: v, universe } => {
            assert_eq!(*v, RegionVid(1));
            assert_eq!(*universe, UniverseId::ROOT);
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_add_outlives_constraint() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_universal_region("'a");
    let vid_b = ctx.add_universal_region("'b");
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    assert_eq!(ctx.num_constraints(), 1);
    let c = &ctx.constraints()[0];
    assert_eq!(c.sup, vid_a);
    assert_eq!(c.sub, vid_b);
}

#[test]
fn test_add_type_test() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // `'static`
    let ty = Ty::new(
        crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
        Span::DUMMY,
    );
    ctx.add_type_test(vid_static, ty.clone(), Span::DUMMY);
    assert_eq!(ctx.num_type_tests(), 1);
    let tt = &ctx.type_tests()[0];
    assert_eq!(tt.universal_region, vid_static);
    assert_eq!(tt.ty, ty);
}

#[test]
fn test_new_universe() {
    let mut ctx = RegionInferenceContext::new();
    assert_eq!(ctx.universe_causes.len(), 1); // Root
    let uid = ctx.new_universe(UniverseCause::Hrtb { span: Span::DUMMY });
    assert_eq!(uid, UniverseId(1));
    assert_eq!(ctx.universe_causes.len(), 2);
}

#[test]
fn test_region_to_vid() {
    let ctx = RegionInferenceContext::new();
    // `'static` → vid 0
    assert_eq!(ctx.region_to_vid(Region::Static), RegionVid(0));
    // `Region::Var(5)` → vid 5
    assert_eq!(ctx.region_to_vid(Region::Var(RegionVid(5))), RegionVid(5));
    // `Region::Erased` → vid 0 (treated as `'static`)
    assert_eq!(ctx.region_to_vid(Region::Erased), RegionVid(0));
}

#[test]
fn test_universe_next() {
    assert_eq!(UniverseId::ROOT, UniverseId(0));
    assert_eq!(UniverseId::ROOT.next(), UniverseId(1));
    assert_eq!(UniverseId(5).next(), UniverseId(6));
}

#[test]
fn test_region_info_predicates() {
    let universal = RegionInfo::Universal {
        name: "'a",
        vid: RegionVid(1),
    };
    assert!(universal.is_universal());
    assert!(!universal.is_inference());
    assert!(!universal.is_placeholder());

    let inference = RegionInfo::Inference {
        vid: RegionVid(2),
        universe: UniverseId::ROOT,
    };
    assert!(!inference.is_universal());
    assert!(inference.is_inference());
    assert!(!inference.is_placeholder());

    let placeholder = RegionInfo::Placeholder {
        universe: UniverseId(1),
        vid: RegionVid(3),
    };
    assert!(!placeholder.is_universal());
    assert!(!placeholder.is_inference());
    assert!(placeholder.is_placeholder());
}

// ================================================================
// Stage 7.2 (TD-015 step 2): Region inference algorithm tests
// ================================================================

#[test]
fn test_infer_regions_empty() {
    let mut ctx = RegionInferenceContext::new();
    // No constraints, no use points → all point sets empty, no errors
    let result = ctx.infer_regions();
    assert!(result.is_ok());
    // `'static` (vid 0) has empty point set
    assert_eq!(ctx.region_points(RegionVid(0)), Some(&Vec::new()));
}

#[test]
fn test_infer_regions_use_points() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // `'static`
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // Add use points for region 'a
    ctx.add_use_point(vid_a, make_point(0, 1));
    ctx.add_use_point(vid_a, make_point(0, 2));

    // Also add use points for 'static so 'a's points don't escape
    ctx.add_use_point(vid_static, make_point(0, 1));
    ctx.add_use_point(vid_static, make_point(0, 2));

    let result = ctx.infer_regions();
    assert!(result.is_ok());

    // 'a should have points {0,1}, {0,2}
    let pts = ctx.region_points(vid_a).unwrap();
    assert_eq!(pts.len(), 2);
    assert!(pts.contains(&make_point(0, 1)));
    assert!(pts.contains(&make_point(0, 2)));

    // 'static also has the same points
    let static_pts = ctx.region_points(vid_static).unwrap();
    assert_eq!(static_pts.len(), 2);
}

#[test]
fn test_infer_regions_constraint_propagation() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // `'static`
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2

    // 'a: 'b (a outlives b) → a ⊇ b
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    // 'b has use point, 'a does not
    ctx.add_use_point(vid_b, make_point(0, 5));
    // 'static also has the use point so 'a doesn't escape
    ctx.add_use_point(vid_static, make_point(0, 5));

    let result = ctx.infer_regions();
    assert!(result.is_ok());

    // 'a should inherit 'b's points via constraint propagation
    let a_pts = ctx.region_points(vid_a).unwrap();
    let b_pts = ctx.region_points(vid_b).unwrap();
    assert!(b_pts.contains(&make_point(0, 5)));
    assert!(a_pts.contains(&make_point(0, 5))); // propagated
}

#[test]
fn test_infer_regions_universal_escape_detected() {
    let mut ctx = RegionInferenceContext::new();
    // vid 0 = 'static (universal)
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // 'a has use point but no constraint linking it to 'static
    ctx.add_use_point(vid_a, make_point(0, 1));

    // 'static has no use points → empty point set
    // 'a has point {0,1} which is NOT subset of 'static's empty set
    // → RegionEscapesUniversal error
    let result = ctx.infer_regions();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    match &errors[0] {
        RegionInferenceError::RegionEscapesUniversal {
            escaping_region,
            universal_region,
            escape_points,
            span: _,
        } => {
            assert_eq!(*escaping_region, vid_a);
            assert_eq!(*universal_region, RegionVid(0)); // 'static
            assert_eq!(escape_points.len(), 1);
            assert!(escape_points.contains(&make_point(0, 1)));
        }
        RegionInferenceError::TypeTestFailed { .. } => {
            panic!("expected RegionEscapesUniversal, got TypeTestFailed");
        }
    }
}

#[test]
fn test_infer_regions_universal_no_escape() {
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // 'static (universal)
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // 'a: 'static (a outlives 'static) → a ⊇ 'static
    // This means 'a can have any points 'static has (but 'static is empty)
    ctx.add_outlives_constraint(
        vid_a,
        vid_static,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    // 'a has no use points → empty point set
    // empty ⊆ empty → no error
    let result = ctx.infer_regions();
    assert!(result.is_ok());
}

#[test]
fn test_point_encoding() {
    let p = make_point(3, 7);
    assert_eq!(point_bb(p), 3);
    assert_eq!(point_stmt(p), 7);

    let p2 = make_point(0, 0);
    assert_eq!(point_bb(p2), 0);
    assert_eq!(point_stmt(p2), 0);

    let p3 = make_point(65535, 65535);
    assert_eq!(point_bb(p3), 65535);
    assert_eq!(point_stmt(p3), 65535);
}

#[test]
fn test_infer_regions_fixed_point_convergence() {
    // Test that the fixed-point iteration converges with chained constraints
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // 'static
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2
    let vid_c = ctx.add_inference_region(UniverseId::ROOT); // vid 3

    // Chain: 'a: 'b, 'b: 'c → 'a ⊇ 'b ⊇ 'c
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    ctx.add_outlives_constraint(
        vid_b,
        vid_c,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    // Only 'c has use points
    ctx.add_use_point(vid_c, make_point(1, 0));
    // 'static also has the use point so 'a doesn't escape
    ctx.add_use_point(vid_static, make_point(1, 0));

    let result = ctx.infer_regions();
    assert!(result.is_ok());

    // All three should have the same point set (propagated through chain)
    let a_pts = ctx.region_points(vid_a).unwrap();
    let b_pts = ctx.region_points(vid_b).unwrap();
    let c_pts = ctx.region_points(vid_c).unwrap();
    assert!(c_pts.contains(&make_point(1, 0)));
    assert!(b_pts.contains(&make_point(1, 0))); // propagated from c
    assert!(a_pts.contains(&make_point(1, 0))); // propagated from b
}

// ================================================================
// Stage 7.3 (TD-015 step 3): Implied bounds + type tests
// ================================================================

#[test]
fn test_extract_regions_from_ref() {
    use crate::mir::ty::{Mutability, TyKind};
    // &'a i32
    let ty = Ty::new(
        TyKind::Ref(
            Region::Var(RegionVid(5)),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );
    let regions = extract_regions_from_ty(&ty);
    assert_eq!(regions, vec![RegionVid(5)]);
}

#[test]
fn test_extract_regions_from_nested_ref() {
    use crate::mir::ty::{Mutability, TyKind};
    // &'a &'b i32
    let inner = Ty::new(
        TyKind::Ref(
            Region::Var(RegionVid(3)),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );
    let outer = Ty::new(
        TyKind::Ref(
            Region::Var(RegionVid(5)),
            Mutability::Immutable,
            Box::new(inner),
        ),
        Span::DUMMY,
    );
    let regions = extract_regions_from_ty(&outer);
    assert_eq!(regions, vec![RegionVid(3), RegionVid(5)]);
}

#[test]
fn test_extract_regions_from_non_ref() {
    // i32 has no regions
    let ty = Ty::new(
        crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
        Span::DUMMY,
    );
    let regions = extract_regions_from_ty(&ty);
    assert!(regions.is_empty());
}

#[test]
fn test_collect_implied_bounds() {
    use crate::mir::ty::{Mutability, TyKind};
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_universal_region("'a"); // vid 1
    let vid_b = ctx.add_universal_region("'b"); // vid 2

    // &'a &'b i32 — implies 'b: 'a
    let inner_ty = Ty::new(
        TyKind::Ref(
            Region::Var(vid_b),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );
    ctx.collect_implied_bounds(vid_a, &inner_ty, Span::DUMMY);

    // Should have added a constraint 'b: 'a (vid_b outlives vid_a)
    assert_eq!(ctx.num_constraints(), 1);
    let c = &ctx.constraints()[0];
    assert_eq!(c.sup, vid_b); // 'b outlives 'a
    assert_eq!(c.sub, vid_a);
}

#[test]
fn test_type_test_passes() {
    use crate::mir::ty::TyKind;
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0);

    // i32 has no regions → type test always passes
    let ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
    ctx.add_type_test(vid_static, ty, Span::DUMMY);

    let result = ctx.infer_regions();
    assert!(result.is_ok());
}

#[test]
fn test_type_test_fails() {
    use crate::mir::ty::{Mutability, TyKind};
    let mut ctx = RegionInferenceContext::new();
    let vid_static = RegionVid(0); // 'static, no use points
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1

    // &'a i32 — has region vid_a
    let ty = Ty::new(
        TyKind::Ref(
            Region::Var(vid_a),
            Mutability::Immutable,
            Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
        ),
        Span::DUMMY,
    );

    // Add a use point for 'a (but NOT for 'static)
    ctx.add_use_point(vid_a, make_point(0, 1));

    // Type test: &'a i32 must outlive 'static
    // 'a has point {0,1}, 'static has {} → 'a doesn't outlive 'static
    ctx.add_type_test(vid_static, ty.clone(), Span::DUMMY);

    let result = ctx.infer_regions();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should have both RegionEscapesUniversal AND TypeTestFailed
    let has_type_test_error = errors
        .iter()
        .any(|e| matches!(e, RegionInferenceError::TypeTestFailed { .. }));
    assert!(has_type_test_error, "expected TypeTestFailed error");
}

// ================================================================
// Stage 7.4 (TD-015 step 4): Universe tracking + SCC compression tests
// ================================================================

#[test]
fn test_region_universe() {
    let mut ctx = RegionInferenceContext::new();
    // 'static (vid 0) is in ROOT universe
    assert_eq!(ctx.region_universe(RegionVid(0)), Some(UniverseId::ROOT));

    // Inference region in ROOT universe
    let vid_a = ctx.add_inference_region(UniverseId::ROOT);
    assert_eq!(ctx.region_universe(vid_a), Some(UniverseId::ROOT));

    // Create a new universe (e.g., for HRTB)
    let uid1 = ctx.new_universe(UniverseCause::Hrtb { span: Span::DUMMY });
    assert_eq!(uid1, UniverseId(1));

    // Inference region in universe 1
    let vid_b = ctx.add_inference_region(uid1);
    assert_eq!(ctx.region_universe(vid_b), Some(UniverseId(1)));
}

#[test]
fn test_check_universe_escapes_no_violation() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_inference_region(UniverseId::ROOT);
    let vid_b = ctx.add_inference_region(UniverseId::ROOT);

    // 'a: 'b (both in ROOT universe — no escape)
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let errors = ctx.check_universe_escapes();
    assert!(errors.is_empty());
}

#[test]
fn test_check_universe_escapes_detected() {
    let mut ctx = RegionInferenceContext::new();
    let uid1 = ctx.new_universe(UniverseCause::Hrtb { span: Span::DUMMY });
    let vid_a = ctx.add_inference_region(uid1); // universe 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // universe 0

    // 'a: 'b where 'a is in universe 1, 'b is in universe 0
    // This is an escape: higher-universe 'a constrained to outlive lower-universe 'b
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let errors = ctx.check_universe_escapes();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].escaping_region, vid_a);
    assert_eq!(errors[0].target_region, vid_b);
    assert_eq!(errors[0].escaping_universe, UniverseId(1));
    assert_eq!(errors[0].target_universe, UniverseId::ROOT);
}

#[test]
fn test_scc_no_constraints() {
    let mut ctx = RegionInferenceContext::new();
    ctx.add_inference_region(UniverseId::ROOT);
    ctx.add_inference_region(UniverseId::ROOT);

    let sccs = ctx.compute_sccs();
    // 3 regions ('static + 2 inference), no constraints → 3 SCCs
    assert_eq!(sccs.len(), 3);
    // Each region is its own SCC (all distinct)
    let unique_sccs: Vec<SccId> = {
        let mut s = sccs.clone();
        s.sort_by_key(|s| s.0);
        s.dedup();
        s
    };
    assert_eq!(unique_sccs.len(), 3);
}

#[test]
fn test_scc_mutual_constraints() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2

    // 'a: 'b AND 'b: 'a → mutual → same SCC
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    ctx.add_outlives_constraint(
        vid_b,
        vid_a,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let sccs = ctx.compute_sccs();
    // vid 1 and vid 2 should be in the same SCC
    assert_eq!(sccs[1], sccs[2]);
    // 'static (vid 0) should be in a different SCC
    assert_ne!(sccs[0], sccs[1]);
}

#[test]
fn test_scc_chain() {
    let mut ctx = RegionInferenceContext::new();
    let vid_a = ctx.add_inference_region(UniverseId::ROOT); // vid 1
    let vid_b = ctx.add_inference_region(UniverseId::ROOT); // vid 2
    let vid_c = ctx.add_inference_region(UniverseId::ROOT); // vid 3

    // Chain: 'a: 'b, 'b: 'c (no cycle) → 3 distinct SCCs (plus 'static)
    ctx.add_outlives_constraint(
        vid_a,
        vid_b,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );
    ctx.add_outlives_constraint(
        vid_b,
        vid_c,
        ConstraintCause::FnSignature { span: Span::DUMMY },
    );

    let sccs = ctx.compute_sccs();
    // All 4 regions should be in distinct SCCs
    let unique_sccs: Vec<SccId> = {
        let mut s = sccs.clone();
        s.sort_by_key(|s| s.0);
        s.dedup();
        s
    };
    assert_eq!(unique_sccs.len(), 4);
}
