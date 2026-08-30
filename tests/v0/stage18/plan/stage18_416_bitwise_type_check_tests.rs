//! Stage 18.416 — §20 iterative audit: BitAnd/BitOr/BitXor type check.
//!
//! Found during the §20 "finding one bug means there are many similar bugs"
//! audit after Stage 18.412 fixed the Shl/Shr lhs type check. The audit
//! checked all BinaryOp arms in typeck's `infer_rvalue` for similar
//! missing type checks.
//!
//! Found: BitAnd/BitOr/BitXor arm only called `unify(a, b)` without checking
//! that `a_ty` is Bool or Int/Uint. For `"hello" & "world"`, unify(&str,
//! &str) succeeds → no error → silent acceptance. Codegen's `_ => "add i32"`
//! fallback then emitted wrong LLVM IR for the non-integer operands.
//!
//! Fix (Stage 18.416): Added `is_notable_ty(&a_ty)` check before unify.
//! Non-Bool/non-Int types now report "bitwise op requires Bool or integer
//! type, found <type>".
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §7.3.1: ≥30 negative audit cases covering all 7 error categories.
//! Per §20: iterative audit — same class as Stage 18.412 (Shl/Shr).
//! Per §1.0 原則 4 (报错 > 静默): typeck must reject, not codegen fallback.
//! Per §1.0 原則 6 (通解 > 特解): one is_notable_ty check covers all
//! non-Bool/non-Int types (Float, Str, Array, Tuple, Adt, Unit).

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn compile_only(code: &str) -> i32 {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_416_{}_{}.lin", std::process::id(), id));
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
// Positive tests: valid BitAnd/BitOr/BitXor on Bool and Int types
// ============================================================================

#[test]
fn stage18_416_pos_int_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 & 2i32; 0 }"#);
    assert_eq!(exit, 0, "int bitwise AND must compile");
}

#[test]
fn stage18_416_pos_int_bitor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 | 2i32; 0 }"#);
    assert_eq!(exit, 0, "int bitwise OR must compile");
}

#[test]
fn stage18_416_pos_int_bitxor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 ^ 2i32; 0 }"#);
    assert_eq!(exit, 0, "int bitwise XOR must compile");
}

#[test]
fn stage18_416_pos_bool_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true & false; 0 }"#);
    assert_eq!(exit, 0, "bool bitwise AND must compile");
}

#[test]
fn stage18_416_pos_bool_bitor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true | false; 0 }"#);
    assert_eq!(exit, 0, "bool bitwise OR must compile");
}

#[test]
fn stage18_416_pos_bool_bitxor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true ^ false; 0 }"#);
    assert_eq!(exit, 0, "bool bitwise XOR must compile");
}

#[test]
fn stage18_416_pos_i64_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i64 & 2i64; 0 }"#);
    assert_eq!(exit, 0, "i64 bitwise AND must compile");
}

#[test]
fn stage18_416_pos_u32_bitor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1u32 | 2u32; 0 }"#);
    assert_eq!(exit, 0, "u32 bitwise OR must compile");
}

// ============================================================================
// Negative tests: BitAnd/BitOr/BitXor on Float (9 cases)
// ============================================================================

#[test]
fn stage18_416_neg_float_bitand_f64() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: f64 = 1.0; let b: f64 = 2.0; let x = a & b; 0 }"#,
    );
    assert_ne!(exit, 0, "f64 bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_float_bitor_f64() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: f64 = 1.0; let b: f64 = 2.0; let x = a | b; 0 }"#,
    );
    assert_ne!(exit, 0, "f64 bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_float_bitxor_f64() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: f64 = 1.0; let b: f64 = 2.0; let x = a ^ b; 0 }"#,
    );
    assert_ne!(exit, 0, "f64 bitwise XOR must be rejected");
}

#[test]
fn stage18_416_neg_float_bitand_f32() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: f32 = 1.0; let b: f32 = 2.0; let x = a & b; 0 }"#,
    );
    assert_ne!(exit, 0, "f32 bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_float_bitor_f32() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: f32 = 1.0; let b: f32 = 2.0; let x = a | b; 0 }"#,
    );
    assert_ne!(exit, 0, "f32 bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_float_bitxor_f32() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: f32 = 1.0; let b: f32 = 2.0; let x = a ^ b; 0 }"#,
    );
    assert_ne!(exit, 0, "f32 bitwise XOR must be rejected");
}

