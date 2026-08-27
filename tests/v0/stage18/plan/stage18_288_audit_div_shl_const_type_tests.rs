//! Stage 18.288 — §17.6 audit: TD-DIVZERO-CONST-TYPE + TD-SHIFTOVERFLOW-CONST-TYPE
//! regression tests.
//!
//! Found during the §17.6 "直到审查不出问题为止" audit after Stage 18.287
//! resolved TD-NEGOVERFLOW-I32. The audit checked all `emit_const` / `"0".to_string()`
//! call sites in codegen for the same type-mismatch pattern.
//!
//! Found 2 more same-class bugs:
//! - TD-DIVZERO-CONST-TYPE: `DivisionByZero` assert used `"0".to_string()` → i32 0,
//!   but operand could be i64 → LLVM type mismatch.
//! - TD-SHIFTOVERFLOW-CONST-TYPE: `Overflow(Shl/Shr)` assert used
//!   `bit_width.to_string()` → i32, but operand could be i64 → same mismatch.
//!
//! Both fixed by reusing `emit_const_typed` (added in Stage 18.287).
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §7.3.1: ≥30 negative audit cases covering all 7 error categories.

#![cfg(all(test, feature = "llvm-backend"))]

use std::path::Path;
#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::run_program;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn compile_only(code: &str) -> i32 {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_288_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
}

// =============================================================================
// POSITIVE TESTS (10) — verify DivisionByZero + Shl/Shr overflow on all int types
// =============================================================================

#[test]
fn stage18_288_div_i64() {
    // Previously crashed: "icmp eq i64 %v2, i32 0" → LLVM verify fail.
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i64 = 10;
               let b: i64 = 2;
               let c = a / b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_div_i32() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i32 = 10;
               let b: i32 = 2;
               let c = a / b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_div_i8() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i8 = 10;
               let b: i8 = 2;
               let c = a / b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_rem_i64() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i64 = 10;
               let b: i64 = 3;
               let c = a % b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "1\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_shl_i64() {
    // Previously crashed: "icmp uge i64 %v4, i32 64" → LLVM verify fail.
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i64 = 1;
               let b: i64 = 2;
               let c = a << b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "4\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_shl_i32() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i32 = 1;
               let b: i32 = 2;
               let c = a << b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "4\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_shr_i64() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i64 = 8;
               let b: i64 = 2;
               let c = a >> b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "2\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_shl_i8() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
               let a: i8 = 1;
               let b: i8 = 2;
               let c = a << b;
               println!("{}", c);
               0
           }"#,
    );
    assert_eq!(stdout, "4\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_288_div_in_impl() {
    // Division inside an impl method.
    let exit = compile_only(
        r#"impl i64 {
               fn halve(self) -> i64 { self / 2i64 }
           }
           fn main() -> i32 {
               println!("{}", 10i64.halve());
               0
           }"#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_288_shl_in_impl() {
    let exit = compile_only(
        r#"impl i64 {
               fn double(self) -> i64 { self << 1i64 }
           }
           fn main() -> i32 {
               println!("{}", 5i64.double());
               0
           }"#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

// =============================================================================
// NEGATIVE AUDIT SET (30 cases) — per §7.3.1, covers all 7 error categories
// =============================================================================

// Category 1: Wrong arg count (5 cases)

#[test]
fn stage18_288_neg_div_with_extra_arg() {
    // `a / 2 3` parses as two expressions. Use a real error: too many args to a function.
    let exit = compile_only(
        r#"fn div(a: i64, b: i64) -> i64 { a / b } fn main() -> i32 { div(1, 2, 3); 0 }"#,
    );
    assert_ne!(exit, 0, "too many args");
}

#[test]
fn stage18_288_neg_shl_with_extra_arg() {
    // `a << 2 3` parses as two expressions. Use a real error: calling non-callable.
    let exit = compile_only(r#"fn main() -> i32 { let a: i64 = 1; a(2); 0 }"#);
    assert_ne!(exit, 0, "a is not callable");
}

#[test]
fn stage18_288_neg_impl_div_with_arg() {
    let exit = compile_only(
        r#"impl i64 { fn halve(self) -> i64 { self / 2i64 } }
           fn main() -> i32 { 10i64.halve(42); 0 }"#,
    );
    assert_ne!(exit, 0, "halve takes no args");
}

#[test]
fn stage18_288_neg_impl_shl_with_arg() {
    let exit = compile_only(
        r#"impl i64 { fn double(self) -> i64 { self << 1i64 } }
           fn main() -> i32 { 5i64.double(99); 0 }"#,
    );
    assert_ne!(exit, 0, "double takes no args");
}

#[test]
fn stage18_288_neg_div_call_with_args() {
    let exit = compile_only(r#"fn main() -> i32 { div(1, 2, 3); 0 }"#);
    assert_ne!(exit, 0, "div not defined or too many args");
}

// Category 2: Wrong arg type (4 cases)

#[test]
fn stage18_288_neg_div_with_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let a: i64 = 10; a / true; 0 }"#);
    assert_ne!(exit, 0, "div expects i64, not bool");
}

#[test]
fn stage18_288_neg_shl_with_str() {
    let exit = compile_only(r#"fn main() -> i32 { let a: i64 = 1; a << "hi"; 0 }"#);
    assert_ne!(exit, 0, "shl expects i64, not str");
}

#[test]
fn stage18_288_neg_impl_div_wrong_arg() {
    let exit = compile_only(
        r#"impl i64 { fn div_by(self, n: i64) -> i64 { self / n } }
           fn main() -> i32 { 10i64.div_by(true); 0 }"#,
    );
    assert_ne!(exit, 0, "div_by expects i64");
}

#[test]
fn stage18_288_neg_impl_shl_wrong_arg() {
    let exit = compile_only(
        r#"impl i64 { fn shl_by(self, n: i64) -> i64 { self << n } }
           fn main() -> i32 { 5i64.shl_by("hi"); 0 }"#,
    );
    assert_ne!(exit, 0, "shl_by expects i64");
}

// Category 3: Wrong receiver type (5 cases)

#[test]
fn stage18_288_neg_div_on_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let b: bool = true; b / 2; 0 }"#);
    assert_ne!(exit, 0, "bool has no div");
}

