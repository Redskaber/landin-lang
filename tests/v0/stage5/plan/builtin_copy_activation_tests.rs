//! Stage 5.9: Builtin Copy activation tests
//!
//! Tests that `impl Copy for S` works WITHOUT the user defining
//! `trait Copy {}` — the compiler recognizes Copy as a builtin trait
//! (Stage 5.8 registry) and `is_copy_builtin()` (Stage 5.9) correctly
//! detects the impl.
//!
//! Also verifies the soundness fix: the old `ty_is_copy_with_resolver`
//! Adt branch fell back to `true` (treating all Adt as Copy) when "Copy"
//! wasn't interned. Stage 5.9 fixes this to `false` — only types with
//! an explicit `impl Copy for <Type>` are Copy.
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

/// `impl Copy for S` (without `trait Copy {}`) should make S be Copy.
#[test]
fn test_builtin_copy_works_without_trait_def() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .is_copy_builtin(s_def_id, &result.interner),
        "S with `impl Copy for S` (no `trait Copy {{}}`) should be Copy"
    );
}

/// Without `impl Copy for S`, S should NOT be Copy if it has a non-Copy field
/// or `impl Drop` (Stage 5.9 soundness fix + Stage 16.06 field-level derivation).
///
/// Stage 16.06: A struct with ALL Copy fields is DERIVED Copy (mirrors
/// Rust's `#[derive(Copy)]`). To test the "not Copy" path, we use a struct
/// with `impl Drop` (Copy+Drop conflict → not Copy).
#[test]
fn test_no_copy_impl_means_not_copy() {
    let result = compile("struct S; impl Drop for S { fn drop(self: &mut S) {} } fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        !result
            .trait_resolver
            .is_copy_builtin(s_def_id, &result.interner),
        "S with `impl Drop for S` should NOT be Copy (Copy+Drop conflict, Stage 5.9 soundness + Stage 16.06 derivation)"
    );
}

/// `trait Copy {}` + `impl Copy for S` should still work (backward compat).
#[test]
fn test_copy_works_with_user_trait_def() {
    let result = compile("trait Copy {} struct S; impl Copy for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");

    assert!(
        result
            .trait_resolver
            .is_copy_builtin(s_def_id, &result.interner),
        "S with `trait Copy {{}}` + `impl Copy for S` should be Copy"
    );
}

/// Multiple types: only those with `impl Copy` should be Copy.
/// Stage 16.06: Uses `impl Drop` to make B non-Copy (unit structs are
/// derived Copy when all fields — zero of them — are Copy).
#[test]
fn test_copy_selective_per_type() {
    let result = compile("struct A; struct B; impl Copy for A {} impl Drop for B { fn drop(self: &mut B) {} } fn main() {}");
    let a_def_id = find_type_def_id(&result, "A").expect("A should have a DefId");
    let b_def_id = find_type_def_id(&result, "B").expect("B should have a DefId");

    assert!(
        result
            .trait_resolver
            .is_copy_builtin(a_def_id, &result.interner),
        "A (with impl Copy) should be Copy"
    );
    assert!(
        !result
            .trait_resolver
            .is_copy_builtin(b_def_id, &result.interner),
        "B (with impl Drop) should NOT be Copy (Copy+Drop conflict, Stage 16.06)"
    );
}

/// The old `is_copy(def_id, copy_spur)` API should still work for backward
/// compatibility — it should give the same result as `is_copy_builtin`.
#[test]
fn test_is_copy_backward_compat() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
    let s_def_id = find_type_def_id(&result, "S").expect("S should have a DefId");
    let copy_spur = result
        .interner
        .get("Copy")
        .expect("Copy should be interned");

    let old_result = result.trait_resolver.is_copy(s_def_id, copy_spur);
    let new_result = result
        .trait_resolver
        .is_copy_builtin(s_def_id, &result.interner);

    assert_eq!(
        old_result, new_result,
        "is_copy() and is_copy_builtin() should agree"
    );
    assert!(old_result, "both should return true for `impl Copy for S`");
}
