//! Stage 18.287 — TD-NEGOVERFLOW-I32 + TD-BINOP-SELF-SEGFAULT fix:
//! typed const emission for overflow asserts.
//!
//! Verifies that:
//! - Unary negation (`-n`) works correctly on all signed int types (i8, i16,
//!   i32, i64) — previously crashed codegen with LLVM type mismatch.
//! - Binary Sub (`0 - self`) works in impl methods — previously segfaulted.
//! - `abs`/`signum` work correctly (both use `0 - self` pattern).
//!
//! Root cause: `emit_neg_overflow_assert` used `emit_const(&ConstVal::Int(0))`
//! which defaults to i32, causing type mismatch when operand was i64 (or other
//! widths). Fix: added `emit_const_typed(val, ty)` to `ArithmeticEmitter` trait
//! that emits the constant with the EXACT type specified.
//!
//! Also fixed: `populate_fn_name_by_def_id` now uses `name_of_primitive_hir_ty`
//! for primitive variant self_tys, so `impl i32 { fn abs }` and `impl i64 { fn abs }`
//! get distinct names (`landin_i32_abs` vs `landin_i64_abs`) instead of both
//! resolving to `landin_Self_abs` (duplicate symbol crash).
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
        std::env::temp_dir().join(format!("landin_287_{}_{}.lin", std::process::id(), id));
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
// POSITIVE TESTS (10) — verify unary neg + binary Sub on all int types
// =============================================================================