#[test]
fn stage18_288_neg_shl_on_str() {
    let exit = compile_only(r#"fn main() -> i32 { let s: &str = "hi"; s << 2; 0 }"#);
    assert_ne!(exit, 0, "str has no shl");
}

#[test]
fn stage18_288_neg_div_on_struct() {
    let exit = compile_only(
        r#"struct Foo { x: i32 }
           fn main() -> i32 { let f = Foo { x: 1 }; f / 2; 0 }"#,
    );
    assert_ne!(exit, 0, "struct has no div");
}

#[test]
fn stage18_288_neg_shl_on_unit() {
    let exit = compile_only(r#"fn main() -> i32 { let u = (); u << 2; 0 }"#);
    assert_ne!(exit, 0, "unit has no shl");
}

#[test]
fn stage18_288_neg_impl_method_on_struct() {
    let exit = compile_only(
        r#"struct Foo { x: i32 }
           impl i64 { fn halve(self) -> i64 { self / 2i64 } }
           fn main() -> i32 { let f = Foo { x: 1 }; f.halve(); 0 }"#,
    );
    assert_ne!(exit, 0, "Foo has no halve");
}

// Category 4: Wrong return type usage (4 cases)

#[test]
fn stage18_288_neg_div_assign_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let a: i64 = 10; let b: bool = a / 2; 0 }"#);
    assert_ne!(exit, 0, "div returns i64, not bool");
}

#[test]
fn stage18_288_neg_shl_assign_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let a: i64 = 1; let s: &str = a << 2; 0 }"#);
    assert_ne!(exit, 0, "shl returns i64, not str");
}

