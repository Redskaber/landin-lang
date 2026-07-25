//! Stage 10.8 — §25 deep review + typecheck expansion verification

#![cfg(test)]

use std::path::Path;

/// Verify §25 deep review document exists
#[test]
fn test_stage10_8_deep_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        manifest
            .join("docs/develop/v0/stage-10/deep-review-stage10-r205.md")
            .exists(),
        "deep-review-stage10-r205.md must exist"
    );
}

/// Verify typecheck expanded to 200+ tests
#[test]
fn test_stage10_8_typecheck_expanded() {
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
        total >= 200,
        "01-typecheck/ should have 200+ .lin files after Stage 10.8, got {total}"
    );
}

/// Verify Stage 10.8 docs created
#[test]
fn test_stage10_8_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-10/plan-10.8.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-10/gate-review-10.8.md")
        .exists());
}

/// Verify conformance total ≥ 1139
#[test]
fn test_stage10_8_conformance_total() {
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
        total >= 1139,
        "conformance total should be 1139+, got {total}"
    );
}
