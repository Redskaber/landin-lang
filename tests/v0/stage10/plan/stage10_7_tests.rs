//! Stage 10.7 — 07-integration conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 07-integration directory has 50+ .lin files
#[test]
fn test_stage10_7_integration_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let int_dir = manifest.join("tests/conformance/07-integration");
    assert!(int_dir.exists(), "07-integration/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&int_dir).expect("read 07-integration/") {
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
        total >= 50,
        "07-integration/ should have 50+ .lin files, got {total}"
    );
}

/// Verify all 3 integration subcategories present
#[test]
fn test_stage10_7_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let int_dir = manifest.join("tests/conformance/07-integration");

    for sub in &["00-multi-crate", "01-cross-module", "02-feature-gate"] {
        assert!(int_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify Stage 10.7 docs created in stage-10 directory
#[test]
fn test_stage10_7_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-10/plan-10.7.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-10/gate-review-10.7.md")
        .exists());
}

/// Verify all 8 conformance categories now exist (00-parse through 07-integration)
#[test]
fn test_stage10_7_all_categories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");

    for cat in &[
        "00-parse",
        "01-typecheck",
        "02-borrowck",
        "03-codegen",
        "04-e2e",
        "05-soundness",
        "06-stdlib",
        "07-integration",
    ] {
        assert!(
            conf_dir.join(cat).exists(),
            "conformance category {cat} must exist"
        );
    }
}

/// Verify conformance total ≥ 1059
#[test]
fn test_stage10_7_conformance_total() {
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
        total >= 1059,
        "conformance total should be 1059+, got {total}"
    );
}
