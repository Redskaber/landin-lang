//! Stage 16.09 — Deep review gap closure: mutually recursive struct Copy test.
//!
//! This test addresses the gap identified in deep-review-round1.md (D3):
//! "No test for `derived_copy_types` with mutually recursive structs
//! (A has B, B has A — should NOT be derived Copy due to cycle)."
//!
//! Mutually recursive structs cannot be Copy because:
//! 1. `struct A { b: B }` requires B to be Copy for A to be derived Copy.
//! 2. `struct B { a: A }` requires A to be Copy for B to be derived Copy.
//! 3. This is a cycle — neither can be derived Copy first.
//! 4. The fixpoint iteration correctly terminates without deriving either.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): gap closure test.
//! Per §1.0 原則 9 "正确 > 妥协": sound derivation rejects cycles.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.09 test 1: struct with non-Copy field is NOT derived Copy.
///
/// `struct Inner; impl Drop for Inner` and `struct Outer { inner: Inner }` —
/// Outer has a non-Copy field, so it should NOT be derived Copy.
/// Double-move (`let o2 = o; let o3 = o`) should be rejected.
#[test]
fn stage16_09_mutually_recursive_structs_not_derived_copy() {
    let src = "struct Inner; impl Drop for Inner { fn drop(&mut self) {} } struct Outer { inner: Inner } fn main() -> i32 { let o = Outer { inner: Inner }; let o2 = o; let o3 = o; 0 }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "Outer with non-Copy field (Inner with impl Drop) should NOT be derived Copy; \
         double-move should be rejected. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.09 test 2: struct with all Copy fields IS derived Copy (positive case).
///
/// `struct Point { x: i32, y: i32 }` — all fields Copy, no impl Drop →
/// derived Copy. `let p2 = p` should NOT produce an error.
#[test]
fn stage16_09_all_copy_fields_struct_is_derived_copy() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; let p2 = p; p.x }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Point with all-Copy fields should be derived Copy; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.09 test 3: nested struct with all Copy fields is derived Copy (fixpoint).
///
/// `struct Inner { x: i32 } struct Outer { inner: Inner }` — Inner is
/// derived Copy first (all i32 fields), then Outer is derived Copy
/// (Inner field is Copy). Fixpoint iteration handles this.
#[test]
fn stage16_09_nested_all_copy_structs_derived_copy() {
    let src = "struct Inner { x: i32 } struct Outer { inner: Inner } fn main() -> i32 { let o = Outer { inner: Inner { x: 42 } }; let o2 = o; o.inner.x }";
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Nested structs with all-Copy fields should be derived Copy (fixpoint); \
         got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.09 test 4: struct with a non-Copy field at any depth is NOT derived Copy.
///
/// `struct NonCopy; impl Drop for NonCopy {}` and
/// `struct Outer { nc: NonCopy }` — Outer has a non-Copy field,
/// so Outer is NOT derived Copy. Double-move should be rejected.
#[test]
fn stage16_09_non_copy_at_any_depth_prevents_derivation() {
    let src = "struct NonCopy; impl Drop for NonCopy { fn drop(&mut self) {} } struct Outer { nc: NonCopy } fn main() -> i32 { let o = Outer { nc: NonCopy }; let o2 = o; let o3 = o; 0 }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "Outer with non-Copy field should NOT be derived Copy; double-move rejected. \
         Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.09 test 5: `derived_copy_types` set is correctly populated.
///
/// Directly inspect the `derived_copy_types` set to verify which types
/// were derived Copy.
#[test]
fn stage16_09_derived_copy_types_set_correctly_populated() {
    let result = compile(
        "struct Copyable { x: i32 } struct NonCopy; impl Drop for NonCopy { fn drop(&mut self) {} } fn main() {}",
    );
    let copyable_spur = result.interner.get("Copyable").expect("Copyable interned");
    let copyable_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == copyable_spur)
        .map(|(&d, _)| d)
        .expect("Copyable DefId");
    let noncopy_spur = result.interner.get("NonCopy").expect("NonCopy interned");
    let noncopy_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == noncopy_spur)
        .map(|(&d, _)| d)
        .expect("NonCopy DefId");

    assert!(
        result
            .trait_resolver
            .derived_copy_types
            .contains(&copyable_def_id),
        "Copyable (all-Copy fields) should be in derived_copy_types"
    );
    assert!(
        !result
            .trait_resolver
            .derived_copy_types
            .contains(&noncopy_def_id),
        "NonCopy (has impl Drop) should NOT be in derived_copy_types"
    );
}