#[test]
fn stage18_416_neg_float_bitand_literal() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.0 & 2.0; 0 }"#);
    assert_ne!(exit, 0, "float literal bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_float_bitor_literal() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.0 | 2.0; 0 }"#);
    assert_ne!(exit, 0, "float literal bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_float_bitxor_literal() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1.0 ^ 2.0; 0 }"#);
    assert_ne!(exit, 0, "float literal bitwise XOR must be rejected");
}

// ============================================================================
// Negative tests: BitAnd/BitOr/BitXor on &str (6 cases)
// ============================================================================

#[test]
fn stage18_416_neg_str_bitand() {
    let exit = compile_only(
        r#"fn main() -> i32 { let s: &str = "hi"; let t: &str = "ok"; let x = s & t; 0 }"#,
    );
    assert_ne!(exit, 0, "&str bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_str_bitor() {
    let exit = compile_only(
        r#"fn main() -> i32 { let s: &str = "hi"; let t: &str = "ok"; let x = s | t; 0 }"#,
    );
    assert_ne!(exit, 0, "&str bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_str_bitxor() {
    let exit = compile_only(
        r#"fn main() -> i32 { let s: &str = "hi"; let t: &str = "ok"; let x = s ^ t; 0 }"#,
    );
    assert_ne!(exit, 0, "&str bitwise XOR must be rejected");
}

#[test]
fn stage18_416_neg_str_bitand_literal() {
    let exit = compile_only(r#"fn main() -> i32 { let x = "a" & "b"; 0 }"#);
    assert_ne!(exit, 0, "str literal bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_str_bitor_literal() {
    let exit = compile_only(r#"fn main() -> i32 { let x = "a" | "b"; 0 }"#);
    assert_ne!(exit, 0, "str literal bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_str_bitxor_literal() {
    let exit = compile_only(r#"fn main() -> i32 { let x = "a" ^ "b"; 0 }"#);
    assert_ne!(exit, 0, "str literal bitwise XOR must be rejected");
}

// ============================================================================
// Negative tests: BitAnd/BitOr/BitXor on Unit (3 cases)
// ============================================================================

#[test]
fn stage18_416_neg_unit_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let u = (); let v = (); let x = u & v; 0 }"#);
    assert_ne!(exit, 0, "unit bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_unit_bitor() {
    let exit = compile_only(r#"fn main() -> i32 { let u = (); let v = (); let x = u | v; 0 }"#);
    assert_ne!(exit, 0, "unit bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_unit_bitxor() {
    let exit = compile_only(r#"fn main() -> i32 { let u = (); let v = (); let x = u ^ v; 0 }"#);
    assert_ne!(exit, 0, "unit bitwise XOR must be rejected");
}

// ============================================================================
// Negative tests: BitAnd/BitOr/BitXor on Struct (6 cases)
// ============================================================================

#[test]
fn stage18_416_neg_struct_bitand() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let a = Foo { x: 1 }; let b = Foo { x: 2 }; let x = a & b; 0 }"#,
    );
    assert_ne!(exit, 0, "struct bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_struct_bitor() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let a = Foo { x: 1 }; let b = Foo { x: 2 }; let x = a | b; 0 }"#,
    );
    assert_ne!(exit, 0, "struct bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_struct_bitxor() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } fn main() -> i32 { let a = Foo { x: 1 }; let b = Foo { x: 2 }; let x = a ^ b; 0 }"#,
    );
    assert_ne!(exit, 0, "struct bitwise XOR must be rejected");
}

#[test]
fn stage18_416_neg_tuple_bitand() {
    let exit =
        compile_only(r#"fn main() -> i32 { let a = (1, 2); let b = (3, 4); let x = a & b; 0 }"#);
    assert_ne!(exit, 0, "tuple bitwise AND must be rejected");
}

#[test]
fn stage18_416_neg_tuple_bitor() {
    let exit =
        compile_only(r#"fn main() -> i32 { let a = (1, 2); let b = (3, 4); let x = a | b; 0 }"#);
    assert_ne!(exit, 0, "tuple bitwise OR must be rejected");
}

#[test]
fn stage18_416_neg_tuple_bitxor() {
    let exit =
        compile_only(r#"fn main() -> i32 { let a = (1, 2); let b = (3, 4); let x = a ^ b; 0 }"#);
    assert_ne!(exit, 0, "tuple bitwise XOR must be rejected");
}

// ============================================================================
// Negative tests: Type mismatch in BitAnd/BitOr/BitXor (6 cases)
// ============================================================================

#[test]
fn stage18_416_neg_int_bool_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 & true; 0 }"#);
    assert_ne!(exit, 0, "i32 & bool type mismatch must be rejected");
}

#[test]
fn stage18_416_neg_bool_int_bitor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = true | 1i32; 0 }"#);
    assert_ne!(exit, 0, "bool | i32 type mismatch must be rejected");
}

#[test]
fn stage18_416_neg_i32_i64_bitxor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 ^ 2i64; 0 }"#);
    assert_ne!(exit, 0, "i32 ^ i64 type mismatch must be rejected");
}

#[test]
fn stage18_416_neg_int_str_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 & "hi"; 0 }"#);
    assert_ne!(exit, 0, "i32 & str type mismatch must be rejected");
}

#[test]
fn stage18_416_neg_str_int_bitor() {
    let exit = compile_only(r#"fn main() -> i32 { let x = "hi" | 1i32; 0 }"#);
    assert_ne!(exit, 0, "str | i32 type mismatch must be rejected");
}

#[test]
fn stage18_416_neg_int_float_bitand() {
    let exit = compile_only(r#"fn main() -> i32 { let x = 1i32 & 2.0; 0 }"#);
    assert_ne!(exit, 0, "i32 & f64 type mismatch must be rejected");
}

// ============================================================================
// Negative tests: BitAnd/BitOr/BitXor result assigned to wrong type (3 cases)
// ============================================================================

#[test]
fn stage18_416_neg_bitand_result_to_str() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: i32 = 1; let b: i32 = 2; let s: &str = a & b; 0 }"#,
    );
    assert_ne!(exit, 0, "int & result assigned to &str must be rejected");
}

#[test]
fn stage18_416_neg_bitor_result_to_float() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: i32 = 1; let b: i32 = 2; let f: f64 = a | b; 0 }"#,
    );
    assert_ne!(exit, 0, "int | result assigned to f64 must be rejected");
}

#[test]
fn stage18_416_neg_bitxor_result_to_bool() {
    let exit = compile_only(
        r#"fn main() -> i32 { let a: i32 = 1; let b: i32 = 2; let x: bool = a ^ b; 0 }"#,
    );
    assert_ne!(exit, 0, "int ^ result assigned to bool must be rejected");
}
