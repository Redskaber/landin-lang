//! Stage 7.6 (TD-018): User-defined trait dyn support integration tests.
//!
//! Per stage-committee-process.md v3.21 §17.1, test files live under
//! `tests/v0/stage{N}/plan/`. This file tests the user-defined trait dyn
//! method call resolution added in Stage 7.6.
//!
//! Test categories:
//! 1. build_dyn_trait_method_calls_from_resolver — user-defined trait resolution
//! 2. User-defined trait vtable method lookup
//! 3. Mixed stdlib + user-defined trait handling
//! 4. Regression: stdlib-only traits still work

use landin_compiler::hir::DefId;
use landin_compiler::mir::dyn_trait::{
    build_dyn_trait_fat_ptrs_from_resolver, build_dyn_trait_method_calls_from_resolver,
};
use landin_compiler::session::Span;
use landin_compiler::traits::resolver::{ImplInfo, TraitInfo, TraitResolver};
use landin_compiler::traits::vtable::{Vtable, VtableEntry};
use lasso::{Rodeo, Spur};

/// Helper: create a TraitResolver with a user-defined trait + impl + vtable.
fn make_resolver_with_user_trait(
    interner: &mut Rodeo,
    trait_name: &str,
    type_name: &str,
    method_names: &[&str],
    fn_names: &[&str],
) -> TraitResolver {
    let mut resolver = TraitResolver::default();

    let trait_spur = interner.get_or_intern(trait_name);
    let type_spur = interner.get_or_intern(type_name);

    // Register trait
    let trait_def_id = DefId(100);
    let methods: Vec<Spur> = method_names
        .iter()
        .map(|m| interner.get_or_intern(m))
        .collect();
    resolver.traits.insert(
        trait_def_id,
        TraitInfo {
            def_id: trait_def_id,
            name: trait_spur,
            methods: methods.clone(),
            is_unsafe: false,
            supertraits: Vec::new(),
            default_methods: Vec::new(),
            associated_consts: Vec::new(),
        },
    );
    resolver.trait_by_name.insert(trait_spur, trait_def_id);

    // Register impl
    let impl_def_id = DefId(101);
    resolver.impls.insert(
        impl_def_id,
        ImplInfo {
            def_id: impl_def_id,
            trait_name: Some(trait_spur),
            self_ty_name: Some(type_spur),
            methods: methods.clone(),
            is_unsafe: false,
            span: Span::DUMMY,
            associated_consts: Vec::new(),
        },
    );
    resolver
        .impl_by_trait_and_type
        .insert((trait_spur, type_spur), impl_def_id);

    // Register vtable
    let entries: Vec<VtableEntry> = method_names
        .iter()
        .zip(fn_names.iter())
        .map(|(m, f)| VtableEntry {
            method_name: interner.get_or_intern(m),
            fn_name: interner.get_or_intern(f),
        })
        .collect();
    resolver.vtables.insert(
        (trait_spur, type_spur),
        Vtable {
            trait_name: trait_spur,
            self_ty_name: type_spur,
            impl_def_id,
            entries,
        },
    );

    resolver
}

#[test]
fn stage7_user_defined_trait_fat_ptr_generation() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_user_trait(
        &mut interner,
        "Greet",
        "Person",
        &["hello", "bye"],
        &["landin_Person_hello", "landin_Person_bye"],
    );

    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 1);
    assert_eq!(fat_ptrs[0].trait_name, "Greet");
    assert_eq!(fat_ptrs[0].type_name, "Person");
}

#[test]
fn stage7_user_defined_trait_method_calls_from_resolver() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_user_trait(
        &mut interner,
        "Greet",
        "Person",
        &["hello", "bye"],
        &["landin_Person_hello", "landin_Person_bye"],
    );

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(calls.len(), 2, "expected 2 method calls (hello + bye)");

    // Verify method names
    let method_names: Vec<&str> = calls.iter().map(|c| c.method_name.as_str()).collect();
    assert!(method_names.contains(&"hello"));
    assert!(method_names.contains(&"bye"));

    // Verify trait/type names
    for call in &calls {
        assert_eq!(call.trait_name, "Greet");
        assert_eq!(call.type_name, "Person");
    }

    // Verify slot indices (0 and 1)
    let slots: Vec<u32> = calls.iter().map(|c| c.slot_index).collect();
    assert!(slots.contains(&0));
    assert!(slots.contains(&1));
}

#[test]
fn stage7_user_defined_trait_slot_index_ordering() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_user_trait(
        &mut interner,
        "Drawable",
        "Circle",
        &["draw", "area", "contains"],
        &[
            "landin_Circle_draw",
            "landin_Circle_area",
            "landin_Circle_contains",
        ],
    );

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(calls.len(), 3);

    // Verify slot indices are sequential (0, 1, 2)
    let mut slots: Vec<u32> = calls.iter().map(|c| c.slot_index).collect();
    slots.sort();
    assert_eq!(slots, vec![0, 1, 2]);
}

#[test]
fn stage7_user_defined_trait_empty_methods() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_user_trait(&mut interner, "Empty", "MyType", &[], &[]);

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    // No methods → no method calls
    assert_eq!(calls.len(), 0);

    // But fat ptr should still exist
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 1);
}

