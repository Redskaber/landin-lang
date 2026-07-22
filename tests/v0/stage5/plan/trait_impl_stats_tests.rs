//! Stage 5.13: Trait impl statistics tests
//!
//! Tests that `impl_count_for_type()`, `impl_count_for_trait()`,
//! `builtin_trait_count()`, and `traits_for_type()` correctly count and
//! list trait implementations.
//!
//! Per §16: tests use the `compile()` public API.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::compile;

/// Helper: find the DefId of a type by name in the resolver.
fn find_type_def_id(
    result: &landin_compiler::CompileResult,
    name: &str,
) -> Option<landin_compiler::hir::DefId> {
    let name_spur = result.interner.get(name)?;
    result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == name_spur)
        .map(|(&d, _)| d)
}

/// `impl_count_for_type` should count impls for a specific type.
#[test]
fn test_impl_count_for_type() {
    let result = compile(
        "trait Foo {} trait Bar {} struct S; impl Foo for S {} impl Bar for S {} fn main() {}",
    );
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    // S has 2 impls: Foo and Bar
    assert_eq!(
        result.trait_resolver.impl_count_for_type(s_def_id),
        2,
        "S should have 2 trait impls"
    );
}

/// `impl_count_for_type` should return 0 for a type with no impls.
#[test]
fn test_impl_count_for_type_zero() {
    let result = compile("trait Foo {} struct S; fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert_eq!(
        result.trait_resolver.impl_count_for_type(s_def_id),
        0,
        "S with no impls should have 0 trait impls"
    );
}

/// `impl_count_for_trait` should count impls for a specific trait.
#[test]
fn test_impl_count_for_trait() {
    let result = compile(
        "trait Foo {} struct A; struct B; impl Foo for A {} impl Foo for B {} fn main() {}",
    );
    let foo_spur = result.interner.get("Foo").expect("Foo interned");

    // Foo has 2 impls: for A and for B
    assert_eq!(
        result.trait_resolver.impl_count_for_trait(foo_spur),
        2,
        "Foo should have 2 impls"
    );
}

/// `impl_count_for_trait` should return 0 for a trait with no impls.
#[test]
fn test_impl_count_for_trait_zero() {
    let result = compile("trait Foo {} struct S; fn main() {}");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");

    assert_eq!(
        result.trait_resolver.impl_count_for_trait(foo_spur),
        0,
        "Foo with no impls should have 0"
    );
}

/// `builtin_trait_count` should return the number of builtin traits (10).
#[test]
fn test_builtin_trait_count() {
    let result = compile("fn main() {}");
    assert_eq!(
        result.trait_resolver.builtin_trait_count(),
        10,
        "should have 10 builtin traits"
    );
}

/// `traits_for_type` should return all trait names a type implements.
#[test]
fn test_traits_for_type() {
    let result = compile(
        "trait Foo {} trait Bar {} struct S; impl Foo for S {} impl Bar for S {} fn main() {}",
    );
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");
    let foo_spur = result.interner.get("Foo").expect("Foo interned");
    let bar_spur = result.interner.get("Bar").expect("Bar interned");

    let traits = result.trait_resolver.traits_for_type(s_def_id);
    assert_eq!(traits.len(), 2, "S should implement 2 traits");
    assert!(
        traits.contains(&foo_spur),
        "traits_for_type should include Foo"
    );
    assert!(
        traits.contains(&bar_spur),
        "traits_for_type should include Bar"
    );
}

/// `traits_for_type` should return empty Vec for a type with no impls.
#[test]
fn test_traits_for_type_empty() {
    let result = compile("trait Foo {} struct S; fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    let traits = result.trait_resolver.traits_for_type(s_def_id);
    assert!(
        traits.is_empty(),
        "S with no impls should have empty traits_for_type"
    );
}
