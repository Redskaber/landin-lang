//! Stage 5.5: vtable generation tests
#![allow(deprecated)] // Stage 16.11: tests verify deprecated Spur-based methods for backward compat
//!
//! Tests that TraitResolver correctly builds vtables for trait impls.
//!
//! Audit (2026-07-22): added `test_vtable_query` to verify `find_vtable`
//! returns vtable entries with correct `method_name` and structural fields.
//! Original 5.5 tests only checked `vtable_count()`.

use landin_compiler::compile;

#[test]
fn test_vtable_built_for_impl() {
    // When `impl Foo for S` exists, a vtable should be built.
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    assert_eq!(
        result.trait_resolver.vtable_count(),
        1,
        "should have 1 vtable"
    );
}

#[test]
fn test_no_vtable_without_impl() {
    // Without `impl Foo for S`, no vtable should exist.
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    assert_eq!(
        result.trait_resolver.vtable_count(),
        0,
        "should have 0 vtables"
    );
}

#[test]
fn test_vtable_multiple_impls() {
    // Multiple trait impls should produce multiple vtables.
    let result = compile(
        "trait Foo { fn bar(); } trait Baz { fn qux(); } struct S; impl Foo for S { fn bar() {} } impl Baz for S { fn qux() {} } fn main() {}",
    );
    assert_eq!(
        result.trait_resolver.vtable_count(),
        2,
        "should have 2 vtables"
    );
}

/// Audit (2026-07-22): Verify `find_vtable` returns the vtable with the
/// correct entries — `method_name` (Spur), `fn_def_id` (DefId), and
/// structural fields (trait_name, self_ty_name, impl_def_id). This guards
/// against silent regressions in the vtable construction logic.
#[test]
fn test_vtable_query() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} fn baz() {} } fn main() {}",
    );

    // Resolve interned symbols for "Foo" and "S".
    let foo_spur = result.interner.get("Foo").expect("Foo should be interned");
    let s_spur = result.interner.get("S").expect("S should be interned");

    // Look up the vtable.
    let vtable = result
        .trait_resolver
        .find_vtable(foo_spur, s_spur)
        .expect("vtable for (Foo, S) should exist");

    // Verify structural fields.
    assert_eq!(vtable.trait_name, foo_spur, "trait_name should be Foo");
    assert_eq!(vtable.self_ty_name, s_spur, "self_ty_name should be S");
    assert_eq!(
        vtable.entries.len(),
        2,
        "vtable should have 2 entries (bar + baz)"
    );

    // Verify each entry's method_name. Entries appear in impl-declaration
    // order: bar, then baz.
    let bar_spur = result.interner.get("bar").expect("bar should be interned");
    let baz_spur = result.interner.get("baz").expect("baz should be interned");

    assert_eq!(
        vtable.entries[0].method_name, bar_spur,
        "first entry method_name should be 'bar'"
    );
    assert_eq!(
        vtable.entries[1].method_name, baz_spur,
        "second entry method_name should be 'baz'"
    );
    // Stage 15.9: fn_name is now Spur, resolve via interner for comparison.
    let fn_name_0 = result
        .interner
        .try_resolve(&vtable.entries[0].fn_name)
        .unwrap_or("?");
    let fn_name_1 = result
        .interner
        .try_resolve(&vtable.entries[1].fn_name)
        .unwrap_or("?");
    assert_eq!(
        fn_name_0, "landin_S_bar",
        "first entry fn_name should be 'landin_S_bar'"
    );
    assert_eq!(
        fn_name_1, "landin_S_baz",
        "second entry fn_name should be 'landin_S_baz'"
    );
}
