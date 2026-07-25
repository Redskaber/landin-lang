//! Stage 10.4 — 04-e2e conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 04-e2e directory has 48+ .lin files
#[test]
fn test_stage10_4_e2e_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let e2e_dir = manifest.join("tests/conformance/04-e2e");
    assert!(e2e_dir.exists(), "04-e2e/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&e2e_dir).expect("read 04-e2e/") {
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
        total >= 48,
        "04-e2e/ should have 48+ .lin files, got {total}"
    );
}

/// Verify all 6 e2e subcategories present
#[test]
fn test_stage10_4_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let e2e_dir = manifest.join("tests/conformance/04-e2e");

    for sub in &[
        "00-hello-world",
        "01-fib",
        "02-traits",
        "03-closures",
        "04-error-handling",
        "05-real-world",
    ] {
        assert!(e2e_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify Stage 10.4 docs created
#[test]
fn test_stage10_4_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-9/plan-10.4.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-9/gate-review-10.4.md")
        .exists());
}

/// Verify conformance total ≥ 909
#[test]
fn test_stage10_4_conformance_total() {
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
        total >= 909,
        "conformance total should be 909+, got {total}"
    );
}
