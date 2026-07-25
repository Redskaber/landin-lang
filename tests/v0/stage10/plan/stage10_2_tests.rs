//! Stage 10.2 — 02-borrowck conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 02-borrowck directory has 80+ .lin files
#[test]
fn test_stage10_2_borrowck_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bk_dir = manifest.join("tests/conformance/02-borrowck");
    assert!(bk_dir.exists(), "02-borrowck/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&bk_dir).expect("read 02-borrowck/") {
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
        total >= 80,
        "02-borrowck/ should have 80+ .lin files, got {total}"
    );
}

/// Verify all 5 borrowck subcategories present
#[test]
fn test_stage10_2_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bk_dir = manifest.join("tests/conformance/02-borrowck");

    for sub in &[
        "00-nll-basic",
        "01-nll-advanced",
        "02-move-semantics",
        "03-closure-capture",
        "99-error-cases",
    ] {
        assert!(bk_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify Stage 10.2 docs created
#[test]
fn test_stage10_2_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-10/plan-10.2.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-10/gate-review-10.2.md")
        .exists());
}

/// Verify conformance total ≥ 800
#[test]
fn test_stage10_2_conformance_total() {
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
        total >= 800,
        "conformance total should be 800+, got {total}"
    );
}