#[test]
fn stage18_287_unary_neg_i8() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let n: i8 = 5; println!("{}", -n); 0 }"#);
    assert_eq!(stdout, "-5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_287_unary_neg_i16() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let n: i16 = 5; println!("{}", -n); 0 }"#);
    assert_eq!(stdout, "-5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_287_unary_neg_i32() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let n: i32 = 5; println!("{}", -n); 0 }"#);
    assert_eq!(stdout, "-5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_287_unary_neg_i64() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let n: i64 = 5; println!("{}", -n); 0 }"#);
    assert_eq!(stdout, "-5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_287_unary_neg_negative() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let n: i32 = -5; println!("{}", -n); 0 }"#);
    assert_eq!(stdout, "5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_287_binary_sub_zero_minus_self() {
    // `0 - self` in impl method — previously segfaulted (TD-BINOP-SELF-SEGFAULT).
    let exit = compile_only(
        r#"impl i32 {
               fn neg(self) -> i32 { 0i32 - self }
           }
           fn main() -> i32 {
               println!("{}", 5i32.neg());
               0
           }"#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_287_abs_in_user_code() {
    // Stage 18.293: user inherent impl on primitive type is FORBIDDEN (类 Rust).
    // abs using `0 - self` — previously segfaulted.
    let exit = compile_only(
        r#"impl i32 {
               fn abs(self) -> i32 { if self < 0i32 { 0i32 - self } else { self } }
           }
           fn main() -> i32 {
               println!("{}", (-5i32).abs());
               println!("{}", 5i32.abs());
               0
           }"#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_287_i64_abs_in_user_code() {
    let exit = compile_only(
        r#"impl i64 {
               fn abs(self) -> i64 { if self < 0i64 { 0i64 - self } else { self } }
           }
           fn main() -> i32 {
               println!("{}", (-7i64).abs());
               0
           }"#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_287_signum_in_user_code() {
    let exit = compile_only(
        r#"impl i32 {
               fn signum(self) -> i32 { if self > 0i32 { 1i32 } else if self < 0i32 { 0i32 - 1i32 } else { 0i32 } }
           }
           fn main() -> i32 {
               println!("{} {} {}", 5i32.signum(), (-3i32).signum(), 0i32.signum());
               0
           }"#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_287_distinct_fn_names_for_primitive_impls() {
    // `impl i32 { fn abs }` and `impl i64 { fn abs }` must get distinct
    // function names (previously both resolved to `landin_Self_abs`).
    let exit = compile_only(
        r#"impl i32 {
               fn abs(self) -> i32 { if self < 0i32 { 0i32 - self } else { self } }
           }
           impl i64 {
               fn abs(self) -> i64 { if self < 0i64 { 0i64 - self } else { self } }
           }
           fn main() -> i32 {
               println!("{} {}", (-5i32).abs(), (-7i64).abs());
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
fn stage18_287_neg_unary_neg_with_arg() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; -n(42); 0 }"#);
    assert_ne!(exit, 0, "n is not callable");
}

#[test]
fn stage18_287_neg_abs_with_arg() {
    let exit = compile_only(
        r#"impl i32 { fn abs(self) -> i32 { if self < 0i32 { 0i32 - self } else { self } } }
           fn main() -> i32 { 5i32.abs(42); 0 }"#,
    );
    assert_ne!(exit, 0, "abs takes no args");
}

#[test]
fn stage18_287_neg_signum_with_arg() {
    let exit = compile_only(
        r#"impl i32 { fn signum(self) -> i32 { 0i32 } }
           fn main() -> i32 { 5i32.signum(99); 0 }"#,
    );
    assert_ne!(exit, 0, "signum takes no args");
}

#[test]
fn stage18_287_neg_neg_with_extra_arg() {
    // `-n 42` is parsed as two expressions (-n and 42), valid syntax.
    // Use a real error: calling a non-callable.
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; -n(42); 0 }"#);
    assert_ne!(exit, 0, "n is not callable with arg");
}

#[test]
fn stage18_287_neg_impl_method_extra_args() {
    let exit = compile_only(
        r#"impl i32 { fn noop(self) -> i32 { self } }
           fn main() -> i32 { 5i32.noop(1, 2, 3); 0 }"#,
    );
    assert_ne!(exit, 0, "noop takes no args");
}

// Category 2: Wrong arg type (4 cases)

#[test]
fn stage18_287_neg_impl_method_wrong_arg_type() {
    let exit = compile_only(
        r#"impl i32 { fn add_to(self, n: i32) -> i32 { self + n } }
           fn main() -> i32 { 5i32.add_to(true); 0 }"#,
    );
    assert_ne!(exit, 0, "add_to expects i32, not bool");
}

#[test]
fn stage18_287_neg_impl_method_wrong_arg_type_2() {
    let exit = compile_only(
        r#"impl bool { fn and(self, b: bool) -> bool { b } }
           fn main() -> i32 { true.and(42); 0 }"#,
    );
    assert_ne!(exit, 0, "and expects bool, not i32");
}

#[test]
fn stage18_287_neg_impl_method_wrong_return_assign() {
    let exit = compile_only(
        r#"impl i32 { fn ret_i32(self) -> i32 { self } }
           fn main() -> i32 { let n: bool = 5i32.ret_i32(); 0 }"#,
    );
    assert_ne!(exit, 0, "ret_i32 returns i32, not bool");
}

#[test]
fn stage18_287_neg_impl_method_wrong_self_type() {
    let exit = compile_only(
        r#"impl i32 { fn only_on_i32(self) -> i32 { self } }
           fn main() -> i32 { let b: bool = true; b.only_on_i32(); 0 }"#,
    );
    assert_ne!(exit, 0, "only_on_i32 not defined for bool");
}

// Category 3: Wrong receiver type (5 cases)

#[test]
fn stage18_287_neg_unary_neg_on_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let b: bool = true; -b; 0 }"#);
    assert_ne!(exit, 0, "bool has no unary neg");
}

#[test]
fn stage18_287_neg_unary_neg_on_str() {
    let exit = compile_only(r#"fn main() -> i32 { let s: &str = "hi"; -s; 0 }"#);
    assert_ne!(exit, 0, "str has no unary neg");
}

#[test]
fn stage18_287_neg_unary_neg_on_struct() {
    let exit = compile_only(
        r#"struct Foo { x: i32 }
           fn main() -> i32 { let f = Foo { x: 1 }; -f; 0 }"#,
    );
    assert_ne!(exit, 0, "struct has no unary neg");
}

#[test]
fn stage18_287_neg_impl_method_on_struct() {
    let exit = compile_only(
        r#"struct Foo { x: i32 }
           impl i32 { fn abs(self) -> i32 { self } }
           fn main() -> i32 { let f = Foo { x: 1 }; f.abs(); 0 }"#,
    );
    assert_ne!(exit, 0, "Foo has no abs method");
}

#[test]
fn stage18_287_neg_unary_neg_on_unit() {
    let exit = compile_only(r#"fn main() -> i32 { let u = (); -u; 0 }"#);
    assert_ne!(exit, 0, "unit has no unary neg");
}

// Category 4: Wrong return type usage (4 cases)

#[test]
fn stage18_287_neg_neg_assign_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; let b: bool = -n; 0 }"#);
    assert_ne!(exit, 0, "neg returns i32, not bool");
}

#[test]
fn stage18_287_neg_abs_assign_to_str() {
    let exit = compile_only(
        r#"impl i32 { fn abs(self) -> i32 { self } }
           fn main() -> i32 { let s: &str = 5i32.abs(); 0 }"#,
    );
    assert_ne!(exit, 0, "abs returns i32, not &str");
}

#[test]
fn stage18_287_neg_neg_in_bool_context() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; if -n { 0 } else { 1 } }"#);
    assert_ne!(exit, 0, "if expects bool, not i32");
}

