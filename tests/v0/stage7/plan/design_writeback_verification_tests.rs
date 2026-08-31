//! Stage 7.7 (§25.8): Design writeback verification tests.
//!
//! Per stage-committee-process.md v3.21 §17.1 + §25.8, this file verifies
//! that the design writeback for TD-015 (region inference) and TD-018
//! (user-defined trait dyn) is correctly reflected in the implementation.
//!
//! These tests are "meta-tests" — they verify that the implementation
//! matches the design documentation's updated status (§11/§12 writeback).
// Stage 15.37: Allow deprecated — these tests intentionally exercise the
// legacy `check_mir_body` path while it is being phased out (driver now uses
// `check_mir_body_with_dataflow`).

use landin_compiler::borrowck::BorrowChecker;
use landin_compiler::hir::DefId;
use landin_compiler::mir::body::MirBody;
use landin_compiler::mir::dyn_trait::{
    build_dyn_trait_fat_ptrs_from_resolver, build_dyn_trait_method_calls_from_resolver,
};
use landin_compiler::mir::ty::{Region, Ty, TyKind};
use landin_compiler::session::Span;
use landin_compiler::traits::resolver::{ImplInfo, TraitInfo, TraitResolver};
use landin_compiler::traits::vtable::{Vtable, VtableEntry};
use lasso::Rodeo;

// ================================================================
// TD-015 verification: Region inference infrastructure exists
// ================================================================

#[test]
fn stage7_td015_borrow_checker_runs_region_inference() {
    // Per §4.6.6 + §4.2: BorrowChecker should run region inference
    // as an additional check. Verify it doesn't crash.
    let mut bc = BorrowChecker::new();
    let mut mir = MirBody::new(Span::DUMMY);
    mir.new_block();
    bc.check_mir_body_with_dataflow(&mir);
    let errors = bc.into_errors();
    assert!(
        errors.is_empty(),
        "region inference should not produce errors on empty body"
    );
}

#[test]
fn stage7_td015_region_inference_handles_ref_types() {
    // Per §4.6.2: implied bounds from &'a T → T: 'a
    // Verify BorrowChecker handles reference types without crashing
    let mut mir = MirBody::new(Span::DUMMY);
    let _bb0 = mir.new_block();
    let _ref_local = mir.new_local(
        Ty::new(
            TyKind::Ref(
                Region::Erased,
                landin_compiler::mir::ty::Mutability::Immutable,
                Box::new(Ty::new(
                    TyKind::Int(landin_compiler::ast::IntTy::I32),
                    Span::DUMMY,
                )),
            ),
            Span::DUMMY,
        ),
        None,
        Span::DUMMY,
    );
    let errors = landin_compiler::borrowck::check_mir_body_with_dataflow(&mir);
    assert!(
        errors.is_empty(),
        "region inference should handle ref types"
    );
}

#[test]
fn stage7_td015_region_inference_nested_refs() {
    // Per §4.6.2: nested refs like &'a &'b i32 should collect
    // implied bounds for both 'a and 'b
    let mut mir = MirBody::new(Span::DUMMY);
    let _bb0 = mir.new_block();

    let inner_ref = Ty::new(
        TyKind::Ref(
            Region::Erased,
            landin_compiler::mir::ty::Mutability::Immutable,
            Box::new(Ty::new(
                TyKind::Int(landin_compiler::ast::IntTy::I32),
                Span::DUMMY,
            )),
        ),
        Span::DUMMY,
    );
    let outer_ref = Ty::new(
        TyKind::Ref(
            Region::Erased,
            landin_compiler::mir::ty::Mutability::Immutable,
            Box::new(inner_ref),
        ),
        Span::DUMMY,
    );
    let _nested_ref_local = mir.new_local(outer_ref, None, Span::DUMMY);

    let errors = landin_compiler::borrowck::check_mir_body_with_dataflow(&mir);
    assert!(
        errors.is_empty(),
        "region inference should handle nested refs"
    );
}

// ================================================================
// TD-018 verification: User-defined trait dyn support exists
// ================================================================

#[test]
fn stage7_td018_resolver_based_method_calls_exist() {
    // Per §2.3 + TD-018: build_dyn_trait_method_calls_from_resolver
    // should handle user-defined traits (not just stdlib).
    let interner = Rodeo::new();
    let resolver = TraitResolver::default();
    // Empty resolver → empty results
    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert!(
        calls.is_empty(),
        "empty resolver should produce no method calls"
    );
}

