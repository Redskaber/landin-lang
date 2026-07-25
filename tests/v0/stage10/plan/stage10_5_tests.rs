//! Stage 10.5 — 05-soundness conformance verification tests

#![cfg(test)]

use std::path::Path;

/// Verify 05-soundness directory has 50+ .lin files
#[test]
fn test_stage10_5_soundness_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s_dir = manifest.join("tests/conformance/05-soundness");
    assert!(s_dir.exists(), "05-soundness/ must exist");

    let mut total = 0;
    for entry in std::fs::read_dir(&s_dir).expect("read 05-soundness/") {
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
        "05-soundness/ should have 50+ .lin files, got {total}"
    );
}

/// Verify all 5 soundness subcategories present
#[test]
fn test_stage10_5_subcategories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s_dir = manifest.join("tests/conformance/05-soundness");

    for sub in &[
        "00-r5-regression",
        "01-drop-check",
        "02-lifetime-edge",
        "03-trait-coherence",
        "04-unsafe-boundary",
    ] {
        assert!(s_dir.join(sub).exists(), "subcategory {sub} must exist");
    }
}

/// Verify Stage 10.5 docs created in stage-10 directory (not stage-9)
#[test]
fn test_stage10_5_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        manifest
            .join("docs/develop/v0/stage-10/plan-10.5.md")
            .exists(),
        "plan-10.5.md must exist in stage-10/ directory"
    );
    assert!(
        manifest
            .join("docs/develop/v0/stage-10/gate-review-10.5.md")
            .exists(),
        "gate-review-10.5.md must exist in stage-10/ directory"
    );
}

/// Verify stage10 test directory is independent from stage9
#[test]
fn test_stage10_test_directory_independent() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        manifest.join("tests/v0/stage10/plan").exists(),
        "tests/v0/stage10/plan/ must exist as independent directory"
    );
}

/// Verify conformance total ≥ 959
#[test]
fn test_stage10_5_conformance_total() {
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
        total >= 959,
        "conformance total should be 959+, got {total}"
    );
}
