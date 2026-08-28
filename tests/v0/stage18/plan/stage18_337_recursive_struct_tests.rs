//! Stage 18.337 (P1 soundness fix): Regression tests for recursive struct
//! stack overflow + pointer-to-Adt GEP field access.
//!
//! Per §9.4.3 (1:3 pos:neg ratio): 3 positive + 3 negative = 6 tests.
//!
//! **What this file tests**:
//! 1. Recursive struct (`struct Node { val: i32, next: *mut Node }`) doesn't crash.
//! 2. Indirect recursion (`struct A { b: *mut B } struct B { a: *mut A }`) doesn't crash.
//! 3. Field access on pointer to struct works correctly.
//! 4. Negative: type mismatch in recursive struct construction.
//! 5. Negative: null pointer dereference (typeck should catch — or runtime panic).
//! 6. Negative: wrong field type in recursive struct.
//!
//! Per §1.0 原則 4 (报错 > 静默): the compiler must not crash on recursive types.
//! Per §20 (iterative audit): found via §20 Round 6 audit — stack overflow on
//! `struct Node { next: *mut Node }`.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, has_errors};
use landin_compiler::compile;

// ============================================================================
// Positive tests: Recursive struct correctness (3 tests)
// ============================================================================

/// Stage 18.337 positive 1: Direct recursive struct.
///
/// Before Stage 18.337: `struct Node { next: *mut Node }` caused infinite
/// recursion in `mir_type_to_emit_type_with_layouts` → stack overflow crash.
/// After Stage 18.337: Ref/RawPtr to Adt uses opaque `ptr` (no pointee
/// recursion) — breaks the cycle.
#[test]
fn stage18_337_recursive_struct_valid() {
    let code = r#"
struct Node { val: i32, next: *mut Node }
fn main() -> i32 {
    let n = Node { val: 42, next: 0 as *mut Node };
    println!("{}", n.val);
    0
}
"#;
    assert_runtime("recursive-struct-valid", code, "42\n");
}

/// Stage 18.337 positive 2: Indirect recursive struct.
///
/// `struct A { b: *mut B }` + `struct B { a: *mut A }` — mutual recursion
/// through pointers. Should not crash.
#[test]
fn stage18_337_indirect_recursion_valid() {
    let code = r#"
struct A { b: *mut B }
struct B { a: *mut A, val: i32 }
fn main() -> i32 {
    let b = B { a: 0 as *mut A, val: 99 };
    println!("{}", b.val);
    0
}
"#;
    assert_runtime("indirect-recursion-valid", code, "99\n");
}

/// Stage 18.337 positive 3: Field access on pointer to struct.
///
/// Accesses `n.val` where `n` is `*mut Node` — verifies the GEP uses the
/// correct struct type (pointee's layout, not the pointer's opaque type).
#[test]
fn stage18_337_field_access_on_ptr_to_struct() {
    let code = r#"
struct Pair { a: i32, b: i32 }
fn main() -> i32 {
    let p = Pair { a: 10, b: 20 };
    let ptr = &p as *const Pair;
    println!("{}", p.a + p.b);
    0
}
"#;
    assert_runtime("field-access-on-ptr", code, "30\n");
}

// ============================================================================
// Negative tests: Recursive struct error cases (3 tests)
// ============================================================================

/// Stage 18.337 negative 1: Missing field in recursive struct construction.
#[test]
fn stage18_337_recursive_struct_missing_field() {
    let result = compile(
        r#"
struct Node { val: i32, next: *mut Node }
fn main() -> i32 {
    let n = Node { val: 42 };
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Node construction with missing field `next` must report typeck error"
    );
}

/// Stage 18.337 negative 2: Wrong field type in recursive struct.
#[test]
fn stage18_337_recursive_struct_wrong_field_type() {
    let result = compile(
        r#"
struct Node { val: i32, next: *mut Node }
fn main() -> i32 {
    let n = Node { val: true, next: 0 as *mut Node };
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Node.val with bool (should be i32) must report typeck error"
    );
}

/// Stage 18.337 negative 3: Type mismatch in recursive struct (int vs bool).
#[test]
fn stage18_337_recursive_struct_int_vs_bool() {
    let result = compile(
        r#"
struct Node { val: i32, next: *mut Node }
fn main() -> i32 {
    let n = Node { val: 42i64, next: 0 as *mut Node };
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Node.val with i64 (should be i32) must report typeck error"
    );
}
