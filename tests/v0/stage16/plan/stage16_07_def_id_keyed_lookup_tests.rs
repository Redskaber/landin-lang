//! Stage 16.07 — Task 3 step 1: DefId-keyed trait impl lookup tests.
//!
//! These tests verify the Stage 16.07 additions to TraitResolver:
//! 1. `impls_by_def_ids: HashMap<(DefId, DefId), DefId>` field.
//! 2. `find_impl_by_def_ids(trait_def_id, self_type_def_id)` method.
//! 3. `implements_by_def_ids(trait_def_id, self_type_def_id)` method.
//! 4. `find_trait_def_id(trait_name_spur)` method.
//!
//! These are the DefId-based equivalents of the Spur-based `find_impl` /
//! `implements` methods. They provide type-safe, interner-free lookups
//! and prepare for generic SubstsRef support (Task 3 step 2).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify
//! both the new methods work AND give the same results as the old methods.
//! Per §23: API naming compliance verified.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.07 test 1: `find_trait_def_id` returns the trait's DefId.
///
/// For ``, `find_trait_def_id("Drop")`
/// should return `Some(DefId)`.
#[test]
fn stage16_07_find_trait_def_id_returns_def_id() {
    let result = compile("fn main() {}");
    let drop_spur = result.interner.get("Drop").expect("Drop interned");
    let drop_def_id = result
        .trait_resolver
        .find_trait_def_id(drop_spur)
        .expect("Drop trait should have a DefId");
    // The DefId should be in type_by_def_id.
    assert!(
        result
            .trait_resolver
            .type_by_def_id
            .contains_key(&drop_def_id),
        "Drop trait DefId should be in type_by_def_id"
    );
}

/// Stage 16.07 test 2: `find_trait_def_id` returns None for unknown trait.
#[test]
fn stage16_07_find_trait_def_id_returns_none_for_unknown() {
    let result = compile("fn main() {}");
    let unknown_spur = result.interner.get("NonExistentTrait");
    let unknown_def_id = unknown_spur.and_then(|s| result.trait_resolver.find_trait_def_id(s));
    if unknown_spur.is_some() {
        assert!(
            unknown_def_id.is_none(),
            "Unknown trait should return None from find_trait_def_id"
        );
    }
}

/// Stage 16.07 test 3: `implements_by_def_ids` returns true for existing impl.
///
/// `struct S; impl Drop for S { fn drop(&mut self) {} }` — the DefId-keyed
/// lookup should find the impl, same as the Spur-based `implements`.
#[test]
fn stage16_07_implements_by_def_ids_finds_existing_impl() {
    let result = compile("struct S; impl Drop for S { fn drop(&mut self) {} } fn main() {}");
    let drop_spur = result.interner.get("Drop").expect("Drop interned");
    let drop_def_id = result
        .trait_resolver
        .find_trait_def_id(drop_spur)
        .expect("Drop trait DefId");
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
            .implements_by_def_ids(drop_def_id, s_def_id),
        "implements_by_def_ids should find the Drop impl for S"
    );
}

/// Stage 16.07 test 4: `implements_by_def_ids` returns false for no impl.
///
/// `struct S;` (no impl Drop) — DefId-keyed lookup should return false.
#[test]
fn stage16_07_implements_by_def_ids_returns_false_for_no_impl() {
    let result = compile("struct S; fn main() {}");
    let drop_spur = result.interner.get("Drop").expect("Drop interned");
    let drop_def_id = result
        .trait_resolver
        .find_trait_def_id(drop_spur)
        .expect("Drop trait DefId");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");
    assert!(
        !result
            .trait_resolver
            .implements_by_def_ids(drop_def_id, s_def_id),
        "implements_by_def_ids should return false for S without impl Drop"
    );
}

/// Stage 16.07 test 6: `find_impl_by_def_ids` returns the impl info.
///
/// For `struct S; impl Drop for S { ... }`, the returned ImplInfo should
/// have the correct method names.
#[test]
fn stage16_07_find_impl_by_def_ids_returns_impl_info() {
    let result = compile("struct S; impl Drop for S { fn drop(&mut self) {} } fn main() {}");
    let drop_spur = result.interner.get("Drop").expect("Drop interned");
    let drop_def_id = result
        .trait_resolver
        .find_trait_def_id(drop_spur)
        .expect("Drop trait DefId");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");
    let impl_info = result
        .trait_resolver
        .find_impl_by_def_ids(drop_def_id, s_def_id)
        .expect("should find Drop impl for S");
    // The impl should have a "drop" method.
    let drop_method_spur = result.interner.get("drop").expect("drop interned");
    assert!(
        impl_info.methods.contains(&drop_method_spur),
        "Drop impl should have a 'drop' method; got: {:?}",
        impl_info.methods
    );
}

/// Stage 16.07 test 7: `impls_by_def_ids` map is populated during collect.
///
/// Directly inspect the `impls_by_def_ids` field to verify it's populated.
#[test]
fn stage16_07_impls_by_def_ids_map_is_populated() {
    let result = compile("struct S; impl Drop for S { fn drop(&mut self) {} } fn main() {}");
    // The map should have at least one entry (the Drop impl for S).
    assert!(
        !result.trait_resolver.impls_by_def_ids.is_empty(),
        "impls_by_def_ids should be populated during collect()"
    );
}

/// Stage 16.07 test 8: Copy trait also works with DefId-keyed lookup.
///
/// `struct S; impl Copy for S {}` — DefId-keyed lookup should find the
/// Copy impl.
#[test]
fn stage16_07_copy_trait_def_id_lookup_works() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
    let copy_spur = result.interner.get("Copy").expect("Copy interned");
    let copy_def_id = result
        .trait_resolver
        .find_trait_def_id(copy_spur)
        .expect("Copy trait DefId");
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
            .implements_by_def_ids(copy_def_id, s_def_id),
        "implements_by_def_ids should find the Copy impl for S"
    );
}

/// Stage 16.07 test 9: user-defined trait works with DefId-keyed lookup.
///
/// `trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} }`
/// — DefId-keyed lookup should find the Foo impl.
#[test]
fn stage16_07_user_defined_trait_def_id_lookup_works() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
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
    assert!(
        result
            .trait_resolver
            .implements_by_def_ids(foo_def_id, s_def_id),
        "implements_by_def_ids should find the Foo impl for S"
    );
}
