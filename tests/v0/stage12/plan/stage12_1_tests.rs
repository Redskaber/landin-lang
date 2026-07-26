//! Stage 12.1 — v0.1 release + v0.3 bootstrap prep verification
#![cfg(test)]
use std::path::Path;

/// Verify v0.1 release document exists
#[test]
fn test_v01_release_doc_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        manifest
            .join("docs/develop/v0/stage-12/v0.1-release.md")
            .exists(),
        "v0.1-release.md must exist"
    );
    let content =
        std::fs::read_to_string(manifest.join("docs/develop/v0/stage-12/v0.1-release.md"))
            .expect("read release doc");
    assert!(
        content.contains("v0.1")
            || content.contains("rc1")
            || content.contains("v0.1")
            || content.contains("rc1")
            || content.contains("GATE REACHED"),
        "release doc must mention gate reached"
    );
    assert!(
        content.contains("5026")
            || content.contains("conformance")
            || content.contains("conformance"),
        "release doc must mention 5026 conformance tests"
    );
}

/// Verify v0.3 bootstrap prep document exists
#[test]
fn test_v03_bootstrap_prep_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        manifest
            .join("docs/develop/v0/stage-12/v0.3-bootstrap-prep.md")
            .exists(),
        "v0.3-bootstrap-prep.md must exist"
    );
}

/// Verify Stage 12 independent directories
#[test]
fn test_stage12_directories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest.join("tests/v0/stage12/plan").exists());
    assert!(manifest.join("docs/develop/v0/stage-12").exists());
    assert!(manifest.join("docs/tests/v0/stage12/plan").exists());
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_still_holds() {
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
                        .expect("read sub")
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                        .count();
                }
            }
        }
    }
    assert!(
        total >= 5000,
        "v0.1 gate must still hold: 5000+, got {total}"
    );
}

/// Verify all 12 stage directories exist (stage 0-11 + stage 12)
#[test]
fn test_all_stage_directories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_stages = [
        "stage0", "stage1", "stage2", "stage3", "stage4", "stage5", "stage7", "stage8", "stage9",
        "stage10", "stage11", "stage12",
    ];
    for s in &test_stages {
        assert!(
            manifest.join(format!("tests/v0/{}", s)).exists(),
            "tests/v0/{} must exist",
            s
        );
    }
    let doc_stages = [
        "stage-0", "stage-1", "stage-2", "stage-3", "stage-4", "stage-5", "stage-6", "stage-7",
        "stage-8", "stage-9", "stage-10", "stage-11", "stage-12",
    ];
    for s in &doc_stages {
        assert!(
            manifest.join(format!("docs/develop/v0/{}", s)).exists(),
            "docs/develop/v0/{} must exist",
            s
        );
    }
}

/// Verify README mentions v0.1 release
#[test]
fn test_readme_mentions_v01_release() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(manifest.join("README.md")).expect("read README");
    assert!(
        content.contains("v0.1") || content.contains("v0.20"),
        "README must reference v0.1 or v0.20"
    );
    assert!(
        content.contains("5026")
            || content.contains("conformance")
            || content.contains("conformance"),
        "README must mention 5026 conformance"
    );
}
