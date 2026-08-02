//! Stage 15.52 — Region allocation integration tests.
//!
//! These tests verify that the region allocation pipeline (Stages 15.48-15.51)
//! works correctly on real MIR. The pipeline:
//! 1. MIR region assignment (Stage 15.49) — each &T gets a fresh Region::Var(vid).
//! 2. Constraint collection (Stage 15.50) — outlives constraints from MIR.
//! 3. Region inference (Stage 7.2) — fixpoint iteration.
//! 4. Error reporting (Stage 15.51) — errors converted to BorrowError.
//!
//! Since the current constraints are simplified (all regions effectively
//! map to 'static), no false positives should be produced. The tests verify
//! this — programs with references compile cleanly.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.52 test 1: Simple program with references compiles cleanly.
/// The region allocation pipeline should not produce false positives.
#[test]
fn stage15_52_ref_program_no_false_positives() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            *r
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Simple ref program should compile cleanly (no false positives from region inference). \
         Errors: {:?}",
        result.errors
    );
}

/// Stage 15.52 test 2: Program with multiple references compiles cleanly.
#[test]
fn stage15_52_multiple_refs_no_false_positives() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let y = 20;
            let rx = &x;
            let ry = &y;
            *rx + *ry
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "Multiple refs should compile cleanly");
}

/// Stage 15.52 test 3: Function taking references compiles cleanly.
#[test]
fn stage15_52_fn_with_ref_params_no_false_positives() {
    let src = r#"
        fn add(a: &i32, b: &i32) -> i32 {
            *a + *b
        }
        fn main() -> i32 {
            let x = 10;
            let y = 20;
            add(&x, &y)
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Function with ref params should compile cleanly"
    );
}

/// Stage 15.52 test 4: Function returning a reference compiles cleanly.
#[test]
fn stage15_52_fn_returning_ref_no_false_positives() {
    let src = r#"
        fn max(a: &i32, b: &i32) -> &i32 {
            if *a > *b { a } else { b }
        }
        fn main() -> i32 {
            let x = 10;
            let y = 20;
            *max(&x, &y)
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Function returning ref should compile cleanly"
    );
}

/// Stage 15.52 test 5: Loop with references compiles cleanly.
#[test]
fn stage15_52_loop_with_refs_no_false_positives() {
    let src = r#"
        fn main() -> i32 {
            let x = 10;
            let r = &x;
            let mut i = 0;
            let mut s = 0;
            while i < 3 { s = s + *r; i = i + 1; }
            s
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Loop with refs should compile cleanly"
    );
}

/// Stage 15.52 test 6: Struct with reference field compiles cleanly.
#[test]
fn stage15_52_struct_with_ref_no_false_positives() {
    let src = r#"
        struct Ref { val: &i32 }
        fn main() -> i32 {
            let x = 10;
            let r = Ref { val: &x };
            *r.val
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Struct with ref field should compile cleanly"
    );
}
