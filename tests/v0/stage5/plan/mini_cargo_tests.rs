//! Stage 5.24: Mini-cargo MVP tests
//!
//! Tests `ProjectManifest::parse_manifest()`, `build_project()`, and
//! `BuildResult` for project-level build orchestration.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::cargo::{build_project, BuildConfig, ProjectManifest};
use std::io::Write;

/// `parse_manifest` should parse a basic landin.toml.
#[test]
fn test_parse_manifest_basic() {
    let toml = r#"
[package]
name = "my_project"
version = "0.1.0"
edition = "v0"
"#;
    let manifest = ProjectManifest::parse_manifest(toml);
    assert_eq!(manifest.name, "my_project");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.edition, "v0");
}

/// `parse_manifest` should use defaults for missing fields.
#[test]
fn test_parse_manifest_defaults() {
    let toml = "[package]\nname = \"test\"\n";
    let manifest = ProjectManifest::parse_manifest(toml);
    assert_eq!(manifest.name, "test");
    assert_eq!(manifest.edition, "v0");
    assert_eq!(manifest.src_dir, std::path::PathBuf::from("src"));
    assert_eq!(
        manifest.entry_point,
        std::path::PathBuf::from("src/main.lin")
    );
}

/// `parse_manifest` should skip comments and empty lines.
#[test]
fn test_parse_manifest_comments() {
    let toml = r#"
# This is a comment
[package]
# Another comment
name = "commented"
version = "1.0.0"
"#;
    let manifest = ProjectManifest::parse_manifest(toml);
    assert_eq!(manifest.name, "commented");
    assert_eq!(manifest.version, "1.0.0");
}

/// `build_project` should succeed for valid source.
#[test]
fn test_build_project_success() {
    let dir = std::env::temp_dir();
    let entry = dir.join("landin_test_main.lin");
    let mut f = std::fs::File::create(&entry).unwrap();
    writeln!(f, "fn main() {{}}").unwrap();

    let manifest = ProjectManifest {
        name: "test".to_string(),
        version: "0.1.0".to_string(),
        edition: "v0".to_string(),
        src_dir: dir.clone(),
        entry_point: entry.clone(),
        target_dir: dir.join("target"),
    };
    let config = BuildConfig::default();
    let result = build_project(&manifest, &config);
    assert!(result.success, "build should succeed");
    assert_eq!(result.error_count, 0);
    assert_eq!(result.files_compiled, 1);

    let _ = std::fs::remove_file(&entry);
}

/// `build_project` should report errors for invalid source.
#[test]
fn test_build_project_errors() {
    let dir = std::env::temp_dir();
    let entry = dir.join("landin_test_err.lin");
    let mut f = std::fs::File::create(&entry).unwrap();
    writeln!(f, "fn main(").unwrap(); // missing closing paren

    let manifest = ProjectManifest {
        name: "test".to_string(),
        version: "0.1.0".to_string(),
        edition: "v0".to_string(),
        src_dir: dir.clone(),
        entry_point: entry.clone(),
        target_dir: dir.join("target"),
    };
    let config = BuildConfig::default();
    let result = build_project(&manifest, &config);
    assert!(!result.success, "build should fail");
    assert!(result.error_count > 0);

    let _ = std::fs::remove_file(&entry);
}

/// `build_project` should report file-not-found error.
#[test]
fn test_build_project_file_not_found() {
    let manifest = ProjectManifest {
        name: "test".to_string(),
        version: "0.1.0".to_string(),
        edition: "v0".to_string(),
        src_dir: std::path::PathBuf::from("/nonexistent"),
        entry_point: std::path::PathBuf::from("/nonexistent/main.lin"),
        target_dir: std::path::PathBuf::from("/nonexistent/target"),
    };
    let config = BuildConfig::default();
    let result = build_project(&manifest, &config);
    assert!(!result.success);
    assert_eq!(result.files_compiled, 0);
}

/// `build_project` with `emit_llvm` should produce LLVM IR.
#[test]
fn test_build_project_emit_llvm() {
    let dir = std::env::temp_dir();
    let entry = dir.join("landin_test_llvm.lin");
    let mut f = std::fs::File::create(&entry).unwrap();
    writeln!(f, "fn main() {{}}").unwrap();

    let manifest = ProjectManifest {
        name: "test".to_string(),
        version: "0.1.0".to_string(),
        edition: "v0".to_string(),
        src_dir: dir.clone(),
        entry_point: entry.clone(),
        target_dir: dir.join("target"),
    };
    let config = BuildConfig {
        emit_llvm: true,
        ..Default::default()
    };
    let result = build_project(&manifest, &config);
    assert!(result.success);
    assert!(result.llvm_ir.is_some(), "should have LLVM IR");
    let ir = result.llvm_ir.unwrap();
    assert!(
        ir.contains("target triple"),
        "IR should contain target triple"
    );

    let _ = std::fs::remove_file(&entry);
}

/// `ProjectManifest::default()` should have sensible defaults.
#[test]
fn test_project_manifest_default() {
    let m = ProjectManifest::default();
    assert_eq!(m.edition, "v0");
    assert_eq!(m.src_dir, std::path::PathBuf::from("src"));
    assert_eq!(m.entry_point, std::path::PathBuf::from("src/main.lin"));
    assert_eq!(m.target_dir, std::path::PathBuf::from("target"));
}
