//! Stage 16.08 — Task 3 Step 3: Builtin trait check migration tests.
#![allow(deprecated)] // Stage 16.11: tests verify deprecated Spur-based methods for backward compat
//!
//! These tests verify that the Stage 16.08 migration of
//! `is_copy_builtin`, `is_clone_builtin`, `is_drop_builtin`, and
//! `implements_builtin_trait` to use DefId-keyed lookup
//! (`implements_by_def_ids`) produces identical results to the old
//! Spur-based path (`implements_by_def_id`).
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify
//! the migration is behavior-preserving.
//! Per §1.0 原則 6 "通用 > 特例": one DefId-keyed lookup path for all.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.08 test 1: `is_copy_builtin` gives correct result for explicit Copy.
///
/// `struct S; impl Copy for S {}` — `is_copy_builtin` should return true.
#[test]
fn stage16_08_is_copy_builtin_explicit_copy() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
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
            .is_copy_builtin(s_def_id, &result.interner),
        "is_copy_builtin should return true for S with explicit impl Copy"
    );
}

/// Stage 16.08 test 2: `is_copy_builtin` gives correct result for derived Copy.
///
/// `struct Point { x: i32, y: i32 }` — no explicit impl Copy, but all
/// fields are Copy, so it's derived Copy (Stage 16.06).
#[test]
fn stage16_08_is_copy_builtin_derived_copy() {
    let result = compile("struct Point { x: i32, y: i32 } fn main() {}");
    let point_spur = result.interner.get("Point").expect("Point interned");
    let point_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == point_spur)
        .map(|(&d, _)| d)
        .expect("Point DefId");
    assert!(
        result
            .trait_resolver
            .is_copy_builtin(point_def_id, &result.interner),
        "is_copy_builtin should return true for Point (derived Copy, all-Copy fields)"
    );
}

/// Stage 16.08 test 3: `is_copy_builtin` gives correct result for non-Copy.
///
/// `struct S; impl Drop for S { fn drop(&mut S) {} }` — Copy+Drop conflict,
/// so S is NOT Copy.
#[test]
fn stage16_08_is_copy_builtin_non_copy_with_drop() {
    let result = compile("struct S; impl Drop for S { fn drop(&mut self) {} } fn main() {}");
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
            .is_copy_builtin(s_def_id, &result.interner),
        "is_copy_builtin should return false for S with impl Drop (Copy+Drop conflict)"
    );
}

/// Stage 16.08 test 4: `is_drop_builtin` gives correct result for explicit Drop.
///
/// `struct S; impl Drop for S { fn drop(&mut self) {} }` — `is_drop_builtin`
/// should return true.
#[test]
fn stage16_08_is_drop_builtin_explicit_drop() {
    let result = compile("struct S; impl Drop for S { fn drop(&mut self) {} } fn main() {}");
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
            .is_drop_builtin(s_def_id, &result.interner),
        "is_drop_builtin should return true for S with explicit impl Drop"
    );
}

/// Stage 16.08 test 5: `is_drop_builtin` gives correct result for no Drop.
///
/// `struct S;` (no impl Drop) — `is_drop_builtin` should return false.
#[test]
fn stage16_08_is_drop_builtin_no_drop() {
    let result = compile("struct S; fn main() {}");
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
            .is_drop_builtin(s_def_id, &result.interner),
        "is_drop_builtin should return false for S without impl Drop"
    );
}

/// Stage 16.08 test 6: `is_clone_builtin` gives correct result for explicit Clone.
///
/// `struct S; impl Clone for S { fn clone(&self) -> Self { S } }` —
/// `is_clone_builtin` should return true.
#[test]
fn stage16_08_is_clone_builtin_explicit_clone() {
    let result =
        compile("struct S; impl Clone for S { fn clone(&self) -> Self { S } } fn main() {}");
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
            .is_clone_builtin(s_def_id, &result.interner),
        "is_clone_builtin should return true for S with explicit impl Clone"
    );
}

