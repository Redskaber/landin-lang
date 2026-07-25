//! Stage 11.4 — e2e expansion (48→160) verification
#![cfg(test)]
use std::path::Path;

#[test]
fn test_stage11_4_e2e_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let e2e_dir = manifest.join("tests/conformance/04-e2e");
    let mut total = 0;
    for entry in std::fs::read_dir(&e2e_dir).expect("read 04-e2e/") {
        let entry = entry.expect("dir entry");
        if entry.path().is_dir() {
            total += std::fs::read_dir(entry.path())
                .expect("read sub")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                .count();
        }
    }
    assert!(
        total >= 160,
        "04-e2e/ should have 160+ .lin files, got {total}"
    );
}

#[test]
fn test_stage11_4_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.4.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.4.md")
        .exists());
}

#[test]
fn test_stage11_4_conformance_total() {
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
        total >= 1841,
        "conformance total should be 1841+, got {total}"
    );
}
