//! Stage 10.6 — 06-stdlib conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 06-stdlib directory has 50+ .lin files
#[test]
fn test_stage10_6_stdlib_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let st_dir = manifest.join("tests/conformance/06-stdlib");
    assert!(st_dir.exists(), "06-stdlib/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&st_dir).expect("read 06-stdlib/") {
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
        "06-stdlib/ should have 50+ .lin files, got {total}"
    );
}

/// Verify all 3 stdlib subcategories present
#[test]
fn test_stage10_6_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let st_dir = manifest.join("tests/conformance/06-stdlib");

    for sub in &["00-core", "01-alloc", "02-std"] {
        assert!(st_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify Stage 10.6 docs created in stage-10 directory
#[test]
fn test_stage10_6_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-10/plan-10.6.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-10/gate-review-10.6.md")
        .exists());
}

/// Verify conformance total ≥ 1009
#[test]
fn test_stage10_6_conformance_total() {
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
        total >= 1009,
        "conformance total should be 1009+, got {total}"
    );
}
