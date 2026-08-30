//! Stage 18.432 — §20 iterative audit: Non-exhaustive match check.
//!
//! Found during the §20 "finding one bug means there are many similar bugs"
//! audit round 8 (Stage 18.430). `match x { 1 => 1, 2 => 2 }` without a
//! catch-all `_` arm silently compiled for primitive types — fell through
//! to undefined behavior if x was not 1 or 2.
//!
//! Stage 18.430 attempted fix but BLOCKED by prelude (prelude has
//! `match self { true => 1, false => 0 }` on Bool without `_` arm).
//!
//! Stage 18.432 unblock: Properly handle Bool exhaustiveness (both `true`
//! and `false` patterns = exhaustive, no `_` needed). Prelude's bool match
//! is now recognized as exhaustive. Primitive types (Int/Uint/Char) still
//! require `_` arm. Enum/Adt/other types defer (exhaustiveness checking
//! requires knowing all variants — future work).
//!
//! Per §9.4.3: positive/negative ratio ≥ 1:3.
//! Per §20: iterative audit — same class as Stage 18.412-18.428.
//! Per §1.0 原則 4 (报错 > 静默): non-exhaustive match must be reported.

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
        std::env::temp_dir().join(format!("landin_432_{}_{}.lin", std::process::id(), id));
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
// Positive tests: exhaustive matches (valid)
// ============================================================================

#[test]
fn stage18_432_pos_int_with_wildcard() {
    let exit = compile_only(r#"fn main() -> i32 { let x: i32 = 5; match x { 1 => 1, _ => 0 } }"#);
    assert_eq!(exit, 0, "int match with _ must compile");
}

#[test]
fn stage18_432_pos_bool_both_arms() {
    let exit =
        compile_only(r#"fn main() -> i32 { let b = true; match b { true => 1, false => 0 } }"#);
    assert_eq!(exit, 0, "bool match with true+false must compile");
}

#[test]
fn stage18_432_pos_bool_with_wildcard() {
    let exit = compile_only(r#"fn main() -> i32 { let b = true; match b { true => 1, _ => 0 } }"#);
    assert_eq!(exit, 0, "bool match with _ must compile");
}

#[test]
fn stage18_432_pos_char_with_wildcard() {
    let exit =
        compile_only(r#"fn main() -> i32 { let c: char = 'a'; match c { 'a' => 1, _ => 0 } }"#);
    assert_eq!(exit, 0, "char match with _ must compile");
}

#[test]
fn stage18_432_pos_int_with_binding() {
    let exit = compile_only(r#"fn main() -> i32 { let x: i32 = 5; match x { 1 => 1, n => n } }"#);
    assert_eq!(exit, 0, "int match with binding (catch-all) must compile");
}

#[test]
fn stage18_432_pos_enum_match() {
    let exit = compile_only(
        r#"enum Opt { Some(i32), None } fn main() -> i32 { let x = Opt::Some(42); match x { Opt::Some(v) => v, Opt::None => 0 } }"#,
    );
    assert_eq!(exit, 0, "enum match (all variants) must compile");
}

// ============================================================================
// Negative tests: non-exhaustive matches on primitives (should error)
// ============================================================================

#[test]
fn stage18_432_neg_int_no_wildcard() {
    let exit =
        compile_only(r#"fn main() -> i32 { let x: i32 = 5; match x { 1 => 1, 2 => 2 }; 0 }"#);
    assert_ne!(exit, 0, "int match without _ must be rejected");
}

#[test]
fn stage18_432_neg_int_single_arm() {
    let exit = compile_only(r#"fn main() -> i32 { let x: i32 = 5; match x { 1 => 1 }; 0 }"#);
    assert_ne!(exit, 0, "int match with single arm must be rejected");
}

#[test]
fn stage18_432_neg_uint_no_wildcard() {
    let exit =
        compile_only(r#"fn main() -> i32 { let x: u64 = 5; match x { 0 => 1, 1 => 2 }; 0 }"#);
    assert_ne!(exit, 0, "uint match without _ must be rejected");
}

#[test]
fn stage18_432_neg_char_no_wildcard() {
    let exit = compile_only(
        r#"fn main() -> i32 { let c: char = 'a'; match c { 'a' => 1, 'b' => 2 }; 0 }"#,
    );
    assert_ne!(exit, 0, "char match without _ must be rejected");
}

// ============================================================================
// Negative tests: non-exhaustive bool match (should error)
// ============================================================================

#[test]
fn stage18_432_neg_bool_only_true() {
    let exit = compile_only(r#"fn main() -> i32 { let b = true; match b { true => 1 }; 0 }"#);
    assert_ne!(exit, 0, "bool match with only true must be rejected");
}

#[test]
fn stage18_432_neg_bool_only_false() {
    let exit = compile_only(r#"fn main() -> i32 { let b = true; match b { false => 0 }; 0 }"#);
    assert_ne!(exit, 0, "bool match with only false must be rejected");
}

// ============================================================================
// Positive tests: match in expression (should compile)
// ============================================================================

#[test]
fn stage18_432_pos_match_in_let() {
    let exit = compile_only(
        r#"fn main() -> i32 { let x: i32 = 5; let y = match x { 1 => 10, _ => 20 }; y }"#,
    );
    assert_eq!(exit, 0, "match in let binding with _ must compile");
}

#[test]
fn stage18_432_pos_match_as_return() {
    let exit = compile_only(
        r#"fn f(x: i32) -> i32 { match x { 0 => 1, _ => 2 } } fn main() -> i32 { f(5) }"#,
    );
    assert_eq!(exit, 0, "match as function return with _ must compile");
}
