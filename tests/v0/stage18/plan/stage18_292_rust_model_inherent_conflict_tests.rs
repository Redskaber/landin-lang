//! Stage 18.292 — 类 Rust 架构修正: inherent impl 冲突检测 (不允许覆盖)。
//!
//! 验证类 Rust 设计:
//! - 用户不能覆盖 prelude 定义的原始类型方法 (冲突报错)
//! - 两个 `impl Type { fn same_method {} }` 报 "duplicate definitions"
//! - 用户可以定义新方法 `impl str { fn my_method {} }` (不冲突)
//!
//! Per §12 (最优>最小): 类 Rust 设计 — 不允许覆盖, 冲突即报错。
//! Per §1.0 原則 6 (通解>特解): one check for all inherent impl conflicts。
//! Per §2 原則 4 (报错>静默): conflicts must be reported。

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
        std::env::temp_dir().join(format!("landin_292_{}_{}.lin", std::process::id(), id));
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
// POSITIVE TESTS (8) — prelude methods + user new methods work
// =============================================================================

#[test]
fn stage18_292_prelude_str_len_intrinsic() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let s = "hello"; println!("{}", s.len()); 0 }"#);
    assert_eq!(stdout, "5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_292_prelude_str_is_empty_intrinsic() {
    let (stdout, exit) = run_program(r#"fn main() -> i32 { println!("{}", "".is_empty()); 0 }"#);
    assert_eq!(stdout, "true\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_292_prelude_i64_is_zero() {
    let (stdout, exit) =
        run_program(r#"fn main() -> i32 { let z: i64 = 0; println!("{}", z.is_zero()); 0 }"#);
    assert_eq!(stdout, "true\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_292_prelude_bool_to_int() {
    let (stdout, exit) = run_program(r#"fn main() -> i32 { println!("{}", true.to_int()); 0 }"#);
    assert_eq!(stdout, "1\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_292_user_new_str_method() {
    let exit = compile_only(
        r#"impl str { fn my_method(&self) -> i64 { 99 } } fn main() -> i32 { println!("{}", "hi".my_method()); 0 }"#,
    );
    assert_eq!(exit, 0, "user inherent impl now allowed (Stage 18.341)");
}

#[test]
fn stage18_292_user_new_i32_method() {
    let exit = compile_only(
        r#"impl i32 { fn double(self) -> i32 { self + self } } fn main() -> i32 { println!("{}", 21i32.double()); 0 }"#,
    );
    assert_eq!(exit, 0, "user inherent impl now allowed (Stage 18.341)");
}

#[test]
fn stage18_292_user_new_bool_method() {
    let exit = compile_only(
        r#"impl bool { fn to_str_val(self) -> i32 { match self { true => 1i32, false => 0i32 } } } fn main() -> i32 { println!("{}", true.to_str_val()); 0 }"#,
    );
    assert_eq!(exit, 0, "user inherent impl now allowed (Stage 18.341)");
}

#[test]
fn stage18_292_two_different_methods_same_type() {
    let exit = compile_only(
        r#"impl i32 { fn a(self) -> i32 { self } } impl i32 { fn b(self) -> i32 { self } } fn main() -> i32 { println!("{} {}", 5i32.a(), 5i32.b()); 0 }"#,
    );
    assert_eq!(exit, 0, "user inherent impl now allowed (Stage 18.341)");
}

// =============================================================================
// NEGATIVE AUDIT SET (32) — 冲突检测 + §7.3.1 全部 7 类错误
// =============================================================================

// Category 1: 冲突 — 用户覆盖 prelude (应报错)

#[test]
fn stage18_292_neg_user_override_str_len() {
    let exit = compile_only(r#"impl str { fn len(&self) -> i64 { 42 } } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "用户不能覆盖 prelude str::len");
}

#[test]
fn stage18_292_neg_user_override_str_is_empty() {
    let exit =
        compile_only(r#"impl str { fn is_empty(&self) -> bool { false } } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "用户不能覆盖 prelude str::is_empty");
}

#[test]
fn stage18_292_neg_user_override_str_as_bytes() {
    let exit =
        compile_only(r#"impl str { fn as_bytes(&self) -> i32 { 99 } } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "用户不能覆盖 prelude str::as_bytes");
}

#[test]
fn stage18_292_neg_user_override_i64_is_zero() {
    let exit =
        compile_only(r#"impl i64 { fn is_zero(self) -> bool { false } } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "用户不能覆盖 prelude i64::is_zero");
}

#[test]
fn stage18_292_neg_user_override_bool_to_int() {
    let exit =
        compile_only(r#"impl bool { fn to_int(self) -> i32 { 99 } } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "用户不能覆盖 prelude bool::to_int");
}

// Category 2: 冲突 — 两个用户 impl 同方法 (应报错)

#[test]
fn stage18_292_neg_two_user_impls_str_len() {
    let exit = compile_only(
        r#"impl str { fn len(&self) -> i64 { 100 } } impl str { fn len(&self) -> i64 { 200 } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "两个 impl str fn len 冲突");
}

#[test]
fn stage18_292_neg_two_user_impls_i32_method() {
    let exit = compile_only(
        r#"impl i32 { fn custom(self) -> i32 { 1 } } impl i32 { fn custom(self) -> i32 { 2 } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "两个 impl i32 fn custom 冲突");
}

#[test]
fn stage18_292_neg_two_user_impls_bool_method() {
    let exit = compile_only(
        r#"impl bool { fn m(self) -> i32 { 1 } } impl bool { fn m(self) -> i32 { 2 } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "两个 impl bool fn m 冲突");
}

#[test]
fn stage18_292_neg_two_user_impls_struct_method() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } impl Foo { fn m(&self) -> i32 { 1 } } impl Foo { fn m(&self) -> i32 { 2 } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "两个 impl Foo fn m 冲突");
}

#[test]
fn stage18_292_neg_two_user_impls_i64_method() {
    let exit = compile_only(
        r#"impl i64 { fn custom(self) -> i64 { 1 } } impl i64 { fn custom(self) -> i64 { 2 } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "两个 impl i64 fn custom 冲突");
}

// Category 3: Wrong arg count (5 cases)

#[test]
fn stage18_292_neg_str_len_with_arg() {
    let exit = compile_only(r#"fn main() -> i32 { "hi".len(42); 0 }"#);
    assert_ne!(exit, 0, "str::len takes no args");
}

#[test]
fn stage18_292_neg_to_int_with_arg() {
    let exit = compile_only(r#"fn main() -> i32 { true.to_int(42); 0 }"#);
    assert_ne!(exit, 0, "to_int takes no args");
}

#[test]
fn stage18_292_neg_is_zero_with_arg() {
    let exit = compile_only(r#"fn main() -> i32 { let z: i64 = 0; z.is_zero(99); 0 }"#);
    assert_ne!(exit, 0, "is_zero takes no args");
}

#[test]
fn stage18_292_neg_user_method_wrong_arg_count() {
    let exit = compile_only(
        r#"impl i32 { fn add_one(self, x: i32) -> i32 { self + x } } fn main() -> i32 { 5.add_one(); 0 }"#,
    );
    assert_ne!(exit, 0, "add_one requires 1 arg");
}

#[test]
fn stage18_292_neg_user_method_extra_args() {
    let exit = compile_only(
        r#"impl i32 { fn noop(self) -> i32 { self } } fn main() -> i32 { 5.noop(1, 2); 0 }"#,
    );
    assert_ne!(exit, 0, "noop takes no args");
}

// Category 4: Wrong arg type (4 cases)

#[test]
fn stage18_292_neg_user_impl_wrong_arg_type() {
    let exit = compile_only(
        r#"impl i32 { fn add_to(self, n: i32) -> i32 { self + n } } fn main() -> i32 { 5.add_to(true); 0 }"#,
    );
    assert_ne!(exit, 0, "add_to expects i32, not bool");
}

#[test]
fn stage18_292_neg_user_impl_wrong_return_assign() {
    let exit = compile_only(
        r#"impl i32 { fn ret_i32(self) -> i32 { self } } fn main() -> i32 { let n: bool = 5.ret_i32(); 0 }"#,
    );
    assert_ne!(exit, 0, "ret_i32 returns i32, not bool");
}

#[test]
fn stage18_292_neg_user_impl_wrong_self_type() {
    let exit = compile_only(
        r#"impl i32 { fn only_on_i32(self) -> i32 { self } } fn main() -> i32 { let b: bool = true; b.only_on_i32(); 0 }"#,
    );
    assert_ne!(exit, 0, "only_on_i32 not defined for bool");
}

#[test]
fn stage18_292_neg_user_impl_takes_wrong_arg() {
    let exit = compile_only(
        r#"impl str { fn takes_i32(&self, n: i32) -> i64 { 42 } } fn main() -> i32 { "hi".takes_i32("str"); 0 }"#,
    );
    assert_ne!(exit, 0, "wrong arg type");
}

// Category 5: Method doesn't exist (5 cases)

#[test]
fn stage18_292_neg_str_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { "hi".nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "unknown str method");
}

#[test]
fn stage18_292_neg_i64_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { let z: i64 = 0; z.nonexistent(); 0 }"#);
    assert_ne!(exit, 0, "unknown i64 method");
}

#[test]
fn stage18_292_neg_bool_nonexistent_method() {
    let exit = compile_only(r#"fn main() -> i32 { true.foobar(); 0 }"#);
    assert_ne!(exit, 0, "unknown bool method");
}

#[test]
fn stage18_292_neg_i32_abs_not_in_prelude() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = -5; n.abs(); 0 }"#);
    assert_ne!(exit, 0, "i32::abs not in prelude");
}

#[test]
fn stage18_292_neg_impl_calls_nonexistent_fn() {
    let exit = compile_only(
        r#"impl str { fn my_method(&self) -> i64 { nonexistent() } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent function in body");
}

// Category 6: Method on wrong primitive (5 cases)

#[test]
fn stage18_292_neg_len_on_i32() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 5; n.len(); 0 }"#);
    assert_ne!(exit, 0, "i32 has no len");
}

#[test]
fn stage18_292_neg_is_empty_on_bool() {
    let exit = compile_only(r#"fn main() -> i32 { true.is_empty(); 0 }"#);
    assert_ne!(exit, 0, "bool has no is_empty");
}

#[test]
fn stage18_292_neg_is_zero_on_i32() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i32 = 0; n.is_zero(); 0 }"#);
    assert_ne!(exit, 0, "i32 has no is_zero");
}

#[test]
fn stage18_292_neg_to_int_on_i64() {
    let exit = compile_only(r#"fn main() -> i32 { let n: i64 = 5; n.to_int(); 0 }"#);
    assert_ne!(exit, 0, "i64 has no to_int");
}

#[test]
fn stage18_292_neg_user_impl_on_struct() {
    let exit = compile_only(
        r#"struct Foo { x: i32 } impl i32 { fn custom(self) -> i32 { self } } fn main() -> i32 { let f = Foo { x: 1 }; f.custom(); 0 }"#,
    );
    assert_ne!(exit, 0, "Foo has no custom method");
}

// Category 7: User impl type issues (3 cases)

#[test]
fn stage18_292_neg_user_impl_returns_mismatch() {
    let exit =
        compile_only(r#"impl str { fn bad_ret(&self) -> i64 { true } } fn main() -> i32 { 0 }"#);
    assert_ne!(exit, 0, "return type mismatch");
}

#[test]
fn stage18_292_neg_user_impl_mismatched_branches() {
    let exit = compile_only(
        r#"impl str { fn bad(&self) -> i64 { if true { 1i64 } else { "str" } } } fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "if branches have different types");
}

#[test]
fn stage18_292_neg_user_impl_wrong_sig() {
    let exit = compile_only(
        r#"impl i32 { fn custom(self, extra: i32) -> i32 { self } } fn main() -> i32 { 5.custom(); 0 }"#,
    );
    assert_ne!(exit, 0, "custom requires 1 arg");
}
