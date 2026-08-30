//! Stage 18.426 — §20 iterative audit: Cast validity check.
//!
//! Found during the §20 "finding one bug means there are many similar bugs"
//! audit after Stage 18.425 fixed the Index typeck. The audit checked Cast
//! operations (`expr as Type`) for similar silent-acceptance bugs.
//!
//! Found: typeck `infer_rvalue` for `Rvalue::Cast` returned `target_ty`
//! without checking source type. Invalid casts like `true as &str`,
//! `(1,2) as i32`, `42 as Foo`, `42 as [i32; 3]` silently compiled.
//! Codegen fell through to `_ => "bitcast"` fallback producing wrong/invalid
//! LLVM IR.
//!
//! Fix (Stage 18.426): Added `is_valid_cast` helper in typeck `infer_rvalue`
//! Cast arm. Validates cast pairs against Rust Reference §5.2.7 rules:
//! - Numeric casts (Int/Uint/Float/Bool/Char): per Rust rules
//! - Int↔Ptr, Ptr↔Ptr: OK
//! - Unsize: OK (checked at codegen)
//! - FnDef→FnPtr: OK (reify)
//! - All others (Str/Tuple/Adt/Array): rejected
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §20: iterative audit — same class as Stage 18.412/18.416/18.420/
//! 18.422/18.425.
//! Per §1.0 原則 4 (报错 > 静默): invalid casts must be rejected.

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
        std::env::temp_dir().join(format!("landin_426_{}_{}.lin", std::process::id(), id));
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
// Positive tests: valid casts (numeric, pointer, char, bool)
// ============================================================================

#[test]
fn stage18_426_pos_int_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42i32 as i64; 0 }"#);
    assert_eq!(exit, 0, "i32 as i64 must compile");
}

#[test]
fn stage18_426_pos_int_to_uint() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42i32 as u64; 0 }"#);
    assert_eq!(exit, 0, "i32 as u64 must compile");
}

#[test]
fn stage18_426_pos_float_to_float() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.5f64 as f32; 0 }"#);
    assert_eq!(exit, 0, "f64 as f32 must compile");
}

#[test]
fn stage18_426_pos_int_to_float() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42i32 as f64; 0 }"#);
    assert_eq!(exit, 0, "i32 as f64 must compile");
}

#[test]
fn stage18_426_pos_float_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.5f64 as i32; 0 }"#);
    assert_eq!(exit, 0, "f64 as i32 must compile");
}

#[test]
fn stage18_426_pos_bool_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true as i32; 0 }"#);
    assert_eq!(exit, 0, "bool as i32 must compile");
}

#[test]
fn stage18_426_pos_int_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 as bool; 0 }"#);
    assert_eq!(exit, 0, "i32 as bool must compile");
}

#[test]
fn stage18_426_pos_char_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 'a' as i32; 0 }"#);
    assert_eq!(exit, 0, "char as i32 must compile");
}

#[test]
fn stage18_426_pos_int_to_char() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 65i32 as char; 0 }"#);
    assert_eq!(exit, 0, "i32 as char must compile");
}

#[test]
fn stage18_426_pos_int_to_ptr() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 0 as *mut u8; 0 }"#);
    assert_eq!(exit, 0, "int as *mut u8 must compile");
}

// ============================================================================
// Negative tests: invalid casts (str, tuple, struct, array)
// ============================================================================

#[test]
fn stage18_426_neg_str_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let x = "hello" as i32; 0 }"#);
    assert_ne!(exit, 0, "&str as i32 must be rejected");
}

#[test]
fn stage18_426_neg_bool_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true as &str; 0 }"#);
    assert_ne!(exit, 0, "bool as &str must be rejected");
}

#[test]
fn stage18_426_neg_int_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42i32 as &str; 0 }"#);
    assert_ne!(exit, 0, "i32 as &str must be rejected");
}

#[test]
fn stage18_426_neg_tuple_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let x = (1, 2) as i32; 0 }"#);
    assert_ne!(exit, 0, "tuple as i32 must be rejected");
}

#[test]
fn stage18_426_neg_int_to_tuple() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42i32 as (i32, i32); 0 }"#);
    assert_ne!(exit, 0, "i32 as tuple must be rejected");
}

#[test]
fn stage18_426_neg_int_to_struct() {
    let exit = compile_only(r#"struct Foo { x: i32 } fn main() -> i32 { let x = 42 as Foo; 0 }"#);
    assert_ne!(exit, 0, "int as struct must be rejected");
}

#[test]
fn stage18_426_neg_struct_to_int() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let f = Foo { x: 1 }; let x = f as i32; 0 }"#,
    );
    assert_ne!(exit, 0, "struct as i32 must be rejected");
}

#[test]
fn stage18_426_neg_int_to_array() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 42 as [i32; 3]; 0 }"#);
    assert_ne!(exit, 0, "int as array must be rejected");
}

#[test]
fn stage18_426_neg_array_to_int() {
    let exit = compile_only(r#"fn main() -> i32 { let a = [1, 2, 3]; let x = a as i32; 0 }"#);
    assert_ne!(exit, 0, "array as i32 must be rejected");
}

#[test]
fn stage18_426_neg_str_to_float() {
    let exit = compile_only(r#"fn main() -> i32 { let x = "hi" as f64; 0 }"#);
    assert_ne!(exit, 0, "&str as f64 must be rejected");
}

// ============================================================================
// Negative tests: invalid numeric casts (Rust rejects these)
// ============================================================================

#[test]
fn stage18_426_neg_bool_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true as bool; 0 }"#);
    assert_ne!(exit, 0, "bool as bool must be rejected (Rust rejects)");
}

#[test]
fn stage18_426_neg_bool_to_float() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true as f64; 0 }"#);
    assert_ne!(exit, 0, "bool as f64 must be rejected (Rust rejects)");
}

#[test]
fn stage18_426_neg_float_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.5 as bool; 0 }"#);
    assert_ne!(exit, 0, "f64 as bool must be rejected (Rust rejects)");
}

#[test]
fn stage18_426_neg_bool_to_char() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true as char; 0 }"#);
    assert_ne!(exit, 0, "bool as char must be rejected (Rust rejects)");
}

#[test]
fn stage18_426_neg_float_to_char() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.5f64 as char; 0 }"#);
    assert_ne!(exit, 0, "f64 as char must be rejected (Rust rejects)");
}

// ============================================================================
// Negative tests: cast result assigned to wrong type
// ============================================================================

#[test]
fn stage18_426_neg_cast_result_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let s: &str = 42i32 as i64; 0 }"#);
    assert_ne!(
        exit, 0,
        "cast result (i64) assigned to &str must be rejected"
    );
}

#[test]
fn stage18_426_neg_cast_result_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let b: bool = 42i32 as i64; 0 }"#);
    assert_ne!(
        exit, 0,
        "cast result (i64) assigned to bool must be rejected"
    );
}
