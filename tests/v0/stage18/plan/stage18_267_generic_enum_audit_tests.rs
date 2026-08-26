//! Stage 18.267 — Extended holistic audit: generic enum variants.
//!
//! Per §17.6 "直到审查不出问题为止" — continue auditing similar
//! patterns. The struct literal + Box::new fixes used Holder<T> tuple
//! struct. Now audit:
//! - Option::Some(Holder(true)) where expected is Option<Holder<i32>>
//! - Result::Ok(Holder(true)) where expected is Result<Holder<i32>, E>
//! - Generic enum variant with struct-style fields
//! - Nested generic types (Box<Box<Holder<i32>>>)
//!
//! Per §1.0 原則 9 (正确 > 妥协): full soundness across all generic
//! type contexts.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Option::Some — `let x: Option<Holder<i32>> = Some(Holder(true))`
// ============================================================================

#[test]
fn test_audit_option_some_with_wrong_inner_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[Option::Some inner] Some(Holder(true)) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: Option::Some inner ctor may not propagate expected_ty");
    }
}

// ============================================================================
// Result::Ok — `let x: Result<Holder<i32>, E> = Ok(Holder(true))`
// ============================================================================

#[test]
fn test_audit_result_ok_with_wrong_inner_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Result<Holder<i32>, i32> = Ok(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[Result::Ok inner] Ok(Holder(true)) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: Result::Ok inner ctor may not propagate expected_ty");
    }
}

// ============================================================================
// Nested Box — `let b: Box<Box<Holder<i32>>> = Box::new(Box::new(Holder(true)))`
// ============================================================================

#[test]
fn test_audit_nested_box_with_wrong_inner_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Box<Holder<i32>>> = Box::new(Box::new(Holder(true)));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[nested Box] Box::new(Box::new(Holder(true))) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: nested Box::new may not propagate expected_ty");
    }
}

// ============================================================================
// Vec::push — `v.push(Holder(true))` where v: Vec<Holder<i32>>
// ============================================================================

#[test]
fn test_audit_vec_push_with_wrong_inner_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let mut v: Vec<Holder<i32>> = Vec::new();
            v.push(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[Vec::push arg] v.push(Holder(true)) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: Vec::push arg may not propagate expected_ty");
    }
}

// ============================================================================
// Vec::get — `v.get(0)` returns Option<Holder<i32>>, then unwrap_or(Holder(true))
// ============================================================================

#[test]
fn test_audit_vec_get_then_unwrap_or_with_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let v: Vec<Holder<i32>> = Vec::new();
            let h = v.get(0).unwrap_or(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[Vec::get + unwrap_or] v.get(0).unwrap_or(Holder(true)) — has_errors = {} (expected: true if unwrap_or checks)",
        result.has_errors()
    );
}