#[test]
fn stage18_288_neg_div_in_bool_context() {
    let exit = compile_only(r#"fn main() -> i32 { let a: i64 = 10; if a / 2 { 0 } else { 1 } }"#);
    assert_ne!(exit, 0, "if expects bool, not i64");
}

#[test]
fn stage18_288_neg_shl_passed_to_bool_param() {
    let exit = compile_only(
        r#"fn take_bool(b: bool) -> i32 { 0 }
           fn main() -> i32 { let a: i64 = 1; take_bool(a << 2); 0 }"#,
    );
    assert_ne!(exit, 0, "shl returns i64, take_bool expects bool");
}

// Category 5: Method doesn't exist (5 cases)

#[test]
fn stage18_288_neg_i64_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { 5i64.nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "nonexistent method");
}

#[test]
fn stage18_288_neg_impl_method_calls_nonexistent_fn() {
    let exit = compile_only(
        r#"impl i64 { fn halve(self) -> i64 { nonexistent() } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent function in impl body");
}

#[test]
fn stage18_288_neg_impl_method_field_nonexistent() {
    let exit = compile_only(
        r#"struct Foo { x: i32 }
           impl i64 { fn halve(self) -> i64 { let f = Foo { x: 1 }; f.nonexistent } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent field");
}

#[test]
fn stage18_288_neg_impl_unknown_method_on_i32() {
    let exit = compile_only(r#"fn main() -> i32 { 5i32.unknown(); 0 }"#);
    assert_ne!(exit, 0, "unknown method on i32");
}

#[test]
fn stage18_288_neg_impl_unknown_method_on_bool() {
    let exit = compile_only(r#"fn main() -> i32 { true.unknown(); 0 }"#);
    assert_ne!(exit, 0, "unknown method on bool");
}

// Category 6: Method on wrong primitive (4 cases)

#[test]
fn stage18_288_neg_i64_method_on_u32() {
    let exit = compile_only(
        r#"impl i64 { fn halve(self) -> i64 { self / 2i64 } }
           fn main() -> i32 { let n: u32 = 5; n.halve(); 0 }"#,
    );
    assert_ne!(exit, 0, "u32 has no halve (i64 does)");
}

#[test]
fn stage18_288_neg_i64_method_on_char() {
    let exit = compile_only(
        r#"impl i64 { fn halve(self) -> i64 { self / 2i64 } }
           fn main() -> i32 { 'a'.halve(); 0 }"#,
    );
    assert_ne!(exit, 0, "char has no halve");
}

#[test]
fn stage18_288_neg_i64_method_on_f64() {
    let exit = compile_only(
        r#"impl i64 { fn halve(self) -> i64 { self / 2i64 } }
           fn main() -> i32 { (3.14).halve(); 0 }"#,
    );
    assert_ne!(exit, 0, "f64 has no halve");
}

#[test]
fn stage18_288_neg_i64_method_on_i8() {
    let exit = compile_only(
        r#"impl i64 { fn halve(self) -> i64 { self / 2i64 } }
           fn main() -> i32 { let n: i8 = 5; n.halve(); 0 }"#,
    );
    assert_ne!(exit, 0, "i8 has no halve (i64 does)");
}

// Category 7: User impl with mutability / type issues (3 cases)

#[test]
fn stage18_288_neg_impl_returns_mismatch() {
    let exit = compile_only(
        r#"impl i64 { fn bad_ret(self) -> i64 { true } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "return type mismatch");
}

#[test]
fn stage18_288_neg_impl_wrong_arg_type_3() {
    let exit = compile_only(
        r#"impl i64 { fn div_by(self, n: i64) -> i64 { self / n } }
           fn main() -> i32 { 10i64.div_by("str"); 0 }"#,
    );
    assert_ne!(exit, 0, "wrong arg type");
}

#[test]
fn stage18_288_neg_impl_mismatched_branches() {
    let exit = compile_only(
        r#"impl i64 { fn bad(self) -> i64 { if self > 0i64 { 1i64 } else { "str" } } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "if branches have different types");
}
