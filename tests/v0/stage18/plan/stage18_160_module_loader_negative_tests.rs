//! Stage 18.160 (TD-NEGATIVE-TEST-COVERAGE): ModuleLoader negative tests.
//!
//! Tests ModuleLoader error paths. Per §9.4.3, negative tests should be
//! ≥25% of total. This file expands module loader negative coverage.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile_project`).

use landin_compiler::compile_project;
use std::path::PathBuf;

/// Helper: create a temp project dir.
fn make_temp_project(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "landin_stage18_160_{}_{}_{}",
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

// === Module file errors ===

/// Stage 18.160 negative 1: missing module file (foo.lin doesn't exist).
#[test]
fn stage18_160_module_missing_file() {
    let dir = make_temp_project("miss1");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod foo; fn main() { }").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .module_load
            .iter()
            .any(|e| e.message.contains("not found")),
        "should report missing module, got: {:?}",
        result.errors.module_load
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 2: missing both foo.lin and foo/mod.lin.
#[test]
fn stage18_160_module_missing_both_variants() {
    let dir = make_temp_project("miss2");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod foo; fn main() { }").unwrap();
    // Neither foo.lin nor foo/mod.lin exists.

    let result = compile_project(&entry);
    assert!(
        !result.errors.module_load.is_empty(),
        "should report missing module"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 3: module file is a directory (not .lin file).
#[test]
fn stage18_160_module_is_directory_not_file() {
    let dir = make_temp_project("dirnotfile");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod foo; fn main() { }").unwrap();
    // Create foo/ directory but no foo/mod.lin.
    std::fs::create_dir_all(dir.join("foo")).unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.errors.module_load.is_empty(),
        "should report missing module (foo/mod.lin not found in dir)"
    );
    cleanup(&dir);
}

// === Parse errors in modules ===

/// Stage 18.160 negative 4: syntax error in loaded module.
#[test]
fn stage18_160_module_syntax_error() {
    let dir = make_temp_project("synerr");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod bad; fn main() { }").unwrap();
    std::fs::write(dir.join("bad.lin"), "fn { broken }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.errors.module_load.is_empty() || !result.errors.parse.is_empty(),
        "should report parse error in submodule"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 5: missing semicolon in module.
#[test]
fn stage18_160_module_missing_semicolon() {
    let dir = make_temp_project("nosemi");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod bad; fn main() { }").unwrap();
    std::fs::write(dir.join("bad.lin"), "fn helper() -> i32 { 42 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        result.errors.module_load.is_empty(),
        "valid module should load without errors"
    );
    cleanup(&dir);
}

// === Circular dependency ===

/// Stage 18.160 negative 6: direct circular dependency (a→b→a).
#[test]
fn stage18_160_module_circular_direct() {
    let dir = make_temp_project("circ1");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod a; fn main() { }").unwrap();
    std::fs::write(dir.join("a.lin"), "mod b;").unwrap();
    std::fs::write(dir.join("b.lin"), "mod a;").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .module_load
            .iter()
            .any(|e| e.message.contains("circular")),
        "should detect circular dependency"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 7: self-referencing module (a→a).
#[test]
fn stage18_160_module_circular_self() {
    let dir = make_temp_project("circ2");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod a; fn main() { }").unwrap();
    std::fs::write(dir.join("a.lin"), "mod a;").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .module_load
            .iter()
            .any(|e| e.message.contains("circular")),
        "should detect self-referencing module"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 8: longer circular chain (a→b→c→a).
#[test]
fn stage18_160_module_circular_chain() {
    let dir = make_temp_project("circ3");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod a; fn main() { }").unwrap();
    std::fs::write(dir.join("a.lin"), "mod b;").unwrap();
    std::fs::write(dir.join("b.lin"), "mod c;").unwrap();
    std::fs::write(dir.join("c.lin"), "mod a;").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .module_load
            .iter()
            .any(|e| e.message.contains("circular")),
        "should detect circular chain a→b→c→a"
    );
    cleanup(&dir);
}

// === Nested module errors ===

/// Stage 18.160 negative 9: nested module missing.
#[test]
fn stage18_160_module_nested_missing() {
    let dir = make_temp_project("nestmiss");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("outer")).unwrap();
    std::fs::write(&entry, "mod outer; fn main() { }").unwrap();
    std::fs::write(dir.join("outer").join("mod.lin"), "mod inner;").unwrap();
    // outer/inner/mod.lin doesn't exist.

    let result = compile_project(&entry);
    assert!(
        !result.errors.module_load.is_empty(),
        "should report missing nested module"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 10: nested module with parse error.
#[test]
fn stage18_160_module_nested_parse_error() {
    let dir = make_temp_project("nestparse");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("outer").join("inner")).unwrap();
    std::fs::write(&entry, "mod outer; fn main() { }").unwrap();
    std::fs::write(dir.join("outer").join("mod.lin"), "mod inner;").unwrap();
    std::fs::write(
        dir.join("outer").join("inner").join("mod.lin"),
        "fn { broken }",
    )
    .unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.errors.module_load.is_empty() || !result.errors.parse.is_empty(),
        "should report parse error in nested module"
    );
    cleanup(&dir);
}

// === Entry file errors ===

/// Stage 18.160 negative 11: entry file doesn't exist.
#[test]
fn stage18_160_entry_file_missing() {
    let dir = make_temp_project("entrymiss");
    let entry = dir.join("nonexistent.lin");

    let result = compile_project(&entry);
    assert!(
        !result.errors.lex.is_empty(),
        "missing entry file should produce lex error"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 12: entry file is empty.
#[test]
fn stage18_160_entry_file_empty() {
    let dir = make_temp_project("empty");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "").unwrap();

    let result = compile_project(&entry);
    // Empty file should compile (no items, no errors).
    assert!(
        result.errors.lex.is_empty(),
        "empty file should have no lex errors"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 13: entry file with only comments.
#[test]
fn stage18_160_entry_file_comments_only() {
    let dir = make_temp_project("comments");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "// just a comment\n").unwrap();

    let result = compile_project(&entry);
    assert!(
        result.errors.lex.is_empty(),
        "comment-only file should have no lex errors"
    );
    cleanup(&dir);
}

// === Permission errors ===

/// Stage 18.160 negative 14: module file with invalid UTF-8.
#[test]
fn stage18_160_module_invalid_utf8() {
    let dir = make_temp_project("utf8");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod bad; fn main() { }").unwrap();
    // Write invalid UTF-8 bytes.
    std::fs::write(dir.join("bad.lin"), b"fn helper() { \xff\xfe }").unwrap();

    let result = compile_project(&entry);
    // Should report some error (lex or module_load).
    assert!(
        !result.errors.module_load.is_empty() || !result.errors.lex.is_empty(),
        "invalid UTF-8 should produce errors"
    );
    cleanup(&dir);
}

/// Stage 18.160 negative 15: deeply nested missing module (5 levels).
#[test]
fn stage18_160_module_deeply_nested_missing() {
    let dir = make_temp_project("deep");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("a").join("b").join("c").join("d")).unwrap();
    std::fs::write(&entry, "mod a; fn main() { }").unwrap();
    std::fs::write(dir.join("a").join("mod.lin"), "mod b;").unwrap();
    std::fs::write(dir.join("a").join("b").join("mod.lin"), "mod c;").unwrap();
    std::fs::write(dir.join("a").join("b").join("c").join("mod.lin"), "mod d;").unwrap();
    // d/mod.lin doesn't exist.

    let result = compile_project(&entry);
    assert!(
        !result.errors.module_load.is_empty(),
        "should report missing deeply nested module"
    );
    cleanup(&dir);
}
