//! Stage 5.17: Vtable method resolution tests
//!
//! Tests `resolve_vtable_method()`, `vtable_method_names()`, and
//! `vtable_has_method()` for single-entry-point method dispatch resolution.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// `resolve_vtable_method` should return the LLVM symbol for a method.
#[test]
fn test_resolve_vtable_method() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");

    let fn_name = result
        .trait_resolver
        .resolve_vtable_method(foo_spur, s_spur, bar_spur)
        .expect("should resolve bar");
    assert_eq!(
        fn_name, "landin_S_bar",
        "resolved fn_name should be landin_S_bar"
    );
}

/// `resolve_vtable_method` should return None for unknown method.
#[test]
fn test_resolve_vtable_method_unknown_method() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    // Use "Foo" spur as a method name that doesn't exist
    assert!(
        result
            .trait_resolver
            .resolve_vtable_method(foo_spur, s_spur, foo_spur)
            .is_none(),
        "should return None for non-existent method"
    );
}

/// `resolve_vtable_method` should return None when no impl exists.
#[test]
fn test_resolve_vtable_method_no_impl() {
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");

    assert!(
        result
            .trait_resolver
            .resolve_vtable_method(foo_spur, s_spur, bar_spur)
            .is_none(),
        "should return None when no impl exists"
    );
}

/// `vtable_method_names` should return all method symbols.
#[test]
fn test_vtable_method_names() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); } struct S; impl Foo for S { fn bar() {} fn baz() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    let names = result.trait_resolver.vtable_method_names(foo_spur, s_spur);
    assert_eq!(names.len(), 2, "should have 2 method names");
    assert!(
        names.contains(&"landin_S_bar"),
        "should contain landin_S_bar"
    );
    assert!(
        names.contains(&"landin_S_baz"),
        "should contain landin_S_baz"
    );
}

/// `vtable_method_names` should return empty Vec when no vtable.
#[test]
fn test_vtable_method_names_empty() {
    let result = compile("trait Foo { fn bar(); } struct S; fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    let names = result.trait_resolver.vtable_method_names(foo_spur, s_spur);
    assert!(names.is_empty(), "should be empty when no vtable exists");
}

/// `vtable_has_method` should return true for existing method.
#[test]
fn test_vtable_has_method_true() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");

    assert!(
        result
            .trait_resolver
            .vtable_has_method(foo_spur, s_spur, bar_spur),
        "vtable should have method bar"
    );
}

/// `vtable_has_method` should return false for non-existent method.
#[test]
fn test_vtable_has_method_false() {
    let result =
        compile("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} } fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");

    assert!(
        !result
            .trait_resolver
            .vtable_has_method(foo_spur, s_spur, foo_spur),
        "vtable should NOT have method named 'Foo'"
    );
}

/// Multiple methods should all be resolvable.
#[test]
fn test_resolve_multiple_methods() {
    let result = compile(
        "trait Foo { fn bar(); fn baz(); fn qux(); } struct S; impl Foo for S { fn bar() {} fn baz() {} fn qux() {} } fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let s_spur = result.interner.get("S").expect("S interned");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    let baz_spur = result.interner.get("baz").expect("baz interned");
    let qux_spur = result.interner.get("qux").expect("qux interned");

    assert_eq!(
        result
            .trait_resolver
            .resolve_vtable_method(foo_spur, s_spur, bar_spur)
            .unwrap(),
        "landin_S_bar"
    );
    assert_eq!(
        result
            .trait_resolver
            .resolve_vtable_method(foo_spur, s_spur, baz_spur)
            .unwrap(),
        "landin_S_baz"
    );
    assert_eq!(
        result
            .trait_resolver
            .resolve_vtable_method(foo_spur, s_spur, qux_spur)
            .unwrap(),
        "landin_S_qux"
    );
}
