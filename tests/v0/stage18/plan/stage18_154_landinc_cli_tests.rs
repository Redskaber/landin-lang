//! Stage 18.154 (TD-SINGLE-FILE Phase 3): `landinc` CLI logic tests.
//!
//! Tests the project creation + build orchestration logic used by `landinc`.
//! The CLI binary itself isn't invoked directly (that would require process
//! spawning); instead, we test the underlying library APIs that `landinc`
//! uses: `ProjectManifest`, `compile_project`, `codegen_crate`.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §9.4.3: 1:3+ positive:negative ratio (5 positive, 2 negative).
//! Per §16: tests use only public API.

use landin_compiler::cargo::ProjectManifest;
use landin_compiler::compile_project;
use std::path::PathBuf;

/// Helper: create a temp project dir.
fn make_temp_project(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "landin_stage18_154_{}_{}_{}",
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

/// Stage 18.154 positive 1: `landinc new` creates a valid project skeleton.
#[test]
fn stage18_154_new_creates_valid_skeleton() {
    let dir = make_temp_project("new");
    let project_dir = dir.join("myapp");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();

    // Simulate `landinc new myapp` (binary project).
    let manifest_content = r#"[package]
name = "myapp"
version = "0.1.0"
edition = "v0"
entry_point = "src/main.lin"
target_dir = "target"
"#;
    std::fs::write(project_dir.join("landin.toml"), manifest_content).unwrap();
    std::fs::write(project_dir.join("src").join("main.lin"), "fn main() { }").unwrap();

    // Verify manifest parses.
    let manifest = ProjectManifest::load_manifest(&project_dir.join("landin.toml")).unwrap();
    assert_eq!(manifest.name, "myapp");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.entry_point, project_dir.join("src/main.lin"));

    cleanup(&dir);
}

/// Stage 18.154 positive 2: `landinc build` compiles a new project.
#[test]
fn stage18_154_build_compiles_new_project() {
    let dir = make_temp_project("build");
    let project_dir = dir.join("myapp");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();

    std::fs::write(
        project_dir.join("landin.toml"),
        r#"[package]
name = "myapp"
version = "0.1.0"
entry_point = "src/main.lin"
"#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("main.lin"),
        "fn main() -> i32 { 42 }",
    )
    .unwrap();

    let manifest = ProjectManifest::load_manifest(&project_dir.join("landin.toml")).unwrap();
    let result = compile_project(&manifest.entry_point);
    assert!(
        !result.has_errors(),
        "new project should compile, got: {:?}",
        result.errors
    );

    cleanup(&dir);
}

/// Stage 18.154 positive 3: `landinc build` with multi-file project.
#[test]
fn stage18_154_build_multi_file_project() {
    let dir = make_temp_project("multi");
    let project_dir = dir.join("myapp");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();

    std::fs::write(
        project_dir.join("landin.toml"),
        r#"[package]
name = "myapp"
version = "0.1.0"
entry_point = "src/main.lin"
"#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("main.lin"),
        "mod helper; fn main() -> i32 { helper::answer() }",
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("helper.lin"),
        "fn answer() -> i32 { 42 }",
    )
    .unwrap();

    let manifest = ProjectManifest::load_manifest(&project_dir.join("landin.toml")).unwrap();
    let result = compile_project(&manifest.entry_point);
    assert!(
        !result.has_errors(),
        "multi-file project should compile, got: {:?}",
        result.errors
    );

    cleanup(&dir);
}

/// Stage 18.154 positive 4: `landinc check` type-checks without codegen.
#[test]
fn stage18_154_check_type_checks() {
    let dir = make_temp_project("check");
    let project_dir = dir.join("myapp");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();

    std::fs::write(
        project_dir.join("landin.toml"),
        r#"[package]
name = "myapp"
version = "0.1.0"
entry_point = "src/main.lin"
"#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("src").join("main.lin"),
        "fn main() { let x: i32 = 42; }",
    )
    .unwrap();

    let manifest = ProjectManifest::load_manifest(&project_dir.join("landin.toml")).unwrap();
    let result = compile_project(&manifest.entry_point);
    assert!(!result.has_errors(), "check should pass");
    // `check` doesn't call codegen, but compile_project still produces MIR.
    assert!(!result.mirs.is_empty(), "MIR should be produced");

    cleanup(&dir);
}

/// Stage 18.154 positive 5: `landinc new --lib` creates a library project.
#[test]
fn stage18_154_new_lib_creates_valid_skeleton() {
    let dir = make_temp_project("newlib");
    let project_dir = dir.join("mylib");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();

    // Simulate `landinc new --lib mylib`.
    let manifest_content = r#"[package]
name = "mylib"
version = "0.1.0"
edition = "v0"
entry_point = "src/lib.lin"
target_dir = "target"
"#;
    std::fs::write(project_dir.join("landin.toml"), manifest_content).unwrap();
    std::fs::write(
        project_dir.join("src").join("lib.lin"),
        "pub fn version() -> i32 { 1 }",
    )
    .unwrap();

    let manifest = ProjectManifest::load_manifest(&project_dir.join("landin.toml")).unwrap();
    assert_eq!(manifest.name, "mylib");
    assert_eq!(manifest.entry_point, project_dir.join("src/lib.lin"));

    let result = compile_project(&manifest.entry_point);
    assert!(!result.has_errors(), "library project should compile");

    cleanup(&dir);
}

/// Stage 18.154 negative 1: `landinc build` with missing manifest.
#[test]
fn stage18_154_build_missing_manifest() {
    let dir = make_temp_project("nomani");
    let manifest_path = dir.join("landin.toml");

    // No manifest file — load_manifest should fail.
    let result = ProjectManifest::load_manifest(&manifest_path);
    assert!(result.is_err(), "missing manifest should be an error");

    cleanup(&dir);
}

/// Stage 18.154 negative 2: `landinc build` with missing entry point.
#[test]
fn stage18_154_build_missing_entry_point() {
    let dir = make_temp_project("noentry");
    let project_dir = dir.join("myapp");
    std::fs::create_dir_all(project_dir.join("src")).unwrap();

    std::fs::write(
        project_dir.join("landin.toml"),
        r#"[package]
name = "myapp"
version = "0.1.0"
entry_point = "src/main.lin"
"#,
    )
    .unwrap();
    // Don't create main.lin — entry point doesn't exist.

    let manifest = ProjectManifest::load_manifest(&project_dir.join("landin.toml")).unwrap();
    assert!(
        !manifest.entry_point.exists(),
        "entry point should not exist"
    );

    // compile_project on a non-existent file should return errors.
    let result = compile_project(&manifest.entry_point);
    assert!(
        result.has_errors(),
        "missing entry point should be an error"
    );

    cleanup(&dir);
}