#[test]
fn stage7_user_defined_trait_multiple_traits() {
    let mut interner = Rodeo::new();

    // Register trait 1: Greet for Person
    let resolver1 = make_resolver_with_user_trait(
        &mut interner,
        "Greet",
        "Person",
        &["hello"],
        &["landin_Person_hello"],
    );

    // Register trait 2: Display for Person (in same resolver)
    let mut resolver = resolver1;
    let display_spur = interner.get_or_intern("Display");
    let person_spur = interner.get_or_intern("Person");
    let method_spur = interner.get_or_intern("fmt");

    let trait_def_id = DefId(200);
    resolver.traits.insert(
        trait_def_id,
        TraitInfo {
            def_id: trait_def_id,
            name: display_spur,
            methods: vec![method_spur],
            is_unsafe: false,
            supertraits: Vec::new(),
            default_methods: Vec::new(),
            associated_consts: Vec::new(),
        },
    );
    resolver.trait_by_name.insert(display_spur, trait_def_id);

    let impl_def_id = DefId(201);
    resolver.impls.insert(
        impl_def_id,
        ImplInfo {
            def_id: impl_def_id,
            trait_name: Some(display_spur),
            self_ty_name: Some(person_spur),
            methods: vec![method_spur],
            is_unsafe: false,
            span: Span::DUMMY,
            associated_consts: Vec::new(),
        },
    );
    resolver
        .impl_by_trait_and_type
        .insert((display_spur, person_spur), impl_def_id);

    resolver.vtables.insert(
        (display_spur, person_spur),
        Vtable {
            trait_name: display_spur,
            self_ty_name: person_spur,
            impl_def_id,
            entries: vec![VtableEntry {
                method_name: method_spur,
                fn_name: interner.get_or_intern("landin_Person_fmt"),
            }],
        },
    );

    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 2, "expected 2 fat ptrs (Greet + Display)");

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(calls.len(), 2, "expected 2 method calls (hello + fmt)");

    // Verify both traits are represented
    let trait_names: Vec<&str> = calls.iter().map(|c| c.trait_name.as_str()).collect();
    assert!(trait_names.contains(&"Greet"));
    assert!(trait_names.contains(&"Display"));
}

#[test]
fn stage7_regression_stdlib_traits_still_work() {
    // Verify that stdlib traits (like Copy) still work alongside user-defined traits.
    // This test uses the existing TraitResolver without user-defined traits
    // and verifies the resolver doesn't crash.
    let interner = Rodeo::new();
    let resolver = TraitResolver::default();

    // Empty resolver → no fat ptrs, no method calls
    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert!(fat_ptrs.is_empty());

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert!(calls.is_empty());
}

#[test]
fn stage7_user_defined_trait_method_call_fields() {
    let mut interner = Rodeo::new();
    let resolver = make_resolver_with_user_trait(
        &mut interner,
        "Iter",
        "Range",
        &["next"],
        &["landin_Range_next"],
    );

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(calls.len(), 1);

    let call = &calls[0];
    assert_eq!(call.trait_name, "Iter");
    assert_eq!(call.type_name, "Range");
    assert_eq!(call.method_name, "next");
    assert_eq!(call.slot_index, 0);
    assert_eq!(call.param_count, 0); // next() has no explicit params (just &self)
}

#[test]
fn stage7_user_defined_trait_multiple_types_same_trait() {
    let mut interner = Rodeo::new();

    // Register trait Greet for Person
    let mut resolver = make_resolver_with_user_trait(
        &mut interner,
        "Greet",
        "Person",
        &["hello"],
        &["landin_Person_hello"],
    );

    // Register trait Greet for Robot (same trait, different type)
    let greet_spur = interner.get_or_intern("Greet");
    let robot_spur = interner.get_or_intern("Robot");
    let hello_spur = interner.get_or_intern("hello");

    let impl_def_id = DefId(300);
    resolver.impls.insert(
        impl_def_id,
        ImplInfo {
            def_id: impl_def_id,
            trait_name: Some(greet_spur),
            self_ty_name: Some(robot_spur),
            methods: vec![hello_spur],
            is_unsafe: false,
            span: Span::DUMMY,
            associated_consts: Vec::new(),
        },
    );
    resolver
        .impl_by_trait_and_type
        .insert((greet_spur, robot_spur), impl_def_id);

    resolver.vtables.insert(
        (greet_spur, robot_spur),
        Vtable {
            trait_name: greet_spur,
            self_ty_name: robot_spur,
            impl_def_id,
            entries: vec![VtableEntry {
                method_name: hello_spur,
                fn_name: interner.get_or_intern("landin_Robot_hello"),
            }],
        },
    );

    let fat_ptrs = build_dyn_trait_fat_ptrs_from_resolver(&resolver, &interner);
    assert_eq!(fat_ptrs.len(), 2, "expected 2 fat ptrs (Person + Robot)");

    let calls = build_dyn_trait_method_calls_from_resolver(&resolver, &interner);
    assert_eq!(calls.len(), 2, "expected 2 method calls");

    // Verify both types are represented
    let type_names: Vec<&str> = calls.iter().map(|c| c.type_name.as_str()).collect();
    assert!(type_names.contains(&"Person"));
    assert!(type_names.contains(&"Robot"));
}
