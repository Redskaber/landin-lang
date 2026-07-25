//! Stage 11.5 — soundness expansion (50→200) verification
#![cfg(test)]
use std::path::Path;

#[test]
fn test_stage11_5_soundness_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let s_dir = manifest.join("tests/conformance/05-soundness");
    let mut total = 0;
    for entry in std::fs::read_dir(&s_dir).expect("read 05-soundness/") {
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
        total >= 200,
        "05-soundness/ should have 200+ .lin files, got {total}"
    );
}

#[test]
fn test_stage11_5_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.5.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.5.md")
        .exists());
}

#[test]
fn test_stage11_5_conformance_total() {
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
        total >= 1991,
        "conformance total should be 1991+, got {total}"
    );
}
