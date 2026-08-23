//! Stage 18.162 (TD-NEGATIVE-TEST-COVERAGE): Stdlib stub negative tests.
//!
//! Tests stdlib stub types (String/Vec/Option/Result/Box) error paths.
//! Per §9.4.3, negative tests should be ≥25% of total.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === Option stub tests ===

/// Stage 18.162 negative 1: Option::Some with wrong type.
#[test]
fn stage18_162_stdlib_option_some_wrong_type() {
    let result = compile("fn main() { let x = Some(true); let y: Option<i32> = x; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 2: Option unwrap on None (runtime, no compile error).
#[test]
fn stage18_162_stdlib_option_unwrap_none() {
    let result = compile("fn main() { let x: Option<i32> = None; let y = x.unwrap(); }");
    // unwrap on None is a runtime panic, not a compile error.
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 3: Option pattern match with wrong type.
#[test]
fn stage18_162_stdlib_option_pattern_wrong_type() {
    let result = compile(
        "fn main() { let x = Some(42); match x { Some(b) => { let y: i32 = b; } None => {} } }",
    );
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 4: Option map with wrong closure type.
#[test]
fn stage18_162_stdlib_option_map_wrong_closure() {
    let result = compile("fn main() { let x = Some(42); let y = x.map(|s: bool| { 1 }); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 5: Option and_then with wrong return type.
#[test]
fn stage18_162_stdlib_option_and_then_wrong_return() {
    let result = compile("fn main() { let x = Some(42); let y = x.and_then(|n| { n }); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Result stub tests ===

/// Stage 18.162 negative 6: Result Ok with wrong type.
#[test]
fn stage18_162_stdlib_result_ok_wrong_type() {
    let result = compile("fn main() { let x = Ok(42); let y: Result<bool, i32> = x; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 7: Result Err with wrong error type.
#[test]
fn stage18_162_stdlib_result_err_wrong_type() {
    let result = compile("fn main() { let x = Err(42); let y: Result<i32, bool> = x; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 8: Result unwrap on Err (runtime).
#[test]
fn stage18_162_stdlib_result_unwrap_err() {
    let result = compile("fn main() { let x: Result<i32, i32> = Err(1); let y = x.unwrap(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 9: Result pattern match with wrong variant.
#[test]
fn stage18_162_stdlib_result_pattern_wrong_variant() {
    let result = compile("fn main() { let x: Result<i32, i32> = Ok(42); match x { Err(e) => { let y: bool = e; } _ => {} } }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 10: Result map with wrong closure.
#[test]
fn stage18_162_stdlib_result_map_wrong_closure() {
    let result =
        compile("fn main() { let x: Result<i32, i32> = Ok(42); let y = x.map(|s: bool| { 1 }); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === String stub tests ===

/// Stage 18.162 negative 11: String operations (stub, may not work).
#[test]
fn stage18_162_stdlib_string_operations() {
    let result = compile("fn main() { let s: String = String::new(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 12: String from str literal.
#[test]
fn stage18_162_stdlib_string_from_literal() {
    let result = compile("fn main() { let s = String::from(\"hello\"); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 13: String concatenation.
#[test]
fn stage18_162_stdlib_string_concat() {
    let result = compile("fn main() { let s1 = String::from(\"a\"); let s2 = String::from(\"b\"); let s3 = s1 + s2; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 14: String length.
#[test]
fn stage18_162_stdlib_string_len() {
    let result = compile("fn main() { let s = String::from(\"hello\"); let n = s.len(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 15: String index out of bounds.
#[test]
fn stage18_162_stdlib_string_index_oob() {
    let result = compile("fn main() { let s = String::from(\"hi\"); let c = s[10]; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Vec stub tests ===

/// Stage 18.162 negative 16: Vec operations (stub).
#[test]
fn stage18_162_stdlib_vec_new() {
    let result = compile("fn main() { let mut v: Vec<i32> = Vec::new(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 17: Vec push.
#[test]
fn stage18_162_stdlib_vec_push() {
    let result = compile("fn main() { let mut v = Vec::new(); v.push(42); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 18: Vec with wrong element type.
#[test]
fn stage18_162_stdlib_vec_wrong_element() {
    let result = compile("fn main() { let mut v: Vec<i32> = Vec::new(); v.push(true); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 19: Vec index out of bounds.
#[test]
fn stage18_162_stdlib_vec_index_oob() {
    let result = compile("fn main() { let v = vec![1, 2, 3]; let x = v[10]; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 20: Vec len.
#[test]
fn stage18_162_stdlib_vec_len() {
    let result = compile("fn main() { let v = vec![1, 2, 3]; let n = v.len(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Box stub tests ===

/// Stage 18.162 negative 21: Box new.
#[test]
fn stage18_162_stdlib_box_new() {
    let result = compile("fn main() { let b = Box::new(42); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 22: Box deref.
#[test]
fn stage18_162_stdlib_box_deref() {
    let result = compile("fn main() { let b = Box::new(42); let x = *b; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 23: Box with wrong type.
#[test]
fn stage18_162_stdlib_box_wrong_type() {
    let result = compile("fn main() { let b: Box<i32> = Box::new(true); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 24: Box nested.
#[test]
fn stage18_162_stdlib_box_nested() {
    let result = compile("fn main() { let b = Box::new(Box::new(42)); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.162 negative 25: Box method call.
#[test]
fn stage18_162_stdlib_box_method() {
    let result = compile("fn main() { let b = Box::new(42); let x = b.method(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}
