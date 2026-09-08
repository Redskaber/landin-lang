//! Stage 160 (v0.16 — TD-TYPECK-GENERIC-ARG-VALIDATION): Fix typeck to
//! validate generic call args against specialized signature.
//!
//! ## Root cause
//!
//! `typeck/check.rs` check_terminator's Call arg validation used
//! `sig.inputs` directly (containing `Param(N)`) instead of specializing
//! with the call-site substs from the FnDef type. `unify(Param(0), i32)`
//! silently succeeded (Param unifies with anything), so
//! `identity::<i64>(42i32)` was accepted without error.
//!
//! ## Fix
//!
//! When `func_ty` is `FnDef(def_id, substs)` and substs are non-empty,
//! substitute `Param(N)` in `sig.inputs` with the concrete substs BEFORE
//! unifying. This makes `unify(i64, i32)` report a type error.
//!
//! Also fixed `bind_ty_var` in `typeck/unify.rs` to be bounds-safe (when
//! Param index exceeds ty_vars len, skip binding instead of panicking).
//!
//! Per §1.0 原則 4 (报错 > 静默): report type mismatch.
//! Per §1.0 原則 9 (正确 > 妥协): validate with specialized sig.
//! Per §1.0 原則 10 (唯一可信数据源): substs from FnDef type.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Positive tests — correct generic calls still work
// ===========================================================================

#[test]
fn stage160_identity_i64_correct() {
    let code = r#"
fn identity<T>(x: T) -> T { x }
fn main() -> i32 {
    let v: i64 = identity::<i64>(42i64);
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage160_identity_i32_correct() {
    let code = r#"
fn identity<T>(x: T) -> T { x }
fn main() -> i32 {
    let v: i32 = identity::<i32>(42i32);
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage160_identity_inferred() {
    let code = r#"
fn identity<T>(x: T) -> T { x }
fn main() -> i32 {
    let v: i64 = identity(42i64);
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

// ===========================================================================
// Negative tests — type mismatch detected
// ===========================================================================

#[test]
fn stage160_identity_i64_with_i32_arg_errors() {
    let code = r#"
fn identity<T>(x: T) -> T { x }
fn main() -> i32 {
    let v: i64 = identity::<i64>(42i32);
    0
}
"#;
    let (_, exit) = run_program(code);
    assert_ne!(exit, 0, "identity::<i64>(42i32) should be a type error");
}

#[test]
fn stage160_identity_i32_with_i64_arg_errors() {
    let code = r#"
fn identity<T>(x: T) -> T { x }
fn main() -> i32 {
    let v: i32 = identity::<i32>(42i64);
    0
}
"#;
    let (_, exit) = run_program(code);
    assert_ne!(exit, 0, "identity::<i32>(42i64) should be a type error");
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn stage160_two_generic_args_correct() {
    let code = r#"
fn pair<T, U>(a: T, b: U) -> T { a }
fn main() -> i32 {
    let v: i64 = pair::<i64, bool>(42i64, true);
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage160_two_generic_args_mismatch() {
    let code = r#"
fn pair<T, U>(a: T, b: U) -> T { a }
fn main() -> i32 {
    let v: i64 = pair::<i64, bool>(42i32, true);
    0
}
"#;
    let (_, exit) = run_program(code);
    assert_ne!(
        exit, 0,
        "pair::<i64, bool>(42i32, true) should be a type error"
    );
}

#[test]
fn stage160_generic_struct_method() {
    let code = r#"
struct Container<T> { val: T }
impl<T> Container<T> {
    fn get(&self) -> T { self.val }
}
fn main() -> i32 {
    let c = Container { val: 42i64 };
    let v: i64 = c.get();
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}
