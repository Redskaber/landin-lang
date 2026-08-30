//! Stage 18.428 — §20 iterative audit: Deref validity check.
//!
//! Found during the §20 "finding one bug means there are many similar bugs"
//! audit after Stage 18.426 fixed the Cast validity. The audit checked Deref
//! operations (`*expr`) for similar silent-acceptance bugs.
//!
//! Found: typeck `infer_projection` for `ProjectionElem::Deref` returned
//! `TyKind::Error` WITHOUT pushing an error for non-pointer types. Invalid
//! derefs like `*42`, `*true`, `*"hello"`, `*(1,2)`, `*arr` silently compiled.
//!
//! Fix (Stage 18.428): Push error for Deref on concrete non-pointer types
//! (Int, Bool, Str, Tuple, Array, Adt, Float, Char). Defer for
//! Infer/Error/Param/Closure (pattern bindings + closure captures produce
//! Deref projections on these types — don't push false-positive errors).
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §20: iterative audit — same class as Stage 18.412/18.416/18.420/
//! 18.422/18.425/18.426.
//! Per §1.0 原則 4 (报错 > 静默): invalid derefs must be rejected.
//! Per §1.0 原則 9 (正确 > 妥协): defer for Closure (internal mechanism).

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn compile_only(code: &str) -> i32 {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_428_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("run landin-stage0");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
}

// ============================================================================
// Positive tests: valid deref (&T, &mut T, *const T, *mut T)
// ============================================================================

#[test]
fn stage18_428_pos_deref_ref() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42; let r = &x; let y = *r; 0 }"#);
    assert_eq!(exit, 0, "*&T must compile");
}

#[test]
fn stage18_428_pos_deref_mut_ref() {
    let exit = compile_only(r#"fn main() -> i32 { let mut x = 42; let r = &mut x; *r = 5; 0 }"#);
    assert_eq!(exit, 0, "*&mut T assignment must compile");
}

#[test]
fn stage18_428_pos_deref_raw_ptr() {
    let exit = compile_only(
        r#"fn main() -> i32 { let x = 42; let p = &x as *const i32; let y = unsafe { *p }; 0 }"#,
    );
    assert_eq!(exit, 0, "*const T must compile (with unsafe)");
}

#[test]
fn stage18_428_pos_deref_ref_in_expr() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42; let r = &x; let y = *r + 1; 0 }"#);
    assert_eq!(exit, 0, "*&T in expression must compile");
}

#[test]
fn stage18_428_pos_deref_nested_ref() {
    let exit = compile_only(
        r#"fn main() -> i32 { let x = 42; let r = &x; let rr = &r; let y = **rr; 0 }"#,
    );
    assert_eq!(exit, 0, "**&&T must compile");
}

// ============================================================================
// Negative tests: deref on non-pointer types (int, bool, float, char)
// ============================================================================

#[test]
fn stage18_428_neg_deref_int() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 42; let x = *n; 0 }"#);
    assert_ne!(exit, 0, "*i32 must be rejected");
}

#[test]
fn stage18_428_neg_deref_uint() {
    let exit = compile_only(r#"fn main() -> i32 { let n: u64 = 42; let x = *n; 0 }"#);
    assert_ne!(exit, 0, "*u64 must be rejected");
}

#[test]
fn stage18_428_neg_deref_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let b: bool = true; let x = *b; 0 }"#);
    assert_ne!(exit, 0, "*bool must be rejected");
}

#[test]
fn stage18_428_neg_deref_float() {
    let exit = compile_only(r#"fn main() -> i32 { let f: f64 = 1.5; let x = *f; 0 }"#);
    assert_ne!(exit, 0, "*f64 must be rejected");
}

#[test]
fn stage18_428_neg_deref_char() {
    let exit = compile_only(r#"fn main() -> i32 { let c: char = 'a'; let x = *c; 0 }"#);
    assert_ne!(exit, 0, "*char must be rejected");
}

// ============================================================================
// Negative tests: deref on aggregate types (tuple, array, struct)
// ============================================================================

#[test]
fn stage18_428_neg_deref_tuple() {
    let exit = compile_only(r#"fn main() -> i32 { let t = (1, 2); let x = *t; 0 }"#);
    assert_ne!(exit, 0, "*tuple must be rejected");
}

#[test]
fn stage18_428_neg_deref_array() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = *a; 0 }"#);
    assert_ne!(exit, 0, "*array must be rejected");
}

#[test]
fn stage18_428_neg_deref_struct() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let f = Foo { x: 1 }; let y = *f; 0 }"#,
    );
    assert_ne!(exit, 0, "*struct must be rejected");
}

#[test]
fn stage18_428_neg_deref_tuple_typed() {
    let exit = compile_only(r#"fn main() -> i32 { let t: (i32, i32) = (1, 2); let x = *t; 0 }"#);
    assert_ne!(exit, 0, "*typed tuple must be rejected");
}

// ============================================================================
// Negative tests: deref result assigned to wrong type
// ============================================================================

#[test]
fn stage18_428_neg_deref_result_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42; let r = &x; let s: &str = *r; 0 }"#);
    assert_ne!(exit, 0, "*&i32 (i32) assigned to &str must be rejected");
}

#[test]
fn stage18_428_neg_deref_result_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42; let r = &x; let b: bool = *r; 0 }"#);
    assert_ne!(exit, 0, "*&i32 (i32) assigned to bool must be rejected");
}

// ============================================================================
// Negative tests: deref in assignment path (LHS)
// ============================================================================

#[test]
fn stage18_428_neg_deref_int_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let n = 42; *n = 5; 0 }"#);
    assert_ne!(exit, 0, "*int = val assignment must be rejected");
}

#[test]
fn stage18_428_neg_deref_bool_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let b = true; *b = false; 0 }"#);
    assert_ne!(exit, 0, "*bool = val assignment must be rejected");
}

#[test]
fn stage18_428_neg_deref_tuple_assign() {
    let exit = compile_only(r#"fn main() -> i32 { let t = (1, 2); *t = (3, 4); 0 }"#);
    assert_ne!(exit, 0, "*tuple = val assignment must be rejected");
}
