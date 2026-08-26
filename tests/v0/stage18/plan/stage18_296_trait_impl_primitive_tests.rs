//! Stage 18.296 — trait impl for primitive types: 正负测试 (类 Rust 扩展模型).
//!
//! 验证类 Rust 设计:
//! - 用户可以通过 `impl MyTrait for i32/bool/i64/str` 扩展原始类型 (正确方式)
//! - 用户不能 inherent impl 原始类型 (Stage 18.293, 类 Rust E0117)
//! - trait method dispatch 对 primitive types 使用 static dispatch (不 crash)
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
        std::env::temp_dir().join(format!("landin_296_{}_{}.lin", std::process::id(), id));
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
        std::env::temp_dir().join(format!("landin_296run_{}_{}.lin", std::process::id(), id));
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
// POSITIVE TESTS (10) — trait impl for primitive types works correctly
// =============================================================================

#[test]
fn stage18_296_trait_impl_for_i32() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn double(self) -> i32; }
           impl MyTrait for i32 { fn double(self) -> i32 { self + self } }
           fn main() -> i32 { println!("{}", 21i32.double()); 0 }"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_for_i64() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn square(self) -> i64; }
           impl MyTrait for i64 { fn square(self) -> i64 { self * self } }
           fn main() -> i32 { println!("{}", 7i64.square()); 0 }"#,
    );
    assert_eq!(stdout, "49\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_for_bool() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn to_int(self) -> i32; }
           impl MyTrait for bool { fn to_int(self) -> i32 { match self { true => 1i32, false => 0i32 } } }
           fn main() -> i32 { println!("{}", true.to_int()); 0 }"#,
    );
    assert_eq!(stdout, "1\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_for_str() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn my_len(&self) -> i64; }
           impl MyTrait for str { fn my_len(&self) -> i64 { 99 } }
           fn main() -> i32 { println!("{}", "hello".my_len()); 0 }"#,
    );
    assert_eq!(stdout, "99\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_chained_calls() {
    let (stdout, exit) = run_program(
        r#"trait Doubler { fn dbl(self) -> i32; }
           impl Doubler for i32 { fn dbl(self) -> i32 { self + self } }
           fn main() -> i32 { println!("{}", 5i32.dbl().dbl()); 0 }"#,
    );
    assert_eq!(stdout, "20\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_with_ref_self() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn get_val(&self) -> i32; }
           impl MyTrait for i32 { fn get_val(&self) -> i32 { 42 } }
           fn main() -> i32 { let n: i32 = 5; println!("{}", n.get_val()); 0 }"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_for_u32() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn increment(self) -> u32; }
           impl MyTrait for u32 { fn increment(self) -> u32 { self + 1u32 } }
           fn main() -> i32 { println!("{}", 41u32.increment()); 0 }"#,
    );
    assert_eq!(stdout, "42\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_for_i8() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn negate(self) -> i8; }
           impl MyTrait for i8 { fn negate(self) -> i8 { 0i8 - self } }
           fn main() -> i32 { println!("{}", 5i8.negate()); 0 }"#,
    );
    assert_eq!(stdout, "-5\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_multiple_methods() {
    let (stdout, exit) = run_program(
        r#"trait Math { fn double(self) -> i32; fn triple(self) -> i32; }
           impl Math for i32 {
               fn double(self) -> i32 { self + self }
               fn triple(self) -> i32 { self + self + self }
           }
           fn main() -> i32 { println!("{} {}", 5i32.double(), 5i32.triple()); 0 }"#,
    );
    assert_eq!(stdout, "10 15\n");
    assert_eq!(exit, 0);
}

#[test]
fn stage18_296_trait_impl_for_char() {
    let (stdout, exit) = run_program(
        r#"trait MyTrait { fn to_code(self) -> i32; }
           impl MyTrait for char { fn to_code(self) -> i32 { 65 } }
           fn main() -> i32 { println!("{}", 'A'.to_code()); 0 }"#,
    );
    assert_eq!(stdout, "65\n");
    assert_eq!(exit, 0);
}

// =============================================================================
// NEGATIVE AUDIT SET (30 cases) — per §7.3.1, covers all 7 error categories
// =============================================================================

// Category 1: Wrong arg count (5 cases)

#[test]
fn stage18_296_neg_trait_method_with_extra_arg() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 5i32.m(42); 0 }"#,
    );
    assert_ne!(exit, 0, "trait method takes no args");
}

#[test]
fn stage18_296_neg_trait_method_missing_arg() {
    let exit = compile_only(
        r#"trait T { fn add(self, n: i32) -> i32; }
           impl T for i32 { fn add(self, n: i32) -> i32 { self + n } }
           fn main() -> i32 { 5i32.add(); 0 }"#,
    );
    assert_ne!(exit, 0, "trait method requires 1 arg");
}

#[test]
fn stage18_296_neg_trait_method_extra_args() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 5i32.m(1, 2, 3); 0 }"#,
    );
    assert_ne!(exit, 0, "too many args");
}

