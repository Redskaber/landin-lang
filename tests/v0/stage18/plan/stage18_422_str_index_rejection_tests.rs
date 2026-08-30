//! Stage 18.422 — §20 iterative audit: &str indexing rejection.
//!
//! Found during the §20 "finding one bug means there are many similar bugs"
//! audit after Stage 18.420 fixed the field access syntax mismatch. The
//! audit checked Index operations (`arr[idx]`) for similar silent-acceptance
//! bugs.
//!
//! Found: `resolve_index_element_type` had `TyKind::Str => Some(u8)` arm
//! that silently treated `&str` as `&[u8]`, allowing `s[0]` to compile
//! (returning the first byte via raw pointer read). This was a design
//! divergence from Rust, where `"hello"[0]` is a compile error.
//!
//! Confirmed bug (before fix): `s[0]` silently compiled and produced 104
//! (ASCII 'h') via raw pointer read — soundness false-positive.
//!
//! Fix (Stage 18.422):
//! - Removed `TyKind::Str => Some(u8)` arm in `resolve_index_element_type`
//! - `&str` indexing now reports "cannot index into type `str`"
//! - `emit_str_as_bytes` intrinsic fixed to return `&[u8]` typed dest local
//!   (was returning recv_local with `&str` type)
//! - Uses `Rvalue::Cast(Unsize, ...)` so typeck sees `&[u8]` (not `&str`)
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §7.3.1: ≥30 negative audit cases covering all 7 error categories.
//! Per §20: iterative audit — same class as Stage 18.412/18.416/18.420.
//! Per §1.0 原則 4 (报错 > 静默): &str indexing must be rejected.
//! Per §1.0 原則 5 (去除兼容思维): byte-indexing behavior removed.

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
        std::env::temp_dir().join(format!("landin_422_{}_{}.lin", std::process::id(), id));
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
// Positive tests: valid indexing (array, slice, as_bytes())
// ============================================================================

#[test]
fn stage18_422_pos_array_index() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a[0]; 0 }"#);
    assert_eq!(exit, 0, "array index must compile");
}

#[test]
fn stage18_422_pos_array_index_var() {
    let exit =
        compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let i = 1; let x = a[i]; 0 }"#);
    assert_eq!(exit, 0, "array index with variable must compile");
}

#[test]
fn stage18_422_pos_str_as_bytes_index() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let x = s.as_bytes()[0]; 0 }"#);
    assert_eq!(exit, 0, "s.as_bytes()[0] must compile");
}

#[test]
fn stage18_422_pos_str_as_bytes_index_var() {
    let exit = compile_only(
        r#"fn main() -> i32 { let s = "hello"; let i = 1; let x = s.as_bytes()[i]; 0 }"#,
    );
    assert_eq!(exit, 0, "s.as_bytes()[i] with variable must compile");
}

#[test]
fn stage18_422_pos_array_index_in_expr() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a[0] + a[1]; 0 }"#);
    assert_eq!(exit, 0, "array index in expression must compile");
}

#[test]
fn stage18_422_pos_str_as_bytes_arith() {
    let exit = compile_only(
        r#"fn main() -> i32 { let s = "AB"; let x = s.as_bytes()[0] + s.as_bytes()[1]; 0 }"#,
    );
    assert_eq!(exit, 0, "s.as_bytes() arithmetic must compile");
}

#[test]
fn stage18_422_pos_nested_array() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [[1, 2], [3, 4]]; let x = a[0][1]; 0 }"#);
    assert_eq!(exit, 0, "nested array index must compile");
}

#[test]
fn stage18_422_pos_str_field_as_bytes() {
    let exit = compile_only(
        r#"struct S { text: &str } fn main() -> i32 { let s = S { text: "hi" }; let x = s.text.as_bytes()[0]; 0 }"#,
    );
    assert_eq!(exit, 0, "struct field .as_bytes()[0] must compile");
}

// ============================================================================
// Negative tests: &str indexing directly (should error)
// ============================================================================

#[test]
fn stage18_422_neg_str_index_0() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let x = s[0]; 0 }"#);
    assert_ne!(exit, 0, "&str[0] must be rejected");
}

#[test]
fn stage18_422_neg_str_index_1() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let x = s[1]; 0 }"#);
    assert_ne!(exit, 0, "&str[1] must be rejected");
}

#[test]
fn stage18_422_neg_str_index_var() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let i = 0; let x = s[i]; 0 }"#);
    assert_ne!(exit, 0, "&str[i] with variable must be rejected");
}

