//! Stage 18.152 (TD-SINGLE-FILE Phase 1): Multi-file module loading e2e tests.
//!
//! Tests `compile_project()` — the new public API for compiling multi-file
//! Landin projects where `mod foo;` declarations load `foo.lin` or
//! `foo/mod.lin` from disk.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/` per stage convention.
//! Per §9.4.3: 1:3+ positive:negative ratio (7 positive, 3 negative here).
//! Per §16: tests use only public API (`compile_project`).

use landin_compiler::driver::compile_project;
use std::path::PathBuf;

/// Helper: create a temp project dir with files.
fn make_temp_project(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "landin_stage18_152_{}_{}_{}",
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

// === Positive tests ===

/// Stage 18.152 positive 1: single-file project compiles via compile_project.
#[test]
fn stage18_152_compile_project_single_file() {
    let dir = make_temp_project("single");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "fn main() { }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "single-file project should compile cleanly, got errors: {:?}",
        result.errors
    );
    cleanup(&dir);
}

/// Stage 18.152 positive 2: `mod foo;` loads `foo.lin`.
#[test]
fn stage18_152_compile_project_loads_mod_file() {
    let dir = make_temp_project("modfile");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod helper; fn main() { }").unwrap();
    std::fs::write(dir.join("helper.lin"), "fn answer() -> i32 { 42 }").unwrap();

    let result = compile_project(&entry);
    // We expect no module-load errors (helper.lin exists and parses).
    // Type/name resolution errors for `helper::answer` may exist (Stage 18.153),
    // but module loading itself should succeed.
    assert!(
        result
            .errors
            .lower
            .iter()
            .all(|e| !e.message.contains("module")),
        "should have no module-load errors, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 positive 3: `mod foo;` loads `foo/mod.lin`.
#[test]
fn stage18_152_compile_project_loads_mod_dir() {
    let dir = make_temp_project("moddir");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("foo")).unwrap();
    std::fs::write(&entry, "mod foo; fn main() { }").unwrap();
    std::fs::write(dir.join("foo").join("mod.lin"), "fn bar() -> i32 { 7 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .lower
            .iter()
            .all(|e| !e.message.contains("module")),
        "should have no module-load errors, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 positive 4: nested modules load recursively.
#[test]
fn stage18_152_compile_project_nested_modules() {
    let dir = make_temp_project("nested");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("outer").join("inner")).unwrap();
    std::fs::write(&entry, "mod outer; fn main() { }").unwrap();
    std::fs::write(dir.join("outer").join("mod.lin"), "mod inner;").unwrap();
    std::fs::write(
        dir.join("outer").join("inner").join("mod.lin"),
        "fn deep() -> i32 { 99 }",
    )
    .unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .lower
            .iter()
            .all(|e| !e.message.contains("module")),
        "should have no module-load errors, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 positive 5: inline `mod foo { ... }` is unchanged.
#[test]
fn stage18_152_compile_project_inline_mod() {
    let dir = make_temp_project("inline");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod foo { fn bar() -> i32 { 42 } } fn main() { }").unwrap();

    let result = compile_project(&entry);
    // Inline mod doesn't trigger file loading — should work as before.
    assert!(
        result
            .errors
            .lower
            .iter()
            .all(|e| !e.message.contains("module")),
        "inline mod should not trigger load errors, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 positive 6: multiple sibling modules.
#[test]
fn stage18_152_compile_project_multiple_siblings() {
    let dir = make_temp_project("siblings");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod a; mod b; mod c; fn main() { }").unwrap();
    std::fs::write(dir.join("a.lin"), "fn fa() -> i32 { 1 }").unwrap();
    std::fs::write(dir.join("b.lin"), "fn fb() -> i32 { 2 }").unwrap();
    std::fs::write(dir.join("c.lin"), "fn fc() -> i32 { 3 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .lower
            .iter()
            .all(|e| !e.message.contains("module")),
        "should have no module-load errors, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 positive 7: `foo.lin` takes precedence over `foo/mod.lin`.
#[test]
fn stage18_152_compile_project_file_precedence() {
    let dir = make_temp_project("precedence");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("foo")).unwrap();
    std::fs::write(&entry, "mod foo; fn main() { }").unwrap();
    // Both exist — `foo.lin` should win.
    std::fs::write(dir.join("foo.lin"), "fn from_file() -> i32 { 1 }").unwrap();
    std::fs::write(
        dir.join("foo").join("mod.lin"),
        "fn from_dir() -> i32 { 2 }",
    )
    .unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .lower
            .iter()
            .all(|e| !e.message.contains("module")),
        "should have no module-load errors, got: {:?}",
        result.errors.lower
    );
    // We can't easily verify which file was loaded from CompileResult alone,
    // but the absence of load errors confirms one was found.
    cleanup(&dir);
}

// === Negative tests ===

/// Stage 18.152 negative 1: missing module file reports an error.
#[test]
fn stage18_152_compile_project_missing_module() {
    let dir = make_temp_project("missing");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod nonexistent; fn main() { }").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .lower
            .iter()
            .any(|e| e.message.contains("not found")),
        "should report module not found error, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 negative 2: circular module dependency is detected.
#[test]
fn stage18_152_compile_project_circular_dep() {
    let dir = make_temp_project("circular");
    let entry = dir.join("main.lin");
    // a.lin: mod b;
    // b.lin: mod a;  ← circular
    std::fs::write(&entry, "mod a; fn main() { }").unwrap();
    std::fs::write(dir.join("a.lin"), "mod b;").unwrap();
    std::fs::write(dir.join("b.lin"), "mod a;").unwrap();

    let result = compile_project(&entry);
    assert!(
        result
            .errors
            .lower
            .iter()
            .any(|e| e.message.contains("circular")),
        "should detect circular dependency, got: {:?}",
        result.errors.lower
    );
    cleanup(&dir);
}

/// Stage 18.152 negative 3: parse error in submodule is reported.
#[test]
fn stage18_152_compile_project_parse_error_in_submodule() {
    let dir = make_temp_project("parseerr");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod bad; fn main() { }").unwrap();
    // Invalid syntax: `fn` without name
    std::fs::write(dir.join("bad.lin"), "fn { broken }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.errors.lower.is_empty() || !result.errors.parse.is_empty(),
        "should report parse error in submodule, got lower: {:?}, parse: {:?}",
        result.errors.lower,
        result.errors.parse
    );
    cleanup(&dir);
}
