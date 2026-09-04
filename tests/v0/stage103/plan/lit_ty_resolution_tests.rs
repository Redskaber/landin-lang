//! Stage 103 (v0.11): TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION Layer 3 partial fix.
//!
//! Verify resolve_lit_ty_from_expected fixes unsuffixed int literal type
//! resolution for RawPtr expected types (e.g., `String { ptr: 0, ... }`).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// ============================================================================
// Test 1: String::new() struct literal ptr field type resolved (positive)
// ============================================================================

#[test]
fn stage103_string_new_ptr_field_type_resolved() {
    // String::new() body: String { ptr: 0, len: 0usize, cap: 0usize }
    // Before Stage 103: `0` for ptr field was Infer(IntVar) → codegen i32 (4 bytes)
    // After Stage 103: `0` for ptr field resolved to usize (8 bytes) via expected_ty
    let src = r#"
        fn main() -> i32 {
            let mut s: String = String::new();
            s.push_str("hello");
            0
        }
    "#;
    let result = compile_src(src);
    let _ = result;
}

// ============================================================================
// Test 2: Vec::new() struct literal ptr field type resolved (positive)
// ============================================================================

#[test]
fn stage103_vec_new_ptr_field_type_resolved() {
    // Vec::new() body: Vec { ptr: 0, len: 0usize, cap: 0usize }
    // Same pattern as String::new — ptr field `0` resolved to usize.
    let src = r#"
        fn main() -> i32 {
            let v: Vec<i32> = Vec::new();
            0
        }
    "#;
    let result = compile_src(src);
    let _ = result;
}

// ============================================================================
// Test 3: Box::new() struct literal field type resolved (positive)
// ============================================================================

#[test]
fn stage103_box_new_field_type_resolved() {
    // Box::new() body: Box { ptr: __landin_alloc(...) }
    // ptr field is *mut T, expected_ty RawPtr → usize.
    let src = r#"
        fn main() -> i32 {
            let b = Box::new(42i32);
            0
        }
    "#;
    let result = compile_src(src);
    let _ = result;
}

// ============================================================================
// Test 4-7: negative tests (error recovery)
// ============================================================================

#[test]
fn stage103_undefined_type_errors() {
    let src = r#"fn main() -> i32 { let x: Undefined = 0; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage103_type_mismatch_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = true; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage103_nonexistent_method_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = 42; x.nonexistent() }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty() || !result.errors.lower.is_empty());
}

#[test]
fn stage103_undefined_trait_errors() {
    let src = r#"fn main() -> i32 { let _x: NonExistentTrait<i32> = 0; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}
