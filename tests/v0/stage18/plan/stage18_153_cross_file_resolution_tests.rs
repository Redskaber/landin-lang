//! Stage 18.153 (TD-SINGLE-FILE Phase 2): Cross-file name resolution e2e tests.
//!
//! Tests that `foo::bar()` in expressions and `use foo::bar;` imports work
//! when `foo` is a module loaded from a separate file by `ModuleLoader`.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §9.4.3: 1:3+ positive:negative ratio (6 positive, 2 negative).
//! Per §16: tests use only public API (`compile_project`).

use landin_compiler::driver::compile_project;
use std::path::PathBuf;

/// Helper: create a temp project dir.
fn make_temp_project(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "landin_stage18_153_{}_{}_{}",
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

// === Positive tests: cross-file function calls ===

/// Stage 18.153 positive 1: `foo::bar()` calls function in loaded module.
#[test]
fn stage18_153_cross_file_fn_call() {
    let dir = make_temp_project("xfn");
    let entry = dir.join("main.lin");
    std::fs::write(&entry, "mod helper; fn main() -> i32 { helper::answer() }").unwrap();
    std::fs::write(dir.join("helper.lin"), "fn answer() -> i32 { 42 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "cross-file fn call should resolve, got errors: {:?}",
        result.errors
    );
    // Verify MIR was produced for both functions.
    assert!(
        result.mirs.len() >= 2,
        "should have MIR for main + helper, got {} bodies",
        result.mirs.len()
    );
    cleanup(&dir);
}

/// Stage 18.153 positive 2: `use foo::bar;` then call `bar()`.
#[test]
fn stage18_153_use_import_from_module() {
    let dir = make_temp_project("useimp");
    let entry = dir.join("main.lin");
    std::fs::write(
        &entry,
        "mod helper; use helper::answer; fn main() -> i32 { answer() }",
    )
    .unwrap();
    std::fs::write(dir.join("helper.lin"), "fn answer() -> i32 { 42 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "use import should resolve, got errors: {:?}",
        result.errors
    );
    cleanup(&dir);
}

/// Stage 18.153 positive 3: cross-file struct usage.
#[test]
fn stage18_153_cross_file_struct() {
    let dir = make_temp_project("xstruct");
    let entry = dir.join("main.lin");
    std::fs::write(
        &entry,
        "mod types; fn main() -> i32 { let p = types::Point { x: 1, y: 2 }; p.x + p.y }",
    )
    .unwrap();
    std::fs::write(dir.join("types.lin"), "struct Point { x: i32, y: i32 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "cross-file struct should resolve, got errors: {:?}",
        result.errors
    );
    cleanup(&dir);
}

/// Stage 18.153 positive 4: `use foo::Point;` then use `Point`.
#[test]
fn stage18_153_use_import_struct() {
    let dir = make_temp_project("usestruct");
    let entry = dir.join("main.lin");
    std::fs::write(
        &entry,
        "mod types; use types::Point; fn main() -> i32 { let p = Point { x: 3, y: 4 }; p.x }",
    )
    .unwrap();
    std::fs::write(dir.join("types.lin"), "struct Point { x: i32, y: i32 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "use import struct should resolve, got errors: {:?}",
        result.errors
    );
    cleanup(&dir);
}

/// Stage 18.153 positive 5: nested module path `outer::inner::func()`.
#[test]
fn stage18_153_nested_module_fn_call() {
    let dir = make_temp_project("nestedfn");
    let entry = dir.join("main.lin");
    std::fs::create_dir_all(dir.join("outer").join("inner")).unwrap();
    std::fs::write(
        &entry,
        "mod outer; fn main() -> i32 { outer::inner::deep() }",
    )
    .unwrap();
    std::fs::write(dir.join("outer").join("mod.lin"), "mod inner;").unwrap();
    std::fs::write(
        dir.join("outer").join("inner").join("mod.lin"),
        "fn deep() -> i32 { 99 }",
    )
    .unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "nested module fn call should resolve, got errors: {:?}",
        result.errors
    );
    cleanup(&dir);
}

/// Stage 18.153 positive 6: inline module with cross-file call still works.
#[test]
fn stage18_153_inline_mod_cross_file() {
    let dir = make_temp_project("inlinecross");
    let entry = dir.join("main.lin");
    std::fs::write(
        &entry,
        "mod helper; mod wrapper { fn call_helper() -> i32 { helper::answer() } } fn main() -> i32 { wrapper::call_helper() }",
    )
    .unwrap();
    std::fs::write(dir.join("helper.lin"), "fn answer() -> i32 { 7 }").unwrap();

    let result = compile_project(&entry);
    assert!(
        !result.has_errors(),
        "inline mod cross-file call should resolve, got errors: {:?}",
        result.errors
    );
    cleanup(&dir);
}

// === Negative tests ===

/// Stage 18.153 negative 1: calling non-existent function in module.
#[test]
fn stage18_153_call_nonexistent_fn() {
    let dir = make_temp_project("nofn");
    let entry = dir.join("main.lin");
    std::fs::write(
        &entry,
        "mod helper; fn main() -> i32 { helper::nonexistent() }",
    )
    .unwrap();
    std::fs::write(dir.join("helper.lin"), "fn answer() -> i32 { 42 }").unwrap();

    let result = compile_project(&entry);
    // Should have resolve errors — `nonexistent` doesn't exist in `helper`.
    // Per §2 原则 4 (报错>静默): unresolved paths must be reported.
    assert!(
        result.has_errors(),
        "should report unresolved path error for helper::nonexistent"
    );
    cleanup(&dir);
}

/// Stage 18.153 negative 2: `use` of non-existent item.
#[test]
fn stage18_153_use_nonexistent_item() {
    let dir = make_temp_project("nouse");
    let entry = dir.join("main.lin");
    std::fs::write(
        &entry,
        "mod helper; use helper::nonexistent; fn main() -> i32 { 0 }",
    )
    .unwrap();
    std::fs::write(dir.join("helper.lin"), "fn answer() -> i32 { 42 }").unwrap();

    let result = compile_project(&entry);
    // Should have resolve errors — `nonexistent` doesn't exist in `helper`.
    assert!(
        result.has_errors(),
        "should report unresolved import error for helper::nonexistent"
    );
    cleanup(&dir);
}
