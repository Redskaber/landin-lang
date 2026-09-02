//! Stage 16.10 — Task 3 Step 3 continuation: DefId-keyed vtable lookup tests.
//!
//! These tests verify the Stage 16.10 additions to TraitResolver:
//! 1. `vtables_by_def_ids: HashMap<(DefId, DefId), Vtable>` field.
//! 2. `find_vtable_by_def_ids(trait_def_id, self_type_def_id)` method.
//! 3. `populate_def_id_keyed_maps()` post-pass in `collect()`.
//! 4. `dyn_trait.rs` migration to DefId-keyed vtable lookup (with fallback).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify
//! the new DefId-keyed vtable lookup works for both builtin and
//! user-defined traits.
//! Per §23: API naming compliance verified.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.10 test 1: `find_vtable_by_def_ids` returns the vtable for a
/// user-defined trait impl.
///
/// `trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} }`
/// — the DefId-keyed lookup should find the vtable with the "bar" method.
#[test]
fn stage16_10_find_vtable_by_def_ids_user_defined_trait() {
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
    let vtable = result
        .trait_resolver
        .find_vtable_by_def_ids(foo_def_id, s_def_id)
        .expect("should find vtable for Foo impl on S");
    let bar_spur = result.interner.get("bar").expect("bar interned");
    assert!(
        vtable.entries.iter().any(|e| e.method_name == bar_spur),
        "vtable should have a 'bar' method entry"
    );
}

/// Stage 16.10 test 2: `find_vtable_by_def_ids` returns None for no impl.
///
/// `trait Foo { fn bar(&self); } struct S;` (no impl Foo for S) —
/// DefId-keyed lookup should return None.
#[test]
fn stage16_10_find_vtable_by_def_ids_returns_none_for_no_impl() {
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
            .find_vtable_by_def_ids(foo_def_id, s_def_id)
            .is_none(),
        "find_vtable_by_def_ids should return None for S without impl Foo"
    );
}

/// Stage 16.10 test 3: `vtables_by_def_ids` map is populated during collect.
///
/// Directly inspect the `vtables_by_def_ids` field to verify it's populated.
#[test]
fn stage16_10_vtables_by_def_ids_map_is_populated() {
    let result = compile(
        "trait Foo { fn bar(&self); } struct S; impl Foo for S { fn bar(&self) {} } fn main() {}",
    );
    assert!(
        !result.trait_resolver.vtables_by_def_ids.is_empty(),
        "vtables_by_def_ids should be populated during collect()"
    );
}

/// Stage 16.10 test 4: DefId-keyed and Spur-based vtable lookups agree.
////// Stage 16.10 test 5: `find_vtable_by_def_ids` works with multiple methods.
///
/// `trait Greet { fn hello(&self); fn bye(&self); } struct Person; impl Greet for Person`
/// — the vtable should have 2 entries (hello + bye) in the correct order.
#[test]
fn stage16_10_find_vtable_by_def_ids_multiple_methods() {
    let result = compile(
        "trait Greet { fn hello(&self); fn bye(&self); } struct Person; impl Greet for Person { fn hello(&self) {} fn bye(&self) {} } fn main() {}",
    );
    let greet_spur = result.interner.get("Greet").expect("Greet interned");
    let greet_def_id = result
        .trait_resolver
        .find_trait_def_id(greet_spur)
        .expect("Greet trait DefId");
    let person_spur = result.interner.get("Person").expect("Person interned");
    let person_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == person_spur)
        .map(|(&d, _)| d)
        .expect("Person DefId");
    let vtable = result
        .trait_resolver
        .find_vtable_by_def_ids(greet_def_id, person_def_id)
        .expect("should find vtable for Greet impl on Person");
    assert_eq!(
        vtable.entries.len(),
        2,
        "vtable should have 2 entries (hello + bye)"
    );
    let hello_spur = result.interner.get("hello").expect("hello interned");
    let bye_spur = result.interner.get("bye").expect("bye interned");
    assert_eq!(
        vtable.entries[0].method_name, hello_spur,
        "first entry should be hello"
    );
    assert_eq!(
        vtable.entries[1].method_name, bye_spur,
        "second entry should be bye"
    );
}

/// Stage 16.10 test 6: `populate_def_id_keyed_maps` post-pass handles HIR
/// iteration ordering for user-defined traits.
///
/// This test verifies that user-defined traits (which may appear after their
/// impls in HIR iteration) are correctly resolved by the post-pass.
/// The test compiles a program where the trait is defined AFTER the impl
/// in source order, which may affect HIR iteration order.
#[test]
fn stage16_10_post_pass_handles_user_defined_trait_ordering() {
    // Note: HIR owners is a HashMap, so iteration order is non-deterministic.
    // This test verifies the post-pass works regardless of source order.
    let result = compile(
        "struct S; impl Foo for S { fn bar(&self) {} } trait Foo { fn bar(&self); } fn main() {}",
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
    // The post-pass should have populated vtables_by_def_ids even though
    // the trait was defined after the impl in source order.
    assert!(
        result
            .trait_resolver
            .find_vtable_by_def_ids(foo_def_id, s_def_id)
            .is_some(),
        "post-pass should populate vtables_by_def_ids for user-defined traits regardless of HIR order"
    );
}

/// Stage 16.10 test 7: dyn Trait method calls work with DefId-keyed vtable.
///
/// Verifies that `build_dyn_trait_method_calls_from_resolver` produces the
/// correct method calls when using the DefId-keyed vtable lookup path.
#[test]
fn stage16_10_dyn_trait_method_calls_with_def_id_vtable() {
    use landin_compiler::mir::dyn_trait::build_dyn_trait_method_calls_from_resolver;
    let result = compile(
        "trait Greet { fn hello(&self); fn bye(&self); } struct Person; impl Greet for Person { fn hello(&self) {} fn bye(&self) {} } fn main() {}",
    );
    let calls =
        build_dyn_trait_method_calls_from_resolver(&result.trait_resolver, &result.interner);
    assert!(
        calls.len() >= 2,
        "expected at least 2 method calls (hello + bye + prelude Clone methods), got {}",
        calls.len()
    );
    let method_names: Vec<&str> = calls.iter().map(|c| c.method_name.as_str()).collect();
    assert!(
        method_names.contains(&"hello"),
        "should have 'hello' method"
    );
    assert!(method_names.contains(&"bye"), "should have 'bye' method");
}