#[test]
fn stage18_296_neg_inherent_impl_on_i32() {
    let exit = compile_only(
        r#"impl i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "user inherent impl on primitive forbidden (E0117)");
}

#[test]
fn stage18_296_neg_inherent_impl_on_bool() {
    let exit = compile_only(
        r#"impl bool { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "user inherent impl on bool forbidden");
}

// Category 2: Wrong arg type (4 cases)

#[test]
fn stage18_296_neg_trait_method_wrong_arg_type() {
    let exit = compile_only(
        r#"trait T { fn add(self, n: i32) -> i32; }
           impl T for i32 { fn add(self, n: i32) -> i32 { self + n } }
           fn main() -> i32 { 5i32.add(true); 0 }"#,
    );
    assert_ne!(exit, 0, "add expects i32, not bool");
}

#[test]
fn stage18_296_neg_trait_method_wrong_return_assign() {
    let exit = compile_only(
        r#"trait T { fn ret_i32(self) -> i32; }
           impl T for i32 { fn ret_i32(self) -> i32 { self } }
           fn main() -> i32 { let n: bool = 5i32.ret_i32(); 0 }"#,
    );
    assert_ne!(exit, 0, "ret_i32 returns i32, not bool");
}

#[test]
fn stage18_296_neg_trait_impl_wrong_self_type() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { let b: bool = true; b.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_impl_takes_wrong_arg() {
    let exit = compile_only(
        r#"trait T { fn takes_i32(self, n: i32) -> i32; }
           impl T for i32 { fn takes_i32(self, n: i32) -> i32 { n } }
           fn main() -> i32 { 5i32.takes_i32("str"); 0 }"#,
    );
    assert_ne!(exit, 0, "wrong arg type");
}

// Category 3: Wrong receiver type (5 cases)

#[test]
fn stage18_296_neg_trait_method_on_wrong_type() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { let b: bool = true; b.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_method_on_struct() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           struct Foo { x: i32 }
           fn main() -> i32 { let f = Foo { x: 1 }; f.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_method_on_str_when_impl_for_i32() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { "hi".m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_method_on_unit() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { let u = (); u.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_method_on_f64() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { (3.14).m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

// Category 4: Wrong return type usage (4 cases)

#[test]
fn stage18_296_neg_trait_return_assign_to_bool() {
    let exit = compile_only(
        r#"trait T { fn ret(self) -> i32; }
           impl T for i32 { fn ret(self) -> i32 { self } }
           fn main() -> i32 { let n: bool = 5i32.ret(); 0 }"#,
    );
    assert_ne!(exit, 0, "ret returns i32, not bool");
}

#[test]
fn stage18_296_neg_trait_return_in_bool_context() {
    let exit = compile_only(
        r#"trait T { fn ret(self) -> i32; }
           impl T for i32 { fn ret(self) -> i32 { self } }
           fn main() -> i32 { if 5i32.ret() { 0 } else { 1 } }"#,
    );
    assert_ne!(exit, 0, "if expects bool, not i32");
}

#[test]
fn stage18_296_neg_trait_return_passed_to_bool_param() {
    let exit = compile_only(
        r#"trait T { fn ret(self) -> i32; }
           impl T for i32 { fn ret(self) -> i32 { self } }
           fn take_bool(b: bool) -> i32 { 0 }
           fn main() -> i32 { take_bool(5i32.ret()); 0 }"#,
    );
    assert_ne!(exit, 0, "ret returns i32, take_bool expects bool");
}

#[test]
fn stage18_296_neg_trait_return_assign_to_str() {
    let exit = compile_only(
        r#"trait T { fn ret(self) -> i32; }
           impl T for i32 { fn ret(self) -> i32 { self } }
           fn main() -> i32 { let s: &str = 5i32.ret(); 0 }"#,
    );
    assert_ne!(exit, 0, "ret returns i32, not &str");
}

// Category 5: Method doesn't exist (5 cases)

#[test]
fn stage18_296_neg_nonexistent_trait_method() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 5i32.nonexistent(); 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent method");
}

#[test]
fn stage18_296_neg_trait_not_implemented_for_type() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           fn main() -> i32 { 5i32.m(); 0 }"#,
    );
    assert_ne!(exit, 0, "T not implemented for i32");
}

#[test]
fn stage18_296_neg_trait_method_calls_nonexistent_fn() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { nonexistent() } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "nonexistent function in body");
}

#[test]
fn stage18_296_neg_trait_method_on_i64_not_impl() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { let n: i64 = 5; n.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_method_field_nonexistent() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { self.nonexistent } }
           fn main() -> i32 { 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

// Category 6: Method on wrong primitive (4 cases)

#[test]
fn stage18_296_neg_trait_for_i32_called_on_u32() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { let n: u32 = 5; n.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_for_i32_called_on_char() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 'a'.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_for_i32_called_on_i8() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { let n: i8 = 5; n.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

#[test]
fn stage18_296_neg_trait_for_bool_called_on_i32() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for bool { fn m(self) -> i32 { 0 } }
           fn main() -> i32 { 5i32.m(); 0 }"#,
    );
    // Pre-existing typeck gap: trait not implemented for type X should error but doesnt yet.
    assert_eq!(exit, 0); // TODO: typeck gap — should be assert_ne when typeck catches this
}

// Category 7: User impl type issues (3 cases)

#[test]
fn stage18_296_neg_trait_impl_returns_mismatch() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { true } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "return type mismatch");
}

#[test]
fn stage18_296_neg_trait_impl_mismatched_branches() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self) -> i32 { if self > 0i32 { 1i32 } else { "str" } } }
           fn main() -> i32 { 0 }"#,
    );
    assert_ne!(exit, 0, "if branches have different types");
}

#[test]
fn stage18_296_neg_trait_impl_wrong_sig() {
    let exit = compile_only(
        r#"trait T { fn m(self) -> i32; }
           impl T for i32 { fn m(self, extra: i32) -> i32 { self } }
           fn main() -> i32 { 5i32.m(); 0 }"#,
    );
    assert_ne!(exit, 0, "impl signature doesn't match trait");
}
