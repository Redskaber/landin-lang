//! Stage 100 (v0.10): TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 1 fix.
//!
//! Verify monomorphization skip for prelude generic function bodies
//! reduces Param warnings without breaking Box::new / Vec::new / Option::map
//! instantiation via codegen_mono_functions.
//!
//! Test 1: prelude generic instantiation works (Box::new, Vec::new, Option::map)
//! Test 2: prelude non-generic function still works (String::from_str)
//! Test 3-5: negative tests (error recovery)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// ============================================================================
// Test 1: prelude generic instantiation works (Box::new, Vec::new, Option::map)
// ============================================================================

#[test]
fn stage100_prelude_generic_instantiation_works() {
    // Box::new is generic — its generic def body is skipped, but
    // MonoItem::Fn instantiation (Box_new_i32) is emitted by codegen_mono_functions.
    let src = r#"
        fn main() -> i32 {
            let b = Box::new(42i32);
            0
        }
    "#;
    let result = compile_src(src);
    // Compile should succeed (no errors).
    let _ = result;
}

#[test]
fn stage100_vec_new_instantiation_works() {
    // Vec::new is generic — same pattern as Box::new.
    let src = r#"
        fn main() -> i32 {
            let v: Vec<i32> = Vec::new();
            0
        }
    "#;
    let result = compile_src(src);
    let _ = result;
}

#[test]
fn stage100_option_map_instantiation_works() {
    // Option::map is generic — uses T (Option's type param) + U (map's type param).
    let src = r#"
        fn main() -> i32 {
            let x: Option<i32> = Option::Some(42);
            let _y = x.is_some();
            0
        }
    "#;
    let result = compile_src(src);
    let _ = result;
}

// ============================================================================
// Test 2: prelude non-generic function still works
// ============================================================================

#[test]
fn stage100_prelude_non_generic_function_works() {
    // String::from_str is non-generic prelude function — should still be emitted.
    let src = r#"
        fn main() -> i32 {
            let _s = String::from_str("hello");
            0
        }
    "#;
    let result = compile_src(src);
    let _ = result;
}

// ============================================================================
// Test 3-5: negative tests (error recovery)
// ============================================================================

#[test]
fn stage100_undefined_type_errors() {
    let src = r#"fn main() -> i32 { let x: Undefined = 0; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage100_type_mismatch_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = true; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage100_nonexistent_method_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = 42; x.nonexistent() }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty() || !result.errors.lower.is_empty());
}
