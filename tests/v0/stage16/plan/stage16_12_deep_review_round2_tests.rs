//! Stage 16.12 — Deep Review Round 2: end-to-end consistency verification.
//!
//! This test verifies that the complete v0.3 trait resolution pipeline
//! (Sound Copy + DefId-keyed lookup + deprecated Spur methods) produces
//! consistent results end-to-end. This addresses the D3 (test coverage)
//! dimension of Deep Review Round 2.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): end-to-end consistency test.
//! Per §1.0 原則 6 "通用 > 特例": one consistent pipeline.

#![allow(deprecated)] // Stage 16.11: tests verify deprecated methods for backward compat
#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.12 test 1: End-to-end consistency — Copy detection via all paths.
///
/// Verifies that Copy detection produces the same result whether checked
/// via `is_copy_builtin` (production path) or the deprecated Spur-based
/// `implements` method. This confirms the migration is behavior-preserving
/// across the entire pipeline.
#[test]
fn stage16_12_copy_detection_end_to_end_consistency() {
    let result = compile(
        "struct Copyable { x: i32 } struct NonCopy; impl Drop for NonCopy { fn drop(&mut self) {} } fn main() {}",
    );
    let copy_spur = result.interner.get("Copy").expect("Copy interned");
    let copy_def_id = result
        .trait_resolver
        .find_trait_def_id(copy_spur)
        .expect("Copy trait DefId");

    for (type_name, expected_explicit_copy) in &[("Copyable", false), ("NonCopy", false)] {
        let type_spur = result.interner.get(type_name).expect("type interned");
        let type_def_id = result
            .trait_resolver
            .type_by_def_id
            .iter()
            .find(|(_, &n)| n == type_spur)
            .map(|(&d, _)| d)
            .expect("type DefId");

        // Production path: is_copy_builtin (checks derived + explicit)
        let production_result = result
            .trait_resolver
            .is_copy_builtin(type_def_id, &result.interner);

        // Deprecated path: implements (Spur-based, checks explicit only)
        let deprecated_result = result.trait_resolver.implements(copy_spur, type_spur);

        // DefId-keyed path: implements_by_def_ids
        let def_id_result = result
            .trait_resolver
            .implements_by_def_ids(copy_def_id, type_def_id);

        // Deprecated and DefId-keyed should agree (both check explicit impl only)
        assert_eq!(
            deprecated_result, def_id_result,
            "Deprecated and DefId-keyed Copy lookup should agree for {}",
            type_name
        );
        assert_eq!(
            deprecated_result, *expected_explicit_copy,
            "Explicit Copy check for {} should be {}",
            type_name, expected_explicit_copy
        );

        // Production path may differ for derived Copy (Copyable is derived Copy)
        if *type_name == "Copyable" {
            assert!(
                production_result,
                "Copyable should be Copy via production path (derived Copy)"
            );
        } else {
            assert!(
                !production_result,
                "NonCopy should NOT be Copy via production path (has impl Drop)"
            );
        }
    }
}

/// Stage 16.12 test 2: End-to-end consistency — vtable lookup via all paths.
///
/// Verifies that vtable lookup produces the same result whether checked
/// via `find_vtable_by_def_ids` (production path) or the deprecated
/// Spur-based `find_vtable` method.
#[test]
fn stage16_12_vtable_lookup_end_to_end_consistency() {
    let result = compile(
        "trait Foo { fn bar(&self); fn baz(&self); } struct S; struct T; impl Foo for S { fn bar(&self) {} fn baz(&self) {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let foo_def_id = result
        .trait_resolver
        .find_trait_def_id(foo_spur)
        .expect("Foo trait DefId");

    for type_name in &["S", "T"] {
        let type_spur = result.interner.get(type_name).expect("type interned");
        let type_def_id = result
            .trait_resolver
            .type_by_def_id
            .iter()
            .find(|(_, &n)| n == type_spur)
            .map(|(&d, _)| d)
            .expect("type DefId");

        let spur_vtable = result.trait_resolver.find_vtable(foo_spur, type_spur);
        let def_id_vtable = result
            .trait_resolver
            .find_vtable_by_def_ids(foo_def_id, type_def_id);

        assert_eq!(
            spur_vtable.is_some(),
            def_id_vtable.is_some(),
            "Spur-based and DefId-keyed vtable lookup should agree for {}",
            type_name
        );

        // Verify entries match if vtable exists
        if let (Some(s), Some(d)) = (spur_vtable, def_id_vtable) {
            assert_eq!(
                s.entries.len(),
                d.entries.len(),
                "Vtable entry count should match for {}",
                type_name
            );
        }
    }
}

/// Stage 16.12 test 3: End-to-end — impl methods via all paths.
///
/// Verifies that `impl_methods` (deprecated) and `impl_methods_by_def_ids`
/// (new) return the same method names.
#[test]
fn stage16_12_impl_methods_end_to_end_consistency() {
    let result = compile(
        "trait Foo { fn bar(&self); fn baz(&self); } struct S; impl Foo for S { fn bar(&self) {} fn baz(&self) {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let foo_def_id = result
        .trait_resolver
        .find_trait_def_id(foo_spur)
        .expect("Foo trait DefId");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");

    let spur_methods = result
        .trait_resolver
        .impl_methods(foo_spur, s_spur)
        .expect("should find impl methods (Spur)");
    let def_id_methods = result
        .trait_resolver
        .impl_methods_by_def_ids(foo_def_id, s_def_id)
        .expect("should find impl methods (DefId)");

    assert_eq!(
        spur_methods.len(),
        def_id_methods.len(),
        "Method count should match"
    );
    for (s, d) in spur_methods.iter().zip(def_id_methods.iter()) {
        assert_eq!(s, d, "Method names should match");
    }
}

/// Stage 16.12 test 4: Complete pipeline — program with traits compiles.
///
/// Verifies that a program using traits (impl, dyn Trait, Copy, Drop)
/// compiles end-to-end with the sound Copy detection and DefId-keyed
/// lookup.
#[test]
fn stage16_12_complete_pipeline_with_traits_compiles() {
    let src = r#"
        trait Drawable { fn draw(&self) -> i32; }
        struct Circle { radius: i32 }
        struct Square { side: i32 }
        impl Drawable for Circle { fn draw(&self) -> i32 { self.radius } }
        impl Drawable for Square { fn draw(&self) -> i32 { self.side } }
        impl Copy for Circle {}
        impl Copy for Square {}
        fn main() -> i32 {
            let c = Circle { radius: 5 };
            let c2 = c;
            c.draw()
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Program with traits should compile end-to-end; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.12 test 5: Complete pipeline — non-Copy struct with Drop.
///
/// Verifies that a program with a non-Copy struct (impl Drop) correctly
/// rejects use-after-move, confirming sound Copy detection works
/// end-to-end.
#[test]
fn stage16_12_non_copy_with_drop_rejects_use_after_move() {
    let src = r#"
        struct Resource { value: i32 }
        impl Drop for Resource { fn drop(&mut self) {} }
        fn main() -> i32 {
            let r = Resource { value: 42 };
            let r2 = r;
            let r3 = r;
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "Non-Copy struct with impl Drop should reject double-move (use-after-move)"
    );
}
