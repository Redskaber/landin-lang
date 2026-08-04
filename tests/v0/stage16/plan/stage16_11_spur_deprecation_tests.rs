//! Stage 16.11 — Task 3 Step 4: Spur-based method deprecation + DefId-keyed alternatives tests.
//!
//! These tests verify the Stage 16.11 changes:
//! 1. Spur-based methods (`find_impl`, `implements`, `implements_by_def_id`,
//!    `find_vtable`, `impl_methods`) are deprecated.
//! 2. New DefId-keyed alternative `impl_methods_by_def_ids` works correctly.
//! 3. Deprecated methods still work (backward compat) with `#[allow(deprecated)]`.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): tests verify both the new method
//! and the deprecation path.
//! Per §23.6: deprecated methods have `note` pointing to alternatives.

#![allow(deprecated)] // Stage 16.11: tests verify deprecated methods for backward compat
#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.11 test 1: `impl_methods_by_def_ids` returns method names.
///
/// `trait Foo { fn bar(&self); fn baz(&self); } struct S; impl Foo for S`
/// — the DefId-keyed lookup should return the method names [bar, baz].
#[test]
fn stage16_11_impl_methods_by_def_ids_returns_methods() {
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
    let methods = result
        .trait_resolver
        .impl_methods_by_def_ids(foo_def_id, s_def_id)
        .expect("should find impl methods for Foo on S");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    let baz_spur = result.interner.get("baz").expect("baz interned");
    assert!(methods.contains(&bar_spur), "should have 'bar' method");
    assert!(methods.contains(&baz_spur), "should have 'baz' method");
}

/// Stage 16.11 test 2: `impl_methods_by_def_ids` returns None for no impl.
///
/// `trait Foo { fn bar(&self); } struct S;` (no impl) — should return None.
#[test]
fn stage16_11_impl_methods_by_def_ids_returns_none_for_no_impl() {
    let result = compile("trait Foo { fn bar(&self); } struct S; fn main() {}");
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
    assert!(
        result
            .trait_resolver
            .impl_methods_by_def_ids(foo_def_id, s_def_id)
            .is_none(),
        "impl_methods_by_def_ids should return None for S without impl Foo"
    );
}

/// Stage 16.11 test 3: DefId-keyed and Spur-based `impl_methods` agree.
///
/// For all (trait, type) pairs with impls, `impl_methods_by_def_ids` should
/// give the same result as the deprecated `impl_methods(trait_spur, type_spur)`.
#[test]
fn stage16_11_impl_methods_def_id_and_spur_agree() {
    let result = compile(
        "trait Foo { fn bar(&self); } trait Baz { fn qux(&self); } struct S; struct T; impl Foo for S { fn bar(&self) {} } impl Baz for T { fn qux(&self) {} } fn main() {}",
    );
    for (trait_name, type_name) in &[("Foo", "S"), ("Baz", "T"), ("Foo", "T"), ("Baz", "S")] {
        let trait_spur = result.interner.get(trait_name).expect("trait interned");
        let trait_def_id = result
            .trait_resolver
            .find_trait_def_id(trait_spur)
            .expect("trait DefId");
        let type_spur = result.interner.get(type_name).expect("type interned");
        let type_def_id = result
            .trait_resolver
            .type_by_def_id
            .iter()
            .find(|(_, &n)| n == type_spur)
            .map(|(&d, _)| d)
            .expect("type DefId");
        let spur_result = result.trait_resolver.impl_methods(trait_spur, type_spur);
        let def_id_result = result
            .trait_resolver
            .impl_methods_by_def_ids(trait_def_id, type_def_id);
        assert_eq!(
            spur_result.is_some(),
            def_id_result.is_some(),
            "Spur-based and DefId-based impl_methods should agree for {}::{}",
            trait_name,
            type_name
        );
    }
}

/// Stage 16.11 test 4: Deprecated `find_impl` still works (backward compat).
///
/// Verifies that the deprecated method still produces correct results.
#[test]
fn stage16_11_deprecated_find_impl_still_works() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let impl_info = result
        .trait_resolver
        .find_impl(foo_spur, s_spur)
        .expect("deprecated find_impl should still work");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    assert!(
        impl_info.methods.contains(&bar_spur),
        "deprecated find_impl should return impl with 'bar' method"
    );
}

/// Stage 16.11 test 5: Deprecated `implements` still works (backward compat).
#[test]
fn stage16_11_deprecated_implements_still_works() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } struct T; fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let t_spur = result.interner.get("T").expect("T interned");
    assert!(
        result.trait_resolver.implements(foo_spur, s_spur),
        "deprecated implements should return true for S with impl Foo"
    );
    assert!(
        !result.trait_resolver.implements(foo_spur, t_spur),
        "deprecated implements should return false for T without impl Foo"
    );
}

/// Stage 16.11 test 6: Deprecated `find_vtable` still works (backward compat).
#[test]
fn stage16_11_deprecated_find_vtable_still_works() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let vtable = result
        .trait_resolver
        .find_vtable(foo_spur, s_spur)
        .expect("deprecated find_vtable should still work");
    assert!(
        !vtable.entries.is_empty(),
        "deprecated find_vtable should return vtable with entries"
    );
}

/// Stage 16.11 test 7: Deprecated `implements_by_def_id` still works.
#[test]
fn stage16_11_deprecated_implements_by_def_id_still_works() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");
    assert!(
        result
            .trait_resolver
            .implements_by_def_id(foo_spur, s_def_id),
        "deprecated implements_by_def_id should return true for S with impl Foo"
    );
}
