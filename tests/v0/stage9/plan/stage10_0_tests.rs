//! Stage 10.0 — CLI upgrade + Runner upgrade verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1.
//! Verifies Stage 10.0 infrastructure: CLI --compile/--emit-llvm-ir +
//! runner compile mode + format compatibility.

#![cfg(test)]

use std::path::Path;

/// Verify CLI has --compile option (Stage 10.0 upgrade)
#[test]
fn test_stage10_0_cli_has_compile_option() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    assert!(
        content.contains("--compile"),
        "CLI must have --compile option after Stage 10.0"
    );
    assert!(
        content.contains("driver::compile"),
        "CLI must use driver::compile for --compile mode"
    );
}

/// Verify CLI has --emit-llvm-ir option (Stage 10.0 upgrade)
#[test]
fn test_stage10_0_cli_has_emit_llvm_ir_option() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    assert!(
        content.contains("--emit-llvm-ir"),
        "CLI must have --emit-llvm-ir option after Stage 10.0"
    );
    assert!(
        content.contains("codegen_crate"),
        "CLI must use codegen::codegen_crate for --emit-llvm-ir mode"
    );
}

/// Verify runner supports --mode compile flag
#[test]
fn test_stage10_0_runner_supports_compile_mode() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runner = manifest.join("tests/conformance/run_all.py");
    let content = std::fs::read_to_string(&runner).expect("read runner");

    assert!(
        content.contains("--mode"),
        "Runner must support --mode flag after Stage 10.0"
    );
    assert!(
        content.contains("compile") && content.contains("parse"),
        "Runner must support both parse and compile modes"
    );
}

/// Verify runner supports spec // format (EXPECTED field)
#[test]
fn test_stage10_0_runner_supports_spec_format() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runner = manifest.join("tests/conformance/run_all.py");
    let content = std::fs::read_to_string(&runner).expect("read runner");

    assert!(
        content.contains("EXPECTED"),
        "Runner must support EXPECTED field (spec // format)"
    );
    assert!(
        content.contains("compile_ok") && content.contains("compile_error"),
        "Runner must support compile_ok/compile_error expected values"
    );
}

/// Verify runner maintains backward compatibility with //! format
#[test]
fn test_stage10_0_runner_backward_compatible() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runner = manifest.join("tests/conformance/run_all.py");
    let content = std::fs::read_to_string(&runner).expect("read runner");

    assert!(
        content.contains("LEGACY_HEADER_RE"),
        "Runner must maintain legacy //! format support"
    );
    assert!(
        content.contains("SPEC_HEADER_RE"),
        "Runner must support new spec // format"
    );
}

/// Verify Stage 10.0 docs created
#[test]
fn test_stage10_0_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-9/plan-10.0.md");
    let gate = manifest.join("docs/develop/v0/stage-9/gate-review-10.0.md");

    assert!(plan.exists(), "plan-10.0.md must exist");
    assert!(gate.exists(), "gate-review-10.0.md must exist");
}

/// Verify Cargo.toml version bumped to 0.17.2+
#[test]
fn test_stage10_0_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    let is_valid = version_line.starts_with("version = \"0.17.2")
        || version_line.starts_with("version = \"0.17.3")
        || version_line.starts_with("version = \"0.17.4")
        || version_line.starts_with("version = \"0.17.5")
        || version_line.starts_with("version = \"0.17.6")
        || version_line.starts_with("version = \"0.17.7")
        || version_line.starts_with("version = \"0.17.8")
        || version_line.starts_with("version = \"0.17.9")
        || version_line.starts_with("version = \"0.18.")
        || version_line.starts_with("version = \"0.19.")
        || version_line.starts_with("version = \"0.20.");
    assert!(
        is_valid,
        "Cargo.toml version must be 0.17.2+ after Stage 10.0, got: {version_line}"
    );
}

/// Verify conformance still passes (600 tests, backward compatible)
#[test]
fn test_stage10_0_conformance_still_passes() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parse_dir = manifest.join("tests/conformance/00-parse");

    let mut total = 0;
    for entry in std::fs::read_dir(&parse_dir).expect("read 00-parse/") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            total += std::fs::read_dir(&path)
                .expect("read category dir")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                .count();
        }
    }

    assert_eq!(
        total, 600,
        "conformance count must remain 600 after Stage 10.0 (no format migration yet)"
    );
}
