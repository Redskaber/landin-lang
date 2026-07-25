//! Stage 11.1 — typecheck expansion (200→400) verification

#![cfg(test)]

use std::path::Path;

/// Verify typecheck expanded to 400+ tests
#[test]
fn test_stage11_1_typecheck_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tc_dir = manifest.join("tests/conformance/01-typecheck");

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
        total >= 400,
        "01-typecheck/ should have 400+ .lin files after Stage 11.1, got {total}"
    );
}

/// Verify Stage 11 directories exist (independent from stage10)
#[test]
fn test_stage11_directories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        manifest.join("tests/v0/stage11/plan").exists(),
        "tests/v0/stage11/plan/ must exist"
    );
    assert!(
        manifest.join("docs/develop/v0/stage-11").exists(),
        "docs/develop/v0/stage-11/ must exist"
    );
    assert!(
        manifest.join("docs/tests/v0/stage11/plan").exists(),
        "docs/tests/v0/stage11/plan/ must exist"
    );
}

/// Verify Stage 11.1 docs created
#[test]
fn test_stage11_1_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.1.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.1.md")
        .exists());
}

/// Verify conformance total ≥ 1339
#[test]
fn test_stage11_1_conformance_total() {
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
        total >= 1339,
        "conformance total should be 1339+, got {total}"
    );
}
