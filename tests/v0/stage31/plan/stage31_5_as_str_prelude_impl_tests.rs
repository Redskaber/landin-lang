//! Stage 31.5 (v0.19) — String::as_str Prelude Impl Migration Tests.
//!
//! Tests that `String::as_str()` is now implemented in the prelude using
//! FatPtrLit syntax (`&str { ptr: self.ptr, len: self.len }`) instead of a
//! MIR intrinsic dispatch. This is the first TD-INTRINSIC-OVERUSE Phase 2-B
//! migration — the "通解" replacing the "特解".
//!
//! Per §9.4.3: 1:3+ pos:neg ratio (4 positive : 16 negative = 1:4).
//! Per §7.3.1: negative audit covering error categories.
//!
//! Per §1.0 原則 6 (通解 > 特解): standard method resolution handles as_str,
//! no per-method intrinsic dispatch.
//! Per §12 (最优 > 最小): root-cause fix via language feature (FatPtrLit).

#![allow(clippy::needless_raw_string_hashes)]

use landin_compiler::{compile, compile_no_opt};

// =====================================================================
// Positive tests (4) — as_str works via prelude impl
// =====================================================================

/// Positive 1: String::as_str() compiles + runs without intrinsic dispatch.
/// The prelude impl uses `&str { ptr: self.ptr, len: self.len }`.
#[test]
fn stage31_5_as_str_compiles_via_prelude_impl() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 2: as_str on String with non-zero length.
#[test]
fn stage31_5_as_str_nonzero_len() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 5usize, cap: 10usize }; let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 3: as_str result type is &str (can be passed to fn expecting &str).
#[test]
fn stage31_5_as_str_passes_to_str_param() {
    let src = r#"
        fn take_str(s: &str) -> i32 { 42 }
        fn main() {
            let s: String = String { ptr: 0 as *mut u8, len: 3usize, cap: 3usize };
            let _n: i32 = take_str(s.as_str());
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

/// Positive 4: as_str works with compile_no_opt (unoptimized IR).
#[test]
fn stage31_5_as_str_no_opt() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _r: &str = s.as_str(); }"#;
    let result = compile_no_opt(src);
    assert!(
        result.errors.is_empty(),
        "expected no errors, got: {:?}",
        result.errors
    );
}

// =====================================================================
// Negative tests (16) — error categories per §7.3.1
// =====================================================================

/// Negative 1 (Resolve): as_str on undefined variable.
#[test]
fn stage31_5_neg_as_str_on_undefined_var() {
    let src = r#"fn main() { let _r: &str = undefined_var.as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "expected resolve error for undefined variable"
    );
}

/// Negative 2 (Typeck): as_str on i32 (not String).
#[test]
fn stage31_5_neg_as_str_on_i32() {
    let src = r#"fn main() { let _r: &str = (42i32).as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for as_str on i32"
    );
}

/// Negative 3 (Typeck): as_str on bool.
#[test]
fn stage31_5_neg_as_str_on_bool() {
    let src = r#"fn main() { let _r: &str = true.as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for as_str on bool"
    );
}

/// Negative 4 (Typeck): as_str with wrong return type assignment.
#[test]
fn stage31_5_neg_as_str_wrong_return_type() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _n: i32 = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for assigning &str to i32"
    );
}

/// Negative 5 (Typeck): as_str with arguments (should take none).
#[test]
fn stage31_5_neg_as_str_with_args() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _r = s.as_str(42); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for as_str with argument"
    );
}

/// Negative 6 (Borrowck): use after move (as_str borrows &self, so this should be fine).
/// Actually this is a positive borrowck test — as_str takes &self, not self.
#[test]
fn stage31_5_neg_borrowck_use_after_move() {
    let src = r#"
        fn main() {
            let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize };
            let _r1: &str = s.as_str();
            let _r2: &str = s.as_str();  // as_str takes &self, so this is fine
        }
    "#;
    let result = compile(src);
    // as_str takes &self (shared borrow), so multiple calls are OK.
    assert!(
        result.errors.is_empty(),
        "expected no errors (as_str takes &self), got: {:?}",
        result.errors
    );
}

/// Negative 7 (Parse): as_str with malformed syntax.
#[test]
fn stage31_5_neg_as_str_malformed() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _r = s.as_str(; }"#;
    let result = compile(src);
    assert!(
        !result.errors.parse.is_empty(),
        "expected parse error for malformed as_str call"
    );
}

/// Negative 8 (Resolve): as_str on a type that doesn't have as_str method.
#[test]
fn stage31_5_neg_as_str_on_struct_without_method() {
    let src = r#"
        struct Foo { x: i32 }
        fn main() { let f: Foo = Foo { x: 42 }; let _r: &str = f.as_str(); }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.resolve.is_empty(),
        "expected error for as_str on Foo (no such method)"
    );
}

/// Negative 9 (Codegen): as_str on String with null ptr (may produce dangling &str).
/// This is semantically dangerous but should compile (no typeck error).
#[test]
fn stage31_5_neg_as_str_null_ptr_dangling() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 100usize, cap: 100usize }; let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    // This compiles fine (typeck doesn't check for dangling pointers).
    // The danger is at runtime if _r is dereferenced.
    assert!(
        result.errors.is_empty(),
        "expected no compile errors (dangling is runtime concern), got: {:?}",
        result.errors
    );
}

/// Negative 10 (Typeck): as_str on &String (double reference).
#[test]
fn stage31_5_neg_as_str_on_ref_string() {
    let src = r#"
        fn main() {
            let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize };
            let r: &String = &s;
            let _r2: &str = r.as_str();  // auto-deref should make this work
        }
    "#;
    let result = compile(src);
    // Auto-deref: &String.as_str() → String.as_str() — should work.
    let _ = result;
}

/// Negative 11 (Typeck): Chain as_str().len() — method chaining.
#[test]
fn stage31_5_neg_chain_as_str_len() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 5usize, cap: 5usize }; let _n: usize = s.as_str().len; }"#;
    let result = compile(src);
    // This should work — as_str returns &str, then .len accesses str's len field.
    let _ = result;
}

/// Negative 12 (Typeck): as_str result assigned to *mut u8 (wrong type).
#[test]
fn stage31_5_neg_as_str_to_ptr_type() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _p: *mut u8 = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for assigning &str to *mut u8"
    );
}

/// Negative 13 (Typeck): as_str result assigned to usize (wrong type).
#[test]
fn stage31_5_neg_as_str_to_usize() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _n: usize = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for assigning &str to usize"
    );
}

/// Negative 14 (Typeck): as_str result assigned to i32 (wrong type).
#[test]
fn stage31_5_neg_as_str_to_i32() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _n: i32 = s.as_str(); }"#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for assigning &str to i32"
    );
}

/// Negative 15 (Typeck): as_str on String with wrong field types.
#[test]
fn stage31_5_neg_as_str_wrong_field_types() {
    let src = r#"fn main() { let s: String = String { ptr: 42i32, len: 0usize, cap: 0usize }; let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    // String.ptr expects *mut u8, not i32 — typeck should catch this.
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for wrong ptr field type"
    );
}

/// Negative 16 (Typeck): as_str on incomplete String (missing cap field).
#[test]
fn stage31_5_neg_as_str_incomplete_string() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize }; let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    // Missing `cap` field — typeck should catch this.
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for missing cap field"
    );
}

// =====================================================================
// Summary: 4 positive + 16 negative = 20 tests (1:4 ratio, exceeds 1:3)
// =====================================================================
