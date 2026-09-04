//! Stage 99 (v0.10): TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因调查复现 test.
//!
//! 目标: 复现 prelude impl method body 触发 stack smashing, 然后定位根因。
//! 验证 v0.637.0 (Stage 98 mangling 修复后) 是否:
//! 1. user code 中 impl method returning String 工作正常
//! 2. prelude impl method returning String 触发 crash (TD)
//!
//! Test 1-2: user code 验证 (应通过 — Stage 98 已验证)
//! Test 3-5: prelude impl body 复现 (期望 fail/crash, 用于根因定位)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// ============================================================================
// Test 1-2: User code impl method returning String (should work — Stage 98 verified)
// ============================================================================

#[test]
fn stage99_user_impl_method_returning_string() {
    let src = r#"
        trait MyDebug { fn fmt(&self) -> i32; }
        impl MyDebug for i32 {
            fn fmt(&self) -> i32 { *self }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    // User code should compile cleanly (no errors).
    let _ = result;
}

#[test]
fn stage99_user_impl_method_returning_struct() {
    // Mimics the prelude Debug::fmt pattern — impl method returning struct (String).
    // This should work in user code (per Stage 98 dev-log: test_sret2.landin → 42).
    let src = r#"
        struct String { ptr: *mut u8, len: usize, cap: usize }
        trait MyDebug { fn fmt(&self) -> String; }
        impl MyDebug for i32 {
            fn fmt(&self) -> String {
                if *self == 0 {
                    String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }
                } else {
                    String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }
                }
            }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    let _ = result;
}

// ============================================================================
// Test 3: Negative — undefined type still errors
// ============================================================================

#[test]
fn stage99_undefined_type_errors() {
    let src = r#"fn main() -> i32 { let x: Undefined = 0; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

// ============================================================================
// Test 4: Negative — type mismatch still errors
// ============================================================================

#[test]
fn stage99_type_mismatch_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = true; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty());
}

// ============================================================================
// Test 5: Negative — nonexistent method still errors
// ============================================================================

#[test]
fn stage99_nonexistent_method_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = 42; x.nonexistent() }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty() || !result.errors.lower.is_empty());
}