/// Stage 16.08 test 7: `is_clone_builtin` gives correct result for no Clone.
///
/// `struct S;` (no impl Clone) — `is_clone_builtin` should return false.
#[test]
fn stage16_08_is_clone_builtin_no_clone() {
    let result = compile("struct S; fn main() {}");
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
            .is_clone_builtin(s_def_id, &result.interner),
        "is_clone_builtin should return false for S without impl Clone"
    );
}

/// Stage 16.08 test 8: `implements_builtin_trait` works for Copy.
///
/// `implements_builtin_trait(def_id, "Copy", interner)` should give the
/// same result as `is_copy_builtin(def_id, interner)`.
#[test]
fn stage16_08_implements_builtin_trait_copy() {
    let result = compile("struct S; impl Copy for S {} fn main() {}");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");
    let via_builtin = result
        .trait_resolver
        .is_copy_builtin(s_def_id, &result.interner);
    let via_generic =
        result
            .trait_resolver
            .implements_builtin_trait(s_def_id, "Copy", &result.interner);
    assert_eq!(
        via_builtin, via_generic,
        "is_copy_builtin and implements_builtin_trait(\"Copy\") should agree"
    );
}

/// Stage 16.08 test 9: `implements_builtin_trait` works for Drop.
///
/// `implements_builtin_trait(def_id, "Drop", interner)` should give the
/// same result as `is_drop_builtin(def_id, interner)`.
#[test]
fn stage16_08_implements_builtin_trait_drop() {
    let result = compile("struct S; impl Drop for S { fn drop(&mut self) {} } fn main() {}");
    let s_spur = result.interner.get("S").expect("S interned");
    let s_def_id = result
        .trait_resolver
        .type_by_def_id
        .iter()
        .find(|(_, &n)| n == s_spur)
        .map(|(&d, _)| d)
        .expect("S DefId");
    let via_builtin = result
        .trait_resolver
        .is_drop_builtin(s_def_id, &result.interner);
    let via_generic =
        result
            .trait_resolver
            .implements_builtin_trait(s_def_id, "Drop", &result.interner);
    assert_eq!(
        via_builtin, via_generic,
        "is_drop_builtin and implements_builtin_trait(\"Drop\") should agree"
    );
}

/// Stage 16.08 test 10: DefId-keyed and Spur-based lookups agree for explicit impls.
///
/// For types with EXPLICIT `impl Copy` or `impl Drop`, the DefId-keyed
/// path (`is_copy_builtin`) and Spur-based path (`implements_by_def_id`)
/// should agree. (Derived Copy from Stage 16.06 is a separate concern —
/// `is_copy_builtin` returns true for derived-Copy types, while
/// `implements_by_def_id` returns false since there's no explicit impl.)
#[test]
fn stage16_08_copy_def_id_and_spur_lookups_agree() {
    let result = compile(
        "struct A; struct B; impl Copy for A {} impl Drop for B { fn drop(&mut self) {} } fn main() {}",
    );
    let copy_spur = result.interner.get("Copy").expect("Copy interned");
    for type_name in &["A", "B"] {
        let type_spur = result.interner.get(type_name).expect("type interned");
        let type_def_id = result
            .trait_resolver
            .type_by_def_id
            .iter()
            .find(|(_, &n)| n == type_spur)
            .map(|(&d, _)| d)
            .expect("type DefId");
        // Old path: Spur-based implements_by_def_id
        let spur_result = result
            .trait_resolver
            .implements_by_def_id(copy_spur, type_def_id);
        // New path: is_copy_builtin (DefId-keyed, checks derived + explicit)
        let def_id_result = result
            .trait_resolver
            .is_copy_builtin(type_def_id, &result.interner);
        assert_eq!(
            spur_result, def_id_result,
            "Spur-based and DefId-based Copy lookups should agree for {} (explicit impl)",
            type_name
        );
    }
}
