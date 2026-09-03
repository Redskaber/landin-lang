//! Stage 18.336 (P1 soundness fix): Regression tests for ZST nested aggregate
//! Void leak + typeck return/trait gaps.
//!
//! Per §9.4.3 (1:3 pos:neg ratio): 4 positive + 8 negative = 12 tests.
//!
//! **What this file tests**:
//! 1. ZST fields/elements in struct/tuple/enum/array produce valid LLVM IR (A1-A4).
//! 2. Typeck catches ZST return type mismatches (B1/B3/B4).
//! 3. Typeck catches struct return with Infer rvalue (B2).
//! 4. Typeck catches Drop self receiver mismatch (C1).
//! 5. Typeck catches trait method return type width mismatch (C3).
//!
//! Per §1.0 原則 4 (报错 > 静默): all bug repros now report errors.
//! Per §20 (iterative audit): found via §20 Round 5 audit after Stages
//! 18.332-18.335.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, has_errors};
use landin_compiler::compile;
use std::process::Command;

// ============================================================================
// Helper: run `llvm-as` on TextEmitter IR output (copied from stage18_335)
// ============================================================================

fn assert_llvm_ir_valid(name: &str, code: &str) {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir_name = format!(
        "landin_ir_test_{}_{}_{}",
        std::process::id(),
        nanos,
        counter
    );
    let temp_dir: std::path::PathBuf = std::env::temp_dir().join(&dir_name);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let lin_file = temp_dir.join("input.lin");
    std::fs::write(&lin_file, code).expect("write .lin file");

    let ir_output = Command::new(&bin)
        .arg("--emit-llvm-ir")
        .arg(&lin_file)
        .env("TMPDIR", &temp_dir)
        .output()
        .expect("failed to execute landin-stage0");

    if !ir_output.status.success() {
        let stderr = String::from_utf8_lossy(&ir_output.stderr);
        panic!(
            "Test '{}': landin-stage0 --emit-llvm-ir failed.\nStderr: {}",
            name, stderr
        );
    }

    let ir_text = String::from_utf8_lossy(&ir_output.stdout).to_string();
    let ir_file = temp_dir.join("output.ll");
    std::fs::write(&ir_file, &ir_text).expect("write .ll file");

    let llvm_as_candidates = ["/tmp/llvm-22-prefix/bin/llvm-as", "llvm-as-22", "llvm-as"];
    let mut llvm_as_path: Option<&str> = None;
    for candidate in &llvm_as_candidates {
        if std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok()
        {
            llvm_as_path = Some(candidate);
            break;
        }
    }
    let llvm_as = match llvm_as_path {
        Some(p) => p,
        None => {
            eprintln!(
                "warn: Test '{}': llvm-as not found, skipping IR validity check",
                name
            );
            let _ = std::fs::remove_dir_all(&temp_dir);
            return;
        }
    };

    let bc_file = temp_dir.join("output.bc");
    let llvm_as_result = Command::new(llvm_as)
        .arg(&ir_file)
        .arg("-o")
        .arg(&bc_file)
        .output()
        .expect("failed to execute llvm-as");

    let _ = std::fs::remove_dir_all(&temp_dir);

    if !llvm_as_result.status.success() {
        let stderr = String::from_utf8_lossy(&llvm_as_result.stderr);
        let stdout = String::from_utf8_lossy(&llvm_as_result.stdout);
        panic!(
            "Test '{}': llvm-as rejected TextEmitter IR.\n\
            llvm-as stderr: {}\n\
            llvm-as stdout: {}\n\
            IR (first 500 chars): {}",
            name,
            stderr,
            stdout,
            ir_text.chars().take(500).collect::<String>()
        );
    }
}

// ============================================================================
// Positive tests: ZST nested aggregates produce valid IR (4 tests — A1-A4)
// ============================================================================

/// Stage 18.336 positive 1 (A1): ZST struct field produces valid IR + runs.
#[test]
fn stage18_336_zst_struct_field_valid() {
    let code = r#"
struct S { u: () }
fn main() -> i32 {
    let _s = S { u: () };
    0
}
"#;
    assert_llvm_ir_valid("zst-struct-field-ir", code);
    assert_runtime("zst-struct-field-run", code, "");
}

