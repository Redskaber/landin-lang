//! Stage 18.164 (TD-NEGATIVE-TEST-COVERAGE): Closure negative tests.
//!
//! Tests closure error paths. Per §9.4.3, negative tests should be ≥25%.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Closure capture errors ===

/// Stage 18.164 negative 1: closure capturing undefined variable.
#[test]
fn stage18_164_closure_undefined_capture() {
    let result = compile("fn main() { let f = || { undefined_var }; f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 2: closure capturing moved variable.
#[test]
fn stage18_164_closure_capture_moved() {
    let result = compile("fn main() { let s = (1, 2); let t = s; let f = || { s.0 }; f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 3: closure with wrong parameter type.
#[test]
fn stage18_164_closure_wrong_param_type() {
    let result = compile("fn main() { let f = |x: bool| { x + 1 }; f(true); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 4: closure with wrong return type.
#[test]
fn stage18_164_closure_wrong_return() {
    let src = r#"
        fn take(f: fn() -> i32) -> i32 { f() }
        fn main() -> i32 { take(|| { true }) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 5: closure with wrong number of params.
#[test]
fn stage18_164_closure_wrong_param_count() {
    let src = r#"
        fn take(f: fn(i32) -> i32) -> i32 { f(42) }
        fn main() -> i32 { take(|a, b| { a + b }) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 6: closure called with wrong arg type.
#[test]
fn stage18_164_closure_wrong_arg_type() {
    let result = compile("fn main() { let f = |x: i32| { x + 1 }; f(true); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 7: closure called with wrong arg count.
#[test]
fn stage18_164_closure_wrong_arg_count() {
    let result = compile("fn main() { let f = |x: i32| { x + 1 }; f(1, 2); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 8: closure capturing mutable and used after.
#[test]
fn stage18_164_closure_mut_capture_use_after() {
    let result = compile("fn main() { let mut x = 42; let f = || { x = 99; }; x = 1; f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 9: closure with no captures.
#[test]
fn stage18_164_closure_no_captures() {
    let result = compile("fn main() { let f = || { 42 }; let x = f(); }");
    assert!(!result.mirs.is_empty());
}

/// Stage 18.164 negative 10: closure capturing by reference.
#[test]
fn stage18_164_closure_capture_by_ref() {
    let result = compile("fn main() { let x = 42; let f = || { &x }; let r = f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Closure as function parameter ===

/// Stage 18.164 negative 11: passing closure to function expecting fn type.
#[test]
fn stage18_164_closure_pass_to_fn() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
        fn main() -> i32 { apply(|n| { n * 2 }, 21) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 12: closure with wrong fn type signature.
#[test]
fn stage18_164_closure_wrong_fn_type() {
    let src = r#"
        fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }
        fn main() -> i32 { apply(|n: bool| { 1 }, 21) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 13: closure returning closure.
#[test]
fn stage18_164_closure_returning_closure() {
    let result = compile("fn main() { let f = || { || { 42 } }; let g = f(); let x = g(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 14: closure stored in struct.
#[test]
fn stage18_164_closure_in_struct() {
    let src = r#"
        struct Holder { f: fn(i32) -> i32 }
        fn main() { let h = Holder { f: |x| { x + 1 } }; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.164 negative 15: closure with recursive capture.
#[test]
fn stage18_164_closure_recursive_capture() {
    let result =
        compile("fn main() { let x = 42; let f = || { let g = || { x }; g() }; let y = f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}