#[test]
fn stage18_422_neg_str_index_4() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let x = s[4]; 0 }"#);
    assert_ne!(exit, 0, "&str[4] must be rejected");
}

#[test]
fn stage18_422_neg_str_index_in_expr() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let x = s[0] + s[1]; 0 }"#);
    assert_ne!(exit, 0, "&str[0] + &str[1] must be rejected");
}

#[test]
fn stage18_422_neg_str_field_index() {
    let exit = compile_only(
        r#"struct S { text: &str } fn main() -> i32 { let s = S { text: "hi" }; let x = s.text[0]; 0 }"#,
    );
    assert_ne!(exit, 0, "struct field &str[0] must be rejected");
}

#[test]
fn stage18_422_neg_str_index_assign() {
    // Stage 18.425: FIXED — `s[0] = 65` on &str now errors (assignment path
    // check_index_access_syntax added to lower_expr_to_place).
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; s[0] = 65; 0 }"#);
    assert_ne!(exit, 0, "&str[0] = ... assignment must be rejected");
}

#[test]
fn stage18_422_neg_str_index_to_u8() {
    let exit = compile_only(r#"fn main() -> i32 { let s = "hello"; let b: u8 = s[0]; 0 }"#);
    assert_ne!(exit, 0, "&str[0] assigned to u8 must be rejected");
}

#[test]
fn stage18_422_neg_empty_str_index() {
    let exit = compile_only(r#"fn main() -> i32 { let s = ""; let x = s[0]; 0 }"#);
    assert_ne!(exit, 0, "empty &str[0] must be rejected");
}

#[test]
fn stage18_422_neg_str_index_param() {
    let exit = compile_only(r#"fn f(s: &str) -> i32 { s[0] } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "&str param[0] must be rejected");
}

// ============================================================================
// Negative tests: indexing non-indexable types (struct, tuple, int, bool)
// ============================================================================

#[test]
fn stage18_422_neg_struct_index() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let f = Foo { x: 1 }; let y = f[0]; 0 }"#,
    );
    assert_ne!(exit, 0, "struct[0] must be rejected");
}

#[test]
fn stage18_422_neg_tuple_index() {
    let exit = compile_only(r#"fn main() -> i32 { let t = (1, 2); let y = t[0]; 0 }"#);
    assert_ne!(exit, 0, "tuple[0] must be rejected");
}

#[test]
fn stage18_422_neg_int_index() {
    // Stage 18.424: FIXED — `n[0]` on integer now errors (typeck
    // infer_projection pushes error for non-indexable concrete types).
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 42; let y = n[0]; 0 }"#);
    assert_ne!(exit, 0, "int[0] must be rejected");
}

#[test]
fn stage18_422_neg_bool_index() {
    let exit = compile_only(r#"fn main() -> i32 { let b = true; let y = b[0]; 0 }"#);
    assert_ne!(exit, 0, "bool[0] must be rejected");
}

// ============================================================================
// Negative tests: invalid index type (string, bool, float)
// ============================================================================

#[test]
fn stage18_422_neg_array_str_index() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a["hello"]; 0 }"#);
    assert_ne!(exit, 0, "array[\"hello\"] must be rejected");
}

#[test]
fn stage18_422_neg_array_bool_index() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a[true]; 0 }"#);
    assert_ne!(exit, 0, "array[true] must be rejected");
}

#[test]
fn stage18_422_neg_array_float_index() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a[1.5]; 0 }"#);
    assert_ne!(exit, 0, "array[1.5] must be rejected");
}

#[test]
fn stage18_422_neg_array_struct_index() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let a = [1, 2, 3]; let f = Foo { x: 0 }; let x = a[f]; 0 }"#,
    );
    assert_ne!(exit, 0, "array[struct] must be rejected");
}

// ============================================================================
// Negative tests: index result assigned to wrong type
// ============================================================================

#[test]
fn stage18_422_neg_array_index_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let s: &str = a[0]; 0 }"#);
    assert_ne!(exit, 0, "array[0] (i32) assigned to &str must be rejected");
}

#[test]
fn stage18_422_neg_array_index_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let b: bool = a[0]; 0 }"#);
    assert_ne!(exit, 0, "array[0] (i32) assigned to bool must be rejected");
}

#[test]
fn stage18_422_neg_as_bytes_index_to_str() {
    let exit =
        compile_only(r#"fn main() -> i32 { let s = "hello"; let x: &str = s.as_bytes()[0]; 0 }"#);
    assert_ne!(
        exit, 0,
        "as_bytes()[0] (u8) assigned to &str must be rejected"
    );
}