/// Stage 18.336 positive 2 (A2): ZST tuple element produces valid IR + runs.
#[test]
fn stage18_336_zst_tuple_elem_valid() {
    let code = r#"
fn main() -> i32 {
    let _t: (i32, ()) = (42, ());
    0
}
"#;
    assert_llvm_ir_valid("zst-tuple-elem-ir", code);
    assert_runtime("zst-tuple-elem-run", code, "");
}

/// Stage 18.336 positive 3 (A3): ZST enum payload produces valid IR + runs.
#[test]
fn stage18_336_zst_enum_payload_valid() {
    let code = r#"
enum E { V(()), W(i32) }
fn main() -> i32 {
    let _e = E::V(());
    0
}
"#;
    assert_llvm_ir_valid("zst-enum-payload-ir", code);
    assert_runtime("zst-enum-payload-run", code, "");
}

/// Stage 18.336 positive 4 (A4): ZST array element produces valid IR + runs.
#[test]
fn stage18_336_zst_array_elem_valid() {
    let code = r#"
fn main() -> i32 {
    let _a: [(); 3] = [(), (), ()];
    0
}
"#;
    assert_llvm_ir_valid("zst-array-elem-ir", code);
    assert_runtime("zst-array-elem-run", code, "");
}

// ============================================================================
// Negative tests: Typeck catches ZST return / struct return / trait gaps (8 tests)
// ============================================================================

/// Stage 18.336 negative 1 (B1): ZST return with i64 rvalue must error.
#[test]
fn stage18_336_zst_return_i64_errors() {
    let result = compile(
        r#"
fn foo() -> () { 42i64 }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Returning i64 from fn declared to return () must report typeck error"
    );
}

/// Stage 18.336 negative 2 (B3): ZST return with bool rvalue must error.
#[test]
fn stage18_336_zst_return_bool_errors() {
    let result = compile(
        r#"
fn foo() -> () { true }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Returning bool from fn declared to return () must report typeck error"
    );
}

/// Stage 18.336 negative 3 (B4): Implicit ZST return with i64 rvalue must error.
#[test]
fn stage18_336_implicit_zst_return_i64_errors() {
    let result = compile(
        r#"
fn foo() { 42i64 }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Returning i64 from fn with implicit () return must report typeck error"
    );
}

/// Stage 18.336 negative 4 (B2): Struct return with Infer int rvalue must error.
#[test]
fn stage18_336_struct_return_infer_errors() {
    let result = compile(
        r#"
struct S { x: i32 }
fn foo() -> S { 42 }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Returning Infer int from fn declared to return S must report typeck error"
    );
}

/// Stage 18.336 negative 5 (C1): Drop with wrong self receiver must error.
#[test]
fn stage18_336_drop_wrong_self_errors() {
    let result = compile(
        r#"
struct Foo;
impl Drop for Foo { fn drop(self) { } }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Drop::drop with `self` receiver (should be `&mut self`) must report typeck error"
    );
}

/// Stage 18.336 negative 6 (C2): Trait method with wrong self receiver must error.
#[test]
fn stage18_336_trait_wrong_self_errors() {
    let result = compile(
        r#"
trait T { fn f(&self); }
struct X;
impl T for X { fn f(self) { } }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Trait method `f` with `self` receiver (should be `&self`) must report typeck error"
    );
}

/// Stage 18.336 negative 7 (C3): Trait method with wrong return int width must error.
#[test]
fn stage18_336_trait_wrong_int_width_errors() {
    let result = compile(
        r#"
trait T { fn f() -> i32; }
struct X;
impl T for X { fn f() -> i64 { 0 } }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Trait method returning i64 (declared i32) must report typeck error"
    );
}

/// Stage 18.336 negative 8 (C3 variant): Trait method with Int vs Uint must error.
#[test]
fn stage18_336_trait_int_vs_uint_errors() {
    let result = compile(
        r#"
trait T { fn f() -> i32; }
struct X;
impl T for X { fn f() -> u32 { 0 } }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Trait method returning u32 (declared i32) must report typeck error"
    );
}
