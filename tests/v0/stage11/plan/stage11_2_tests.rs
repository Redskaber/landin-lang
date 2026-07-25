//! Stage 11.2 — borrowck expansion (80→300) verification
#![cfg(test)]
use std::path::Path;

#[test]
fn test_stage11_2_borrowck_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bk_dir = manifest.join("tests/conformance/02-borrowck");
    let mut total = 0;
    for entry in std::fs::read_dir(&bk_dir).expect("read 02-borrowck/") {
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
        total >= 300,
        "02-borrowck/ should have 300+ .lin files, got {total}"
    );
}

#[test]
fn test_stage11_2_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.2.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.2.md")
        .exists());
}

#[test]
fn test_stage11_2_conformance_total() {
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
        total >= 1559,
        "conformance total should be 1559+, got {total}"
    );
}
