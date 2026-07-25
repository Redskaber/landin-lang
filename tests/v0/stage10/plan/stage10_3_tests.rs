//! Stage 10.3 — 03-codegen conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 03-codegen directory has 60+ .lin files
#[test]
fn test_stage10_3_codegen_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cg_dir = manifest.join("tests/conformance/03-codegen");
    assert!(cg_dir.exists(), "03-codegen/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&cg_dir).expect("read 03-codegen/") {
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
        total >= 60,
        "03-codegen/ should have 60+ .lin files, got {total}"
    );
}

/// Verify all 6 codegen subcategories present
#[test]
fn test_stage10_3_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cg_dir = manifest.join("tests/conformance/03-codegen");

    for sub in &[
        "00-llvm-ir-output",
        "01-abi",
        "02-type-layout",
        "03-drop-glue",
        "04-vtable",
        "99-panic-paths",
    ] {
        assert!(cg_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify Stage 10.3 docs created
#[test]
fn test_stage10_3_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-10/plan-10.3.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-10/gate-review-10.3.md")
        .exists());
}

/// Verify conformance total ≥ 861
#[test]
fn test_stage10_3_conformance_total() {
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
        total >= 861,
        "conformance total should be 861+, got {total}"
    );
}
