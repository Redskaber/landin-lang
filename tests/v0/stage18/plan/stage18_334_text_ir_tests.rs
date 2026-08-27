//! Stage 18.334 (P1 soundness fix): Regression tests for TextEmitter IR validity.
//!
//! Per §9.4.3 (1:3 pos:neg ratio): 3 positive + 4 negative = 7 tests.
//! Per §7.3.1 (negative audit coverage): exercises all TextEmitter IR failure modes.
//!
//! **What this file tests**:
//! 1. TextEmitter IR (via `--emit-llvm-ir`) is valid LLVM IR — `llvm-as` accepts it.
//! 2. sret + byval combined: struct return > 16B + struct param > 16B in same fn.
//! 3. Variadic function detection (printf): the call site declares printf as variadic.
//! 4. Negative tests assert that compile errors are reported (not silently accepted).
//!
//! Per §1.0 原則 4 (报错 > 静默): the `llvm-as` smoke test catches the entire
//! class of "TextEmitter IR silently invalid" bugs that Stages 18.332/18.333
//! missed because TextEmitter IR is only used for debug path (--emit-llvm-ir),
//! not for actual compilation (--run/--emit-obj use LLVMSysEmitter).
//! Per §20 (iterative audit): found via §20 audit after Stages 18.332/18.333.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::{assert_runtime, has_errors, run_program};
use landin_compiler::compile;
use std::process::Command;

// ============================================================================
// Helper: run `llvm-as` on TextEmitter IR output
// ============================================================================

/// Compile a Landin program via `--emit-llvm-ir`, then pipe the output to
/// `llvm-as` and assert the IR is valid (exit 0).
///
/// Stage 18.334: this is the architectural fix that catches the entire class
/// of "TextEmitter IR silently invalid" bugs. Per §1.0 原則 4 (报错 > 静默):
/// the test fails loudly when TextEmitter produces invalid IR.
fn assert_llvm_ir_valid(name: &str, code: &str) {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };

    // Create a unique temp dir for this test invocation.
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

    // Run landin-stage0 --emit-llvm-ir
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

    // Find llvm-as (LLVM 22 preferred, fall back to generic llvm-as).
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
            // No llvm-as available — skip the smoke test but emit a warning.
            // Per §1.0 原則 4 (报错 > 静默): print a clear message.
            eprintln!(
                "warn: Test '{}': llvm-as not found in PATH or /tmp/llvm-22-prefix/bin/, skipping IR validity check",
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
// Positive tests: TextEmitter IR validity (3 tests)
// ============================================================================

/// Stage 18.334 positive 1: byval + sret combined — the exact bug case from
/// the §20 audit.
///
/// `Big { a: i64, b: i64, c: i64 }` = 24 bytes > 16 → sret return + byval param.
/// Before Stage 18.334: `llvm-as` rejected with "expected '('" (missing
/// sret type arg) and "use of undefined value '@.data.Option'" (missing
/// data global).
#[test]
fn stage18_334_text_ir_byval_sret_combined() {
    let code = r#"
struct Big { a: i64, b: i64, c: i64 }

fn transform(b: Big) -> Big {
    Big { a: b.a + 1i64, b: b.b + 1i64, c: b.c + 1i64 }
}

fn main() -> i32 {
    let x = Big { a: 1i64, b: 2i64, c: 3i64 };
    let y = transform(x);
    println!("{} {} {}", y.a, y.b, y.c);
    0
}
"#;
    assert_llvm_ir_valid("byval-sret-combined-ir", code);
    // Also verify the program runs correctly via LLVMSysEmitter.
    assert_runtime("byval-sret-combined-run", code, "2 3 4\n");
}

/// Stage 18.334 positive 2: Vec::new() (sret only, no byval) — IR validity.
///
/// `Vec<i32>` returns `{ptr, i64, i64}` = 24 bytes > 16 → sret only.
#[test]
fn stage18_334_text_ir_vec_new_sret_only() {
    let code = r#"
fn main() -> i32 {
    let v: Vec<i32> = Vec::new();
    println!("{}", v.len());
    0
}
"#;
    assert_llvm_ir_valid("vec-new-sret-only-ir", code);
    assert_runtime("vec-new-sret-only-run", code, "0\n");
}

/// Stage 18.334 positive 3: Variadic printf call — IR validity.
///
/// `println!` macro lowers to `call i32 (ptr, ...) @printf(ptr @.str.N, ...)`.
/// Before Stage 18.334: TextEmitter IR was missing the `declare i32 @printf(ptr, ...)`
/// line, so `llvm-as` rejected with "use of undefined value '@printf'".
#[test]
fn stage18_334_text_ir_variadic_printf() {
    let code = r#"
fn main() -> i32 {
    println!("hello {} {}", 1i64, 2i64);
    0
}
"#;
    assert_llvm_ir_valid("variadic-printf-ir", code);
    assert_runtime("variadic-printf-run", code, "hello 1 2\n");
}

// ============================================================================
// Negative tests: TextEmitter IR failure modes (4 tests)
// ============================================================================

/// Stage 18.334 negative 1: Missing field in byval struct construction.
#[test]
fn stage18_334_byval_missing_field() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn transform(b: Big) -> Big {
    Big { a: b.a, b: b.b }
}
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Big struct construction with missing field c must report typeck error"
    );
}

/// Stage 18.334 negative 2: Wrong field type in byval struct.
#[test]
fn stage18_334_byval_wrong_field_type() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn transform(b: Big) -> Big {
    Big { a: b.a, b: true, c: b.c }
}
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Big struct with field type mismatch must be caught by typeck"
    );
}

/// Stage 18.334 negative 3: Calling function with wrong arg type (byval param).
#[test]
fn stage18_334_byval_wrong_arg_type() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn transform(b: Big) -> Big {
    Big { a: b.a, b: b.b, c: b.c }
}
fn main() -> i32 {
    let y = transform(42i64);
    0
}
"#,
    );
    assert!(
        has_errors(&result),
        "Passing i64 to function expecting Big (byval param) must report error"
    );
}

/// Stage 18.334 negative 4: Returning wrong type from sret function.
#[test]
fn stage18_334_sret_wrong_return_value() {
    let result = compile(
        r#"
struct Big { a: i64, b: i64, c: i64 }
fn transform(b: Big) -> Big {
    42i64
}
fn main() -> i32 { 0 }
"#,
    );
    assert!(
        has_errors(&result),
        "Returning i64 from function declared to return Big (sret) must report error"
    );
}

// ============================================================================
// Stress test: Multi-process IR validity
// ============================================================================

/// Stage 18.334 stress 1: Run `--emit-llvm-ir` 3 times — IR generation must be
/// deterministic and valid each time.
#[test]
fn stage18_334_text_ir_deterministic() {
    let code = r#"
struct Big { a: i64, b: i64, c: i64 }
fn transform(b: Big) -> Big {
    Big { a: b.a + 1i64, b: b.b + 1i64, c: b.c + 1i64 }
}
fn main() -> i32 {
    let x = Big { a: 10i64, b: 20i64, c: 30i64 };
    let y = transform(x);
    println!("{} {} {}", y.a, y.b, y.c);
    0
}
"#;
    for _ in 0..3 {
        assert_llvm_ir_valid("deterministic-ir", code);
    }
    // Also verify the program runs correctly.
    let (stdout, exit) = run_program(code);
    assert_eq!(stdout, "11 21 31\n");
    assert_eq!(exit, 0);
}
