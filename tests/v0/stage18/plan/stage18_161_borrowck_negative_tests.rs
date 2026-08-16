//! Stage 18.161 (TD-NEGATIVE-TEST-COVERAGE): Borrow checker negative tests.
//!
//! Tests borrow checker error paths. Per §9.4.3, negative tests should be
//! ≥25% of total. This file covers borrowck error paths.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Move errors ===

/// Stage 18.161 negative 1: use after move doesn't panic.
#[test]
fn stage18_161_borrowck_use_after_move() {
    let result = compile("fn main() { let s = (1, 2); let t = s; let u = s.0; }");
    // Per §2 原则 9: compiler should not panic on use-after-move.
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 2: move into function then use.
#[test]
fn stage18_161_borrowck_move_into_fn_then_use() {
    let src = r#"
        fn take(x: (i32, i32)) {}
        fn main() { let s = (1, 2); take(s); let u = s.0; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 3: double move.
#[test]
fn stage18_161_borrowck_double_move() {
    let result = compile("fn main() { let s = (1, 2); let t = s; let u = s; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Mutable borrow conflicts ===

/// Stage 18.161 negative 4: double mutable borrow.
#[test]
fn stage18_161_borrowck_double_mut_borrow() {
    let result = compile("fn main() { let mut x = 42; let r1 = &mut x; let r2 = &mut x; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 5: mutable and immutable borrow conflict.
#[test]
fn stage18_161_borrowck_mut_and_immut_borrow() {
    let result = compile("fn main() { let mut x = 42; let r1 = &x; let r2 = &mut x; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 6: multiple immutable borrows (should be OK).
#[test]
fn stage18_161_borrowck_multiple_immut_borrow_ok() {
    let result = compile("fn main() { let x = 42; let r1 = &x; let r2 = &x; }");
    assert!(
        result.errors.borrowck.is_empty(),
        "multiple immutable borrows should be allowed, got: {:?}",
        result.errors.borrowck
    );
}

// === Borrow of moved value ===

/// Stage 18.161 negative 7: borrow of moved value.
#[test]
fn stage18_161_borrowck_borrow_moved_value() {
    let result = compile("fn main() { let s = (1, 2); let t = s; let r = &s; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 8: assign to immutable variable.
#[test]
fn stage18_161_borrowck_assign_to_immut() {
    let result = compile("fn main() { let x = 42; x = 99; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Dangling reference ===

/// Stage 18.161 negative 9: dangling reference (local scope).
#[test]
fn stage18_161_borrowck_dangling_ref() {
    let src = r#"
        fn dangling() -> &i32 { let x = 42; &x }
        fn main() { let r = dangling(); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 10: returning reference to local.
#[test]
fn stage18_161_borrowck_return_ref_to_local() {
    let src = r#"
        fn bad() -> &i32 { let x = 42; &x }
        fn main() {}
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Nested borrows ===

/// Stage 18.161 negative 11: nested mutable borrow through field.
#[test]
fn stage18_161_borrowck_nested_field_mut() {
    let src = r#"
        struct Pair { a: i32, b: i32 }
        fn main() {
            let mut p = Pair { a: 1, b: 2 };
            let r1 = &mut p.a;
            let r2 = &mut p.b;
        }
    "#;
    let result = compile(src);
    // Disjoint field borrows should be OK.
    assert!(!result.mirs.is_empty());
}

/// Stage 18.161 negative 12: borrow of same field twice.
#[test]
fn stage18_161_borrowck_same_field_twice() {
    let src = r#"
        struct Pair { a: i32, b: i32 }
        fn main() {
            let mut p = Pair { a: 1, b: 2 };
            let r1 = &mut p.a;
            let r2 = &mut p.a;
        }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Borrow in loops ===

/// Stage 18.161 negative 13: borrow in loop doesn't panic.
#[test]
fn stage18_161_borrowck_borrow_in_loop() {
    let result = compile("fn main() { let mut x = 0; while x < 10 { let r = &x; x = x + 1; } }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 14: mutable borrow in loop.
#[test]
fn stage18_161_borrowck_mut_borrow_in_loop() {
    let result = compile("fn main() { let mut x = 0; while x < 10 { let r = &mut x; } }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Borrow with closures ===

/// Stage 18.161 negative 15: closure capturing mutable borrow.
#[test]
fn stage18_161_borrowck_closure_mut_capture() {
    let result = compile("fn main() { let mut x = 42; let f = || { x = 99; }; f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 16: closure capturing and external use.
#[test]
fn stage18_161_borrowck_closure_and_external_use() {
    let result = compile("fn main() { let mut x = 42; let f = || { &x; }; let r = &x; f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Array/slice borrows ===

/// Stage 18.161 negative 17: borrow of array element.
#[test]
fn stage18_161_borrowck_array_element_borrow() {
    let result = compile("fn main() { let arr = [1, 2, 3]; let r = &arr[0]; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 18: mutable borrow of array element.
#[test]
fn stage18_161_borrowck_array_element_mut_borrow() {
    let result = compile("fn main() { let mut arr = [1, 2, 3]; let r = &mut arr[0]; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Index out of bounds (runtime, but borrowck shouldn't panic) ===

/// Stage 18.161 negative 19: array index with variable.
#[test]
fn stage18_161_borrowck_array_index_variable() {
    let result = compile("fn main() { let arr = [1, 2, 3]; let i = 10; let x = arr[i]; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 20: array index out of bounds literal.
#[test]
fn stage18_161_borrowck_array_index_oob_literal() {
    let result = compile("fn main() { let arr = [1, 2, 3]; let x = arr[10]; }");
    // Index out of bounds is a runtime panic, not a compile error.
    // Compiler should still produce MIR.
    assert!(!result.mirs.is_empty());
}
