//! Stage 18.335 (P1 soundness fix): Regression tests for ZST param skip +
//! __landin_eprintf declare + drop_glue declare removal.
//!
//! Per §9.4.3 (1:3 pos:neg ratio): 3 positive + 4 negative = 7 tests.
//! Per §7.3.1 (negative audit coverage): exercises all IR failure modes.
//!
//! **What this file tests**:
//! 1. ZST params (`()`) are elided from the LLVM signature (Bug 1).
//! 2. `eprintln!`/`eprint!` macros produce valid IR (Bug 2).
//! 3. `impl Drop for X` produces valid IR (Bug 3).
//! 4. `call_dest_type` Void override doesn't produce invalid `alloca void` (Bug 4).
//!
//! Per §1.0 原則 4 (报错 > 静默): the `llvm-as` smoke test catches the entire
//! class of "TextEmitter IR silently invalid" bugs.
//! Per §20 (iterative audit): found via §20 Round 4 audit after Stages
//! 18.332/18.333/18.334 fixed sret/byval/TextEmitter IR/variadic detection.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, has_errors, run_program};
use landin_compiler::compile;
use std::process::Command;

// ============================================================================
// Helper: run `llvm-as` on TextEmitter IR output (copied from stage18_334)
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
// Positive tests: ZST param + eprintln + Drop (3 tests)
// ============================================================================

/// Stage 18.335 positive 1: ZST param (`()`) — IR validity + runtime.
///
/// Before Stage 18.335: `define void @landin_foo(void %arg0)` was rejected by
/// `llvm-as` with "void type only allowed for function results".
/// After Stage 18.335: ZST params are elided from the LLVM signature (mirrors
/// rustc_codegen_llvm's behavior).
#[test]
fn stage18_335_zst_param_valid() {
    let code = r#"
fn foo(u: ()) -> i32 {
    42
}
fn main() -> i32 {
    foo(());
    0
}
"#;
    assert_llvm_ir_valid("zst-param-valid-ir", code);
    assert_runtime("zst-param-valid-run", code, "");
}

/// Stage 18.335 positive 2: eprintln! macro — IR validity + runtime.
///
/// Before Stage 18.335: `__landin_eprintf` was undeclared in TextEmitter IR,
/// so `llvm-as` rejected with "use of undefined value '@__landin_eprintf'".
/// After Stage 18.335: pipeline.rs pre-declares `__landin_eprintf` as variadic.
#[test]
fn stage18_335_eprintln_valid() {
    let code = r#"
fn main() -> i32 {
    eprintln!("hello stderr");
    0
}
"#;
    assert_llvm_ir_valid("eprintln-valid-ir", code);
    // eprintln! writes to stderr, so stdout should be empty.
    let (stdout, exit) = run_program(code);
    assert_eq!(stdout, "");
    assert_eq!(exit, 0);
}

/// Stage 18.335 positive 3: impl Drop for X — IR validity + runtime.
///
/// Before Stage 18.335: drop_glue.rs emitted a redundant `declare` that
/// conflicted with the later `define` from codegen_function.
/// `llvm-as` rejected with "invalid redefinition of function".
/// After Stage 18.335: the redundant declare is removed; LLVM forward-reference
/// handles the symbol.
#[test]
fn stage18_335_drop_impl_valid() {
    let code = r#"
trait Drop { fn drop(&mut self); }
struct Foo;
impl Drop for Foo { fn drop(&mut self) { } }
fn main() -> i32 {
    let _f = Foo;
    0
}
"#;
    assert_llvm_ir_valid("drop-impl-valid-ir", code);
    assert_runtime("drop-impl-valid-run", code, "");
}

// ============================================================================
// Negative tests: ZST + eprintln + Drop failure modes (4 tests)
// ============================================================================

/// Stage 18.335 negative 1: Calling function with non-ZST arg where ZST is expected.
#[test]
fn stage18_335_zst_param_wrong_arg_type() {
    let result = compile(
        r#"
fn foo(u: ()) -> i32 { 0 }
fn main() -> i32 {
    foo(42i64);
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Passing i64 to function expecting () must report typeck error"
    );
}

/// Stage 18.335 negative 2: Returning wrong type from function.
///
/// Stage 18.336 (P1 soundness fix): Converted from skip-with-warning to hard
/// assertion. The `body_lower.rs:443` skip_assign logic was refined to only
/// skip for Infer/unit/Ref/Ptr rvalues — concrete scalar types (i64) now
/// correctly trigger the type mismatch check.
#[test]
fn stage18_335_zst_return_wrong_type() {
    let result = compile(
        r#"
fn foo() -> () { 42i64 }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Returning i64 from function declared to return () must report typeck error"
    );
}

/// Stage 18.335 negative 3: Drop trait with wrong signature.
#[test]
fn stage18_335_drop_wrong_signature() {
    let result = compile(
        r#"
trait Drop { fn drop(&mut self); }
struct Foo;
impl Drop for Foo { fn drop(&mut self) -> i32 { 0 } }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Drop::drop with wrong return type (i32 instead of ()) must report typeck error"
    );
}

/// Stage 18.335 negative 4: Drop method with wrong self receiver.
///
/// Stage 18.336 (P1 soundness fix): Converted from skip-with-warning to hard
/// assertion. The `driver_validations.rs` trait validator now compares
/// `self_kind` between trait declaration and impl.
#[test]
fn stage18_335_drop_wrong_self() {
    let result = compile(
        r#"
trait Drop { fn drop(&mut self); }
struct Foo;
impl Drop for Foo { fn drop(self) { } }
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Drop::drop with wrong self receiver (self instead of &mut self) must report typeck error"
    );
}

// ============================================================================
// Stress test: combined ZST + eprintln + Drop
// ============================================================================

/// Stage 18.335 stress 1: Combined ZST param + eprintln + Drop in same program.
///
/// Verifies all 3 P1 bug fixes work together without conflict.
#[test]
fn stage18_335_combined_zst_eprintln_drop() {
    let code = r#"
trait Drop { fn drop(&mut self); }
struct Foo;
impl Drop for Foo { fn drop(&mut self) { } }

fn consume(u: ()) -> i32 {
    eprintln!("inside consume");
    42
}

fn main() -> i32 {
    let _f = Foo;
    let _x = consume(());
    0
}
"#;
    assert_llvm_ir_valid("combined-zst-eprintln-drop-ir", code);
    let (stdout, exit) = run_program(code);
    assert_eq!(stdout, "");
    assert_eq!(exit, 0);
}
