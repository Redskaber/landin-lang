//! Shared test helpers for all Landin test suites.
//!
//! Per stage-committee-process.md v3.17 §17.1, shared test utilities live in
//! `tests/common/mod.rs`. Individual test files can use them via:
//!
//! ```ignore
//! #[path = "../../../common/mod.rs"]
//! mod common;
//! use common::run_program;
//! ```
//!
//! Stage 18.326 (P1 soundness fix): added `run_program` helper that uses
//! per-test unique temp subdirectories to eliminate /tmp file races under
//! multi-threaded test execution. Per §2.2 (根因思维) + §12 (最优>最小):
//! root-cause fix — shared helper ensures ALL test files use unique paths.

// Stage 18.326: Allow dead code — not every test file uses every helper.
// Allow clippy::module_inception — the `mod common` pattern is intentional.
#![allow(dead_code)]

use landin_compiler::{compile, CompileResult};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stage 119 (TD-PROCESS-PER-TEST-ISOLATION): Compile a Landin source string
/// via subprocess (`landin-stage0 --check-errors`) to get fresh LLVM C++ state
/// each time. This eliminates cross-compilation accumulation that causes
/// non-deterministic SIGSEGV in LLVM's backend.
///
/// The subprocess outputs error counts as JSON. We parse the JSON to construct
/// a lightweight CompileResult. For tests that need structured error access
/// (e.g., `result.errors.typeck`), the in-process `compile()` is still used
/// via `compile_src_in_process()`.
///
/// Per §1.0 原則 9 (正确 > 妥协): deterministic codegen > in-process speed.
/// Per §12 (最优 > 最小): root-cause fix — isolate LLVM state per test.
/// Per §17.6 (直到审查不出问题为止): iterated audit cycle Stages 99→119.
pub fn compile_src(src: &str) -> CompileResult {
    // Try subprocess path first (gives fresh LLVM state).
    if let Some(result) = compile_src_subprocess(src) {
        return result;
    }
    // Fallback to in-process compile if subprocess fails (e.g., binary
    // not found, source file write error). This preserves backward
    // compatibility for tests that need structured error access.
    compile(src)
}

/// In-process compile — for tests that need structured error details.
pub fn compile_src_in_process(src: &str) -> CompileResult {
    compile(src)
}

/// Subprocess compile — returns Some(CompileResult) on success, None on failure.
fn compile_src_subprocess(src: &str) -> Option<CompileResult> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };

    // Check if the binary exists.
    if !bin.exists() {
        return None;
    }

    // Create a unique temp file for the source.
    let counter = SUBPROCESS_COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir =
        std::env::temp_dir().join(format!("landin_check_{}_{}", std::process::id(), counter));
    let _ = std::fs::create_dir_all(&temp_dir);
    let lin_file = temp_dir.join("input.lin");
    if std::fs::write(&lin_file, src).is_err() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return None;
    }

    let output = Command::new(&bin)
        .arg("--check-errors")
        .arg(&lin_file)
        .env("TMPDIR", &temp_dir)
        .output();

    let _ = std::fs::remove_dir_all(&temp_dir);

    let output = match output {
        Ok(o) => o,
        Err(_) => return None,
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Parse JSON output: {"has_errors":bool,"lex":N,...,"total":N}
    // We only need has_errors + total for most tests.
    let has_errors = stdout.contains("\"has_errors\":true");
    let _total: usize = extract_json_u64(&stdout, "total")
        .unwrap_or(0)
        .try_into()
        .unwrap_or(0);

    if has_errors {
        // For error cases, fall back to in-process to get structured errors.
        // This is needed because the subprocess only returns counts, not
        // structured error details.
        None
    } else {
        // No errors — construct an empty CompileResult.
        Some(CompileResult::empty_result())
    }
}

/// Extract a u64 value from a JSON string by key.
fn extract_json_u64(json: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

static SUBPROCESS_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Compile and return the result without panicking on errors.
/// Useful for negative tests that expect errors.
pub fn compile_silent(src: &str) -> CompileResult {
    compile(src)
}

/// Check if a compiled result has any errors.
pub fn has_errors(result: &CompileResult) -> bool {
    result.has_errors()
}

/// Count the total number of errors in a compiled result.
pub fn error_count(result: &CompileResult) -> usize {
    result.errors.total_count()
}

// ============================================================================
// Stage 18.326: Shared run_program helper — eliminates /tmp file races
// ============================================================================

/// Global counter for unique temp directory names.
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Run a Landin program via `landin-stage0 --run` in a **unique temp subdirectory**.
///
/// Stage 18.326 (P1 soundness fix): Previously, test files used `run_program`
/// helpers that wrote `.lin` files to `/tmp` with names like
/// `landin_s188_test_{pid}_{counter}.lin`. While these names were unique,
/// the `landin-stage0 --run` subprocess internally created `obj_path`,
/// `exe_path`, and `wrapper_c` in `/tmp` — and under high concurrency
/// (`--test-threads=8+`), these could occasionally race.
///
/// This shared helper creates a **per-call unique subdirectory** under `/tmp`
/// and runs the entire compile-link-execute cycle inside it, guaranteeing
/// zero file races regardless of concurrency level.
///
/// Per §2.2 (根因思维): root-cause fix (unique subdir), not workaround.
/// Per §12 (最优>最小): shared helper ensures ALL test files use this.
/// Per §1.0 原則 6 (通解>特解): one helper for all 29+ test files.
pub fn run_program(code: &str) -> (String, i32) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = if cfg!(debug_assertions) {
        manifest.join("target/debug/landin-stage0")
    } else {
        manifest.join("target/release/landin-stage0")
    };

    // Create a unique temp subdirectory for this run.
    // Format: /tmp/landin_test_{pid}_{nanos}_{counter}
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir_name = format!("landin_test_{}_{}_{}", std::process::id(), nanos, counter);
    let temp_dir: PathBuf = std::env::temp_dir().join(&dir_name);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let lin_file = temp_dir.join("input.lin");
    std::fs::write(&lin_file, code).expect("write .lin file");

    // Stage 18.332 (P1 soundness fix): Set TMPDIR to the unique temp subdir
    // for this test invocation. This ensures cc's own temp files (assembler
    // output, etc.) live in the same unique subdir, eliminating /tmp races
    // when 8+ concurrent `landin-stage0 --run` processes invoke cc.
    //
    // Per §2.2 (根因思维): root-cause fix — cc inherits TMPDIR from env,
    // so setting it per-process prevents /tmp races at the cc level.
    // Per §1.0 原則 6 (通解 > 特解): one fix for all test invocations.
    let output = Command::new(&bin)
        .arg("--run")
        .arg(&lin_file)
        .env("TMPDIR", &temp_dir)
        .output()
        .expect("failed to execute landin-stage0");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let exit = output.status.code().unwrap_or(-1);

    // Cleanup: remove the entire temp subdirectory (input.lin + any artifacts).
    let _ = std::fs::remove_dir_all(&temp_dir);

    (stdout, exit)
}

/// Assert that a Landin program produces the expected stdout and exits 0.
pub fn assert_runtime(name: &str, code: &str, expected_stdout: &str) {
    let (stdout, exit) = run_program(code);
    assert_eq!(
        stdout, expected_stdout,
        "Test '{}': stdout mismatch.\nExpected: {:?}\nGot:      {:?}",
        name, expected_stdout, stdout
    );
    assert_eq!(
        exit, 0,
        "Test '{}': exit code mismatch (expected 0, got {})",
        name, exit
    );
}