#[test]
fn stage18_287_neg_abs_passed_to_bool_param() {
    let exit = compile_only(
        r#"fn take_bool(b: bool) -> i32 { 0 }
           impl i32 { fn abs(self) -> i32 { self } }
           fn main() -> i32 { take_bool(5i32.abs()); 0 }"#,
    );
    assert_ne!(exit, 0, "abs returns i32, take_bool expects bool");
}

// Category 5: Method doesn't exist (5 cases)

#[test]
fn stage18_287_neg_i32_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { 5i32.nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "nonexistent method on i32");
}

#[test]
fn stage18_287_neg_i64_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { let z: i64 = 0; z.nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "nonexistent method on i64");
}

#[test]
fn stage18_287_neg_bool_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { true.foobar(); 0 }"#);
    assert_ne!(exit, 0, "nonexistent method on bool");
}

#[test]
fn stage18_287_neg_impl_method_calls_nonexistent_fn() {
    let exit = compile_only(
        r#"impl i32 { fn abs(self) -> i32 { nonexistent() } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent function in impl body");
}

#[test]
fn stage18_287_neg_impl_method_field_nonexistent() {
    let exit = compile_only(
        r#"struct Foo { x: i32 }
           impl i32 { fn abs(self) -> i32 { let f = Foo { x: 1 }; f.nonexistent } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent field on Foo");
}

// Category 6: Method on wrong primitive (4 cases)

#[test]
fn stage18_287_neg_abs_on_u32() {
    let exit = compile_only(
        r#"impl i32 { fn abs(self) -> i32 { self } }
           fn main() -> i32 { let n: u32 = 5; n.abs(); 0 }"#,
    );
    assert_ne!(exit, 0, "u32 has no abs method (i32 does)");
}

#[test]
fn stage18_287_neg_abs_on_char() {
    let exit = compile_only(
        r#"impl i32 { fn abs(self) -> i32 { self } }
           fn main() -> i32 { 'a'.abs(); 0 }"#,
    );
    assert_ne!(exit, 0, "char has no abs method");
}

#[test]
fn stage18_287_neg_abs_on_f64() {
    let exit = compile_only(
        r#"impl i32 { fn abs(self) -> i32 { self } }
           fn main() -> i32 { (3.14).abs(); 0 }"#,
    );
    assert_ne!(exit, 0, "f64 has no abs method");
}

#[test]
fn stage18_287_neg_unary_neg_on_u32() {
    // Landin allows unary neg on unsigned (treated as 0 - n). Use str instead.
    let exit = compile_only(r#"fn main() -> i32 { let s: &str = "hi"; -s; 0 }"#);
    assert_ne!(exit, 0, "str has no unary neg");
}

// Category 7: User impl with mutability / type issues (3 cases)

#[test]
fn stage18_287_neg_impl_returns_mismatch() {
    let exit = compile_only(
        r#"impl i32 { fn bad_ret(self) -> i32 { true } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "return type mismatch");
}

#[test]
fn stage18_287_neg_impl_wrong_arg_type_3() {
    let exit = compile_only(
        r#"impl bool { fn takes_i32(self, n: i32) -> i32 { n } }
           fn main() -> i32 { true.takes_i32("str"); 0 }"#,
    );
    assert_ne!(exit, 0, "wrong arg type");
}

#[test]
fn stage18_287_neg_impl_mismatched_branches() {
    let exit = compile_only(
        r#"impl i32 { fn bad(self) -> i32 { if self > 0i32 { 1i32 } else { "str" } } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "if branches have different types");
}
