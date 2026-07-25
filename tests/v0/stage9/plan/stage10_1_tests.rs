//! Stage 10.1 — 01-typecheck conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 01-typecheck directory has 120+ .lin files
#[test]
fn test_stage10_1_typecheck_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tc_dir = manifest.join("tests/conformance/01-typecheck");
    assert!(tc_dir.exists(), "01-typecheck/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&tc_dir).expect("read 01-typecheck/") {
        let entry = entry.expect("dir entry");
        if entry.path().is_dir() {
            total += std::fs::read_dir(entry.path())
                .expect("read subcategory")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                .count();
        }
    }
    assert!(
        total >= 120,
        "01-typecheck/ should have 120+ .lin files, got {total}"
    );
}

/// Verify all 6 typecheck subcategories present
#[test]
fn test_stage10_1_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tc_dir = manifest.join("tests/conformance/01-typecheck");

    for sub in &[
        "00-basic-inference",
        "01-trait-resolution",
        "02-generics",
        "03-closures",
        "04-lifetimes",
        "99-error-cases",
    ] {
        assert!(tc_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify tests use spec // format (EXPECTED field)
#[test]
fn test_stage10_1_uses_spec_format() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join("tests/conformance/01-typecheck/00-basic-inference/001-let-i32.lin");
    let content = std::fs::read_to_string(&sample).expect("read sample");
    assert!(
        content.contains("// EXPECTED:"),
        "typecheck tests must use spec // format"
    );
}

/// Verify runner auto-detect mode
#[test]
fn test_stage10_1_runner_auto_mode() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runner = manifest.join("tests/conformance/run_all.py");
    let content = std::fs::read_to_string(&runner).expect("read runner");
    assert!(content.contains("auto"), "runner must support auto mode");
}

/// Verify Stage 10.1 docs created
#[test]
fn test_stage10_1_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-9/plan-10.1.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-9/gate-review-10.1.md")
        .exists());
}

/// Verify conformance total ≥ 720
#[test]
fn test_stage10_1_conformance_total() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");
    let mut total = 0;
    for entry in std::fs::read_dir(&conf_dir).expect("read conformance/") {
        let entry = entry.expect("dir entry");
        if entry.path().is_dir() {
            for sub in std::fs::read_dir(entry.path()).expect("read category") {
                let sub = sub.expect("sub entry");
                if sub.path().is_dir() {
                    total += std::fs::read_dir(sub.path())
                        .expect("read subcategory")
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                        .count();
                }
            }
        }
    }
    assert!(
        total >= 720,
        "conformance total should be 720+, got {total}"
    );
}
