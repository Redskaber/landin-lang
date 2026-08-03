//! Stage 16.06 — Field-level Copy derivation + Sound Copy migration tests.
//!
//! These tests verify the Stage 16.06 changes:
//! 1. TraitResolver derives Copy for structs/enums whose ALL fields are
//!    Copy (and no `impl Drop`), mirroring Rust's `#[derive(Copy)]`.
//! 2. The driver now uses `with_resolver_and_sigs` (sound Copy detection).
//! 3. The MIR lowerer uses `Operand::Move` for let bindings, function
//!    returns, and call arguments.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify
//! the sound Copy detection works end-to-end.
//! Per §1.0 原則 9 "正确 > 妥协": sound Copy is now the production path.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.06 test 1: struct with all primitive fields is derived Copy.
///
/// `struct Point { x: i32, y: i32 }` — no `impl Copy`, but all fields
/// (i32, i32) are Copy, so Point is DERIVED Copy. `let p2 = p` should
/// compile without "use of moved value" errors.
#[test]
fn stage16_06_struct_with_primitive_fields_is_derived_copy() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let p2 = p; p.x }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Point with all-Copy fields should be derived Copy; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 2: struct with `impl Drop` is NOT Copy.
///
/// `struct Counter { value: i32 } impl Drop for Counter` — even though
/// all fields are Copy, the `impl Drop` makes it non-Copy (Copy+Drop
/// conflict). Moving the same value twice should be rejected.
#[test]
fn stage16_06_struct_with_impl_drop_is_not_copy() {
    let src = "struct Counter { value: i32 } impl Drop for Counter { fn drop(self: &mut Counter) {} } fn main() -> i32 { let c = Counter { value: 1 }; let _c2 = c; let _c3 = c; 0 }";
    let result = compile(src);
    // Should have borrowck error (use of moved value — double move)
    assert!(
        !result.errors.borrowck.is_empty(),
        "Counter with impl Drop should NOT be Copy; double-move should be rejected"
    );
}

/// Stage 16.06 test 3: enum with all Copy variants is derived Copy.
///
/// `enum Color { Red, Green, Blue }` — all variants are unit (no data),
/// so Color is DERIVED Copy.
#[test]
fn stage16_06_enum_with_copy_variants_is_derived_copy() {
    let src =
        "enum Color { Red, Green, Blue } fn main() -> i32 { let c = Color::Red; let c2 = c; 0 }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Color enum with all-Copy variants should be derived Copy; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 4: nested struct with all Copy fields is derived Copy.
///
/// `struct Inner { x: i32 } struct Outer { inner: Inner }` — Inner is
/// derived Copy (all i32 fields), then Outer is derived Copy (Inner
/// field is Copy). Fixpoint iteration handles this.
#[test]
fn stage16_06_nested_struct_all_copy_is_derived_copy() {
    let src = "struct Inner { x: i32 } struct Outer { inner: Inner } fn main() -> i32 { let o = Outer { inner: Inner { x: 42 } }; let o2 = o; o.inner.x }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Nested structs with all-Copy fields should be derived Copy; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 5: explicit `impl Copy` still works alongside derivation.
///
/// `struct A; impl Copy for A {}` — explicit Copy. `struct B { x: i32 }`
/// — derived Copy (no explicit impl). Both should be Copy.
#[test]
fn stage16_06_explicit_and_derived_copy_coexist() {
    let src = "struct A; impl Copy for A {} struct B { x: i32 } fn main() -> i32 { let a = A; let a2 = a; let b = B { x: 1 }; let b2 = b; b.x }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Explicit impl Copy and derived Copy should coexist; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 6: struct with tuple of Copy fields is derived Copy.
///
/// `struct S { t: (i32, bool) }` — tuple (i32, bool) is Copy (all
/// elements Copy), so S is derived Copy.
#[test]
fn stage16_06_struct_with_tuple_of_copy_is_derived_copy() {
    let src = "struct S { t: (i32, bool) } fn main() -> i32 { let s = S { t: (1, true) }; let s2 = s; s.t.0 }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Struct with tuple-of-Copy field should be derived Copy; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 7: struct with array of Copy is derived Copy.
///
/// `struct S { arr: [i32; 3] }` — array [i32; 3] is Copy (element is
/// Copy), so S is derived Copy.
#[test]
fn stage16_06_struct_with_array_of_copy_is_derived_copy() {
    let src = "struct S { arr: [i32; 3] } fn main() -> i32 { let s = S { arr: [1, 2, 3] }; let s2 = s; s.arr[0] }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Struct with array-of-Copy field should be derived Copy; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 8: function returning non-Copy struct compiles.
///
/// `struct Counter { value: i32 } impl Drop for Counter` — function
/// returns Counter. The return should use Operand::Move (not Copy),
/// which doesn't trigger "does not implement Copy" error.
#[test]
fn stage16_06_function_returning_non_copy_struct_compiles() {
    let src = "struct Counter { value: i32 } impl Drop for Counter { fn drop(self: &mut Counter) {} } fn make(v: i32) -> Counter { Counter { value: v } } fn main() -> i32 { let c = make(42); c.value }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Function returning non-Copy struct should compile (Move, not Copy); got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 9: enum variant with non-Copy payload.
///
/// `struct Inner { x: i32 } impl Drop for Inner` — enum variant
/// `A(Inner)` has non-Copy payload. Constructing `E::A(Inner { x: 42 })`
/// should use Move for the argument, not Copy.
#[test]
fn stage16_06_enum_variant_with_non_copy_payload() {
    let src = "struct Inner { x: i32 } impl Drop for Inner { fn drop(self: &mut Inner) {} } enum E { A(Inner), B(i32) } fn main() -> i32 { let e = E::A(Inner { x: 42 }); 0 }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum variant with non-Copy payload should compile (Move arg); got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.06 test 10: is_copy_builtin returns true for derived Copy struct.
///
/// Directly queries `trait_resolver.is_copy_builtin` to verify the
/// derivation is registered in `derived_copy_types`.
#[test]
fn stage16_06_is_copy_builtin_returns_true_for_derived() {
    let result = compile("struct Point { x: i32, y: i32 } fn main() {}");
    // Find the DefId for Point
    let point_spur = result.interner.get("Point").expect("Point interned");
    let point_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == point_spur)
        .map(|(&d, _)| d)
        .expect("Point DefId found");
    assert!(
        result
            .trait_resolver
            .is_copy_builtin(point_def_id, &result.interner),
        "Point (all-Copy fields) should be derived Copy via is_copy_builtin"
    );
    assert!(
        result
            .trait_resolver
            .derived_copy_types
            .contains(&point_def_id),
        "Point should be in derived_copy_types set"
    );
}
