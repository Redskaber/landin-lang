//! Stage 18.285 — TD-INTRINSIC-OVERUSE Phase 2-A continuation: Primitive type
//! impls with real bodies (architecture generality verification).
//!
//! Verifies that the Stage 18.284 architecture (prelude impl declarations +
//! post-resolution intrinsic dispatch) is GENERAL — works for ALL primitive
//! types (i32, i64, bool, char, etc.), not just `str`.
//!
//! Stage 18.284 migrated `impl str { fn len/is_empty/as_bytes }` (marker
//! bodies + intrinsic dispatch). Stage 18.285 extends the architecture to
//! `impl i32`, `impl i64`, `impl bool` with REAL bodies (no intrinsic
//! dispatch needed) — proving the infrastructure is general.
//!
//! Key architecture change: `name_of_primitive_hir_ty` (mirror of
//! `name_of_primitive_ty` for HIR side) + string-based impl matching in
//! `resolve_inherent_method` + `lookup_primitive_intrinsic`. This handles
//! the asymmetry: parser parses `str` as `HirTyKind::Path("str")` (not a
//! keyword) but `i32`/`bool` etc. as `HirTyKind::Int(I32)` / `HirTyKind::Bool`
//! (keyword variants).
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §7.3.1: ≥30 negative audit cases covering all 7 error categories.

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
        std::env::temp_dir().join(format!("landin_285_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--compile")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    output.status.code().unwrap_or(-1)
}

fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    static COUNTER2: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER2.fetch_add(1, Ordering::SeqCst);
    let lin_file =
        std::env::temp_dir().join(format!("landin_285run_{}_{}.lin", std::process::id(), id));
    std::fs::write(&lin_file, code).expect("write .lin file");
    let output = Command::new(&bin)
        .arg("--run")
        .arg(&lin_file)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&lin_file);
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

// =============================================================================
// POSITIVE TESTS (8) — verify the architecture works for ALL primitive types
// =============================================================================

