//! Stage 18.155 (TD-SINGLE-FILE Phase 4): mini-cargo deficiency fixes.
//!
//! Tests the fixes for the 简写/缺陷 recorded in Stage 18.154:
//! - 缺陷3 fix: project name validation (`is_valid_ident`)
//! - 简写1 fix: `compile_project_opt(entry_path, optimize)` API
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §9.4.3: 1:3+ positive:negative ratio (3 positive, 2 negative).
//! Per §16: tests use only public API.

use landin_compiler::compile_project_opt;
use landin_compiler::lexer::is_valid_ident;
use std::path::PathBuf;

/// Helper: create a temp project dir.
fn make_temp_project(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "landin_stage18_155_{}_{}_{}",
        suffix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
}

// === Tests for `is_valid_ident` (缺陷3 fix) ===

/// Stage 18.155 positive 1: valid project names accepted.
#[test]
fn stage18_155_valid_project_names() {
    assert!(is_valid_ident("myapp"));
    assert!(is_valid_ident("my_app"));
    assert!(is_valid_ident("app2"));
    assert!(is_valid_ident("_internal"));
}

/// Stage 18.155 negative 1: invalid project names rejected.
#[test]
fn stage18_155_invalid_project_names() {
    assert!(!is_valid_ident(""));
    assert!(!is_valid_ident("2app")); // starts with digit
    assert!(!is_valid_ident("my-app")); // hyphen
    assert!(!is_valid_ident("my.app")); // dot
    assert!(!is_valid_ident("fn")); // keyword
    assert!(!is_valid_ident("struct")); // keyword
}

// === Tests for `compile_project_opt` (简写1 fix) ===

/// Stage 18.155 positive 2: `compile_project_opt(path, true)` runs MIR opt.
#[test]
fn stage18_155_compile_project_opt_with_optimization() {
    let dir = make_temp_project("opt");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "fn main() -> i32 { 42 }").unwrap();

    let result = compile_project_opt(&entry, true);
    assert!(
        !result.has_errors(),
        "optimized compile should succeed, got: {:?}",
        result.errors
    );
    assert!(!result.mirs.is_empty(), "MIR should be produced");
    cleanup(&dir);
}

/// Stage 18.155 positive 3: `compile_project_opt(path, false)` skips MIR opt.
#[test]
fn stage18_155_compile_project_opt_without_optimization() {
    let dir = make_temp_project("noopt");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "fn main() -> i32 { 42 }").unwrap();

    let result = compile_project_opt(&entry, false);
    assert!(
        !result.has_errors(),
        "unoptimized compile should succeed, got: {:?}",
        result.errors
    );
    assert!(!result.mirs.is_empty(), "MIR should still be produced");
    cleanup(&dir);
}

/// Stage 18.155 negative 2: `compile_project_opt` on missing file reports error.
#[test]
fn stage18_155_compile_project_opt_missing_file() {
    let dir = make_temp_project("missing");
    let entry = dir.join("nonexistent.lin");

    let result = compile_project_opt(&entry, true);
    assert!(
        result.has_errors(),
        "missing file should produce errors, got: {:?}",
        result.errors
    );
    cleanup(&dir);
}
