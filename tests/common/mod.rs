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

/// Compile a Landin source string and return the CompileResult.
pub fn compile_src(src: &str) -> CompileResult {
    compile(src)
}

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

    let output = Command::new(&bin)
        .arg("--run")
        .arg(&lin_file)
        .output()
        .expect("failed to execute landin-stage0");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Cleanup: remove the entire temp subdirectory (input.lin + any artifacts).
    let _ = std::fs::remove_dir_all(&temp_dir);

    (stdout, output.status.code().unwrap_or(-1))
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