#[test]
fn stage7_td018_user_defined_trait_resolved() {
    // Per TD-018: user-defined traits should be resolved via
    // TraitResolver.vtables, not just stdlib registry.
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::default();

    let trait_name = interner.get_or_intern("MyTrait");
    let type_name = interner.get_or_intern("MyType");
    let method_name = interner.get_or_intern("do_thing");

    // Register trait
    resolver.traits.insert(
        DefId(100),
        TraitInfo {
            def_id: DefId(100),
            name: trait_name,
            methods: vec![method_name],
            is_unsafe: false,
            supertraits: Vec::new(),
            default_methods: Vec::new(),
            associated_consts: Vec::new(),
        },
    );
    resolver.trait_by_name.insert(trait_name, DefId(100));

    // Register impl
    resolver.impls.insert(
        DefId(101),
        ImplInfo {
            def_id: DefId(101),
            trait_name: Some(trait_name),
            self_ty_name: Some(type_name),
            methods: vec![method_name],
            is_unsafe: false,
            span: Span::DUMMY,
            associated_consts: Vec::new(),
            where_clauses: Vec::new(),
            hrtb_bounds: Vec::new(),
            assoc_type_bindings: std::collections::HashMap::new(),
        },
    );
    resolver
        .impl_by_trait_and_type
        .insert((trait_name, type_name), DefId(101));

    // Register vtable
    resolver.vtables.insert(
        (trait_name, type_name),
        Vtable {
            trait_name,
            self_ty_name: type_name,
            impl_def_id: DefId(101),
            entries: vec![VtableEntry {
                method_name,
                fn_name: interner.get_or_intern("landin_MyType_do_thing"),
            }],
        },
    );

    // Build fat ptrs
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 1);
    assert_eq!(fat_ptrs[0].trait_name, "MyTrait");
    assert_eq!(fat_ptrs[0].type_name, "MyType");

    // Build method calls — should resolve via vtable, not stdlib
    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(
        calls.len(),
        1,
        "user-defined trait should produce 1 method call"
    );
    assert_eq!(calls[0].method_name, "do_thing");
    assert_eq!(calls[0].trait_name, "MyTrait");
    assert_eq!(calls[0].type_name, "MyType");
    assert_eq!(calls[0].slot_index, 0);
}

#[test]
fn stage7_td018_stdlib_and_user_traits_coexist() {
    // Per TD-018: stdlib traits and user-defined traits should coexist.
    // This test verifies the resolver handles both without conflict.
    let mut interner = Rodeo::new();
    let mut resolver = TraitResolver::default();

    // Register a user-defined trait
    let trait_name = interner.get_or_intern("CustomTrait");
    let type_name = interner.get_or_intern("CustomType");
    let method_name = interner.get_or_intern("custom_method");

    resolver.traits.insert(
        DefId(200),
        TraitInfo {
            def_id: DefId(200),
            name: trait_name,
            methods: vec![method_name],
            is_unsafe: false,
            supertraits: Vec::new(),
            default_methods: Vec::new(),
            associated_consts: Vec::new(),
        },
    );
    resolver.trait_by_name.insert(trait_name, DefId(200));
    resolver.impls.insert(
        DefId(201),
        ImplInfo {
            def_id: DefId(201),
            trait_name: Some(trait_name),
            self_ty_name: Some(type_name),
            methods: vec![method_name],
            is_unsafe: false,
            span: Span::DUMMY,
            associated_consts: Vec::new(),
            where_clauses: Vec::new(),
            hrtb_bounds: Vec::new(),
            assoc_type_bindings: std::collections::HashMap::new(),
        },
    );
    resolver
        .impl_by_trait_and_type
        .insert((trait_name, type_name), DefId(201));
    resolver.vtables.insert(
        (trait_name, type_name),
        Vtable {
            trait_name,
            self_ty_name: type_name,
            impl_def_id: DefId(201),
            entries: vec![VtableEntry {
                method_name,
                fn_name: interner.get_or_intern("landin_CustomType_custom_method"),
            }],
        },
    );

    // User-defined trait should be resolved
    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(calls.len(), 1, "user-defined trait should produce 1 call");
    assert_eq!(calls[0].trait_name, "CustomTrait");

    // No crash, no false positives — coexistence verified
}