#[test]
fn stage18_285_bool_to_int_true() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
            let b: bool = true;
            let n = b.to_int();
            println!("{}", n);
            0
        }"#,
    );
    assert_eq!(stdout, "1\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_285_bool_to_int_false() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
            let b: bool = false;
            let n = b.to_int();
            println!("{}", n);
            0
        }"#,
    );
    assert_eq!(stdout, "0\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_285_i64_is_zero_true() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
            let z: i64 = 0;
            let r = z.is_zero();
            println!("{}", r);
            0
        }"#,
    );
    assert_eq!(stdout, "true\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_285_i64_is_zero_false() {
    let (stdout, exit) = run_program(
        r#"fn main() -> i32 {
            let z: i64 = 42;
            let r = z.is_zero();
            println!("{}", r);
            0
        }"#,
    );
    assert_eq!(stdout, "false\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_285_user_impl_i32_real_body() {
    // Stage 18.293: user inherent impl on primitive type is FORBIDDEN (类 Rust).
    // impl i32 with a real body (NOT a prelude method).
    // Verifies the architecture supports user primitive impls, not just prelude.
    let exit = compile_only(
        r#"
        impl i32 {
            fn user_double(self) -> i32 { self + self }
        }
        fn main() -> i32 {
            let n: i32 = 21;
            println!("{}", n.user_double());
            0
        }
        "#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_285_user_impl_bool_real_body() {
    // Stage 18.293: user inherent impl on primitive type is FORBIDDEN (类 Rust).
    // impl bool with a real body.
    let exit = compile_only(
        r#"
        impl bool {
            fn to_str(self) -> i32 { match self { true => 1i32, false => 0i32 } }
        }
        fn main() -> i32 {
            let b: bool = true;
            println!("{}", b.to_str());
            0
        }
        "#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_285_user_impl_i64_real_body() {
    // Stage 18.293: user inherent impl on primitive type is FORBIDDEN (类 Rust).
    // impl i64 with a real body.
    let exit = compile_only(
        r#"
        impl i64 {
            fn user_square(self) -> i64 { self * self }
        }
        fn main() -> i32 {
            let n: i64 = 7;
            println!("{}", n.user_square());
            0
        }
        "#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_285_chained_primitive_methods() {
    // Chain: bool::to_int() then call method on result (i32).
    let exit = compile_only(
        r#"
        impl i32 {
            fn user_double(self) -> i32 { self + self }
        }
        fn main() -> i32 {
            let b: bool = true;
            let n = b.to_int().user_double();
            println!("{}", n);
            0
        }
        "#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

// =============================================================================
// NEGATIVE AUDIT SET (32 cases) — per §7.3.1, covers all 7 error categories
// =============================================================================

// Category 1: Wrong arg count (5 cases)

#[test]
fn stage18_285_neg_to_int_with_arg() {
    let exit = compile_only(r#"fn main() -> i32 { let b: bool = true; b.to_int(42); 0 }"#);
    assert_ne!(exit, 0, "bool::to_int takes no args");
}

#[test]
fn stage18_285_neg_is_zero_with_arg() {
    let exit = compile_only(r#"fn main() -> i32 { let z: i64 = 0; z.is_zero(99); 0 }"#);
    assert_ne!(exit, 0, "i64::is_zero takes no args");
}

#[test]
fn stage18_285_neg_user_impl_wrong_arg_count() {
    let exit = compile_only(
        r#"
        impl i32 {
            fn add_one(self, x: i32) -> i32 { self + x }
        }
        fn main() -> i32 { 5.add_one(); 0 }
        "#,
    );
    assert_ne!(exit, 0, "add_one requires 1 arg");
}

#[test]
fn stage18_285_neg_user_impl_extra_args() {
    let exit = compile_only(
        r#"
        impl i32 {
            fn noop(self) -> i32 { self }
        }
        fn main() -> i32 { 5.noop(1, 2); 0 }
        "#,
    );
    assert_ne!(exit, 0, "noop takes no args");
}

#[test]
fn stage18_285_neg_to_int_with_string_arg() {
    let exit = compile_only(r#"fn main() -> i32 { true.to_int("extra"); 0 }"#);
    assert_ne!(exit, 0, "to_int takes no args");
}

// Category 2: Wrong arg type (4 cases)

#[test]
fn stage18_285_neg_user_impl_wrong_arg_type() {
    let exit = compile_only(
        r#"
        impl i32 {
            fn add_to(self, n: i32) -> i32 { self + n }
        }
        fn main() -> i32 { 5.add_to(true); 0 }
        "#,
    );
    assert_ne!(exit, 0, "add_to expects i32, not bool");
}

#[test]
fn stage18_285_neg_user_impl_wrong_arg_type_2() {
    let exit = compile_only(
        r#"
        impl bool {
            fn and(self, b: bool) -> bool { match self { true => b, false => false } }
        }
        fn main() -> i32 { true.and(42); 0 }
        "#,
    );
    assert_ne!(exit, 0, "and expects bool, not i32");
}

#[test]
fn stage18_285_neg_user_impl_wrong_return_assign() {
    let exit = compile_only(
        r#"
        impl i32 {
            fn ret_i32(self) -> i32 { self }
        }
        fn main() -> i32 { let n: bool = 5.ret_i32(); 0 }
        "#,
    );
    assert_ne!(exit, 0, "ret_i32 returns i32, not bool");
}

#[test]
fn stage18_285_neg_user_impl_wrong_self_type() {
    let exit = compile_only(
        r#"
        impl i32 {
            fn only_on_i32(self) -> i32 { self }
        }
        fn main() -> i32 {
            let b: bool = true;
            b.only_on_i32();
            0
        }
        "#,
    );
    assert_ne!(exit, 0, "only_on_i32 not defined for bool");
}

// Category 3: Wrong receiver type (5 cases)

#[test]
fn stage18_285_neg_to_int_on_i32() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; n.to_int(); 0 }"#);
    assert_ne!(exit, 0, "i32 has no to_int method");
}

#[test]
fn stage18_285_neg_is_zero_on_i32() {
    let exit = compile_only(r#"fn main() -> i32 { 5.is_zero(); 0 }"#);
    assert_ne!(exit, 0, "i32 has no is_zero method");
}

#[test]
fn stage18_285_neg_is_zero_on_bool() {
    let exit = compile_only(r#"fn main() -> i32 { true.is_zero(); 0 }"#);
    assert_ne!(exit, 0, "bool has no is_zero method");
}

#[test]
fn stage18_285_neg_to_int_on_string() {
    let exit = compile_only(r#"fn main() -> i32 { "hi".to_int(); 0 }"#);
    assert_ne!(exit, 0, "str has no to_int method");
}

#[test]
fn stage18_285_neg_user_impl_on_struct() {
    let exit = compile_only(
        r#"
        struct Foo { x: i32 }
        impl i32 {
            fn user_method(self) -> i32 { self }
        }
        fn main() -> i32 {
            let f = Foo { x: 1 };
            f.user_method();
            0
        }
        "#,
    );
    assert_ne!(exit, 0, "Foo has no user_method (i32 does)");
}

// Category 4: Wrong return type usage (4 cases)

#[test]
fn stage18_285_neg_to_int_assign_to_bool() {
    let exit = compile_only(r#"fn main() -> i32 { let n: bool = true.to_int(); 0 }"#);
    assert_ne!(exit, 0, "to_int returns i32, not bool");
}

#[test]
fn stage18_285_neg_to_int_assign_to_str() {
    let exit = compile_only(r#"fn main() -> i32 { let n: &str = true.to_int(); 0 }"#);
    assert_ne!(exit, 0, "to_int returns i32, not &str");
}

#[test]
fn stage18_285_neg_is_zero_assign_to_i32() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 0i64.is_zero(); 0 }"#);
    assert_ne!(exit, 0, "is_zero returns bool, not i32");
}

#[test]
fn stage18_285_neg_to_int_in_bool_expr() {
    let exit = compile_only(r#"fn main() -> i32 { if true.to_int() { 0 } else { 1 } }"#);
    assert_ne!(exit, 0, "if expects bool, not i32");
}

// Category 5: Method doesn't exist on primitive (6 cases)

#[test]
fn stage18_285_neg_i32_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; n.nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "unknown i32 method");
}

#[test]
fn stage18_285_neg_bool_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { true.foobar(); 0 }"#);
    assert_ne!(exit, 0, "unknown bool method");
}

#[test]
fn stage18_285_neg_i64_nonexistent_method() {
    // Stage 18.287: i64::abs now exists in prelude. Use a truly nonexistent method.
    let exit = compile_only(r#"fn main() -> i32 { let z: i64 = 0; z.nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "i64 has no nonexistent method");
}

#[test]
fn stage18_285_neg_i32_str_method() {
    let exit = compile_only(r#"fn main() -> i32 { 5.len(); 0 }"#);
    assert_ne!(exit, 0, "i32 has no len method");
}

#[test]
fn stage18_285_neg_bool_str_method() {
    let exit = compile_only(r#"fn main() -> i32 { true.as_bytes(); 0 }"#);
    assert_ne!(exit, 0, "bool has no as_bytes method");
}

#[test]
fn stage18_285_neg_i32_push_str() {
    let exit = compile_only(r#"fn main() -> i32 { 5.push_str("hi"); 0 }"#);
    assert_ne!(exit, 0, "i32 has no push_str method");
}

// Category 6: Method on wrong primitive (5 cases)

#[test]
fn stage18_285_neg_is_zero_on_u32() {
    let exit = compile_only(r#"fn main() -> i32 { let z: u32 = 0; z.is_zero(); 0 }"#);
    assert_ne!(exit, 0, "u32 has no is_zero method (i64 does)");
}

#[test]
fn stage18_285_neg_to_int_on_char() {
    let exit = compile_only(r#"fn main() -> i32 { 'a'.to_int(); 0 }"#);
    assert_ne!(exit, 0, "char has no to_int method");
}

#[test]
fn stage18_285_neg_is_zero_on_f64() {
    let exit = compile_only(r#"fn main() -> i32 { (3.14).is_zero(); 0 }"#);
    assert_ne!(exit, 0, "f64 has no is_zero method");
}

#[test]
fn stage18_285_neg_to_int_on_i8() {
    let exit = compile_only(r#"fn main() -> i32 { let b: i8 = 1; b.to_int(); 0 }"#);
    assert_ne!(exit, 0, "i8 has no to_int method");
}

#[test]
fn stage18_285_neg_user_impl_on_unit() {
    let exit = compile_only(r#"fn main() -> i32 { (()).to_int(); 0 }"#);
    assert_ne!(exit, 0, "unit has no to_int method");
}

// Category 7: User impl with mutability issues (3 cases)

#[test]
fn stage18_285_neg_user_impl_value_self_called_on_ref() {
    // User impl declares `fn by_ref_only(&self)` — verify &self methods
    // resolve correctly on primitive types (auto-ref path).
    // Note: returns constant to avoid dereferencing `self` (which requires
    // `*self` syntax not yet supported on primitive &self).
    let exit = compile_only(
        r#"
        impl i32 {
            fn by_ref_only(&self) -> i32 { 42 }
        }
        fn main() -> i32 {
            let n: i32 = 99;
            println!("{}", n.by_ref_only());
            0
        }
        "#,
    );
    assert_ne!(
        exit, 0,
        "user inherent impl on primitive type should be forbidden (类 Rust E0117)"
    );
}

#[test]
fn stage18_285_neg_user_impl_returns_mismatch() {
    let exit = compile_only(
        r#"
        impl i32 {
            fn bad_ret(self) -> i32 { true }
        }
        fn main() -> i32 { 0 }
        "#,
    );
    assert_ne!(exit, 0, "return type mismatch should fail");
}

#[test]
fn stage18_285_neg_user_impl_wrong_arg_type_3() {
    let exit = compile_only(
        r#"
        impl bool {
            fn takes_i32(self, n: i32) -> i32 { n }
        }
        fn main() -> i32 { true.takes_i32("str"); 0 }
        "#,
    );
    assert_ne!(exit, 0, "wrong arg type should fail");
}
