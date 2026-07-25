//! Stage 11.3 — codegen expansion (61→231) verification
#![cfg(test)]
use std::path::Path;

#[test]
fn test_stage11_3_codegen_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cg_dir = manifest.join("tests/conformance/03-codegen");
    let mut total = 0;
    for entry in std::fs::read_dir(&cg_dir).expect("read 03-codegen/") {
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
        total >= 230,
        "03-codegen/ should have 230+ .lin files, got {total}"
    );
}

#[test]
fn test_stage11_3_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.3.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.3.md")
        .exists());
}

#[test]
fn test_stage11_3_conformance_total() {
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
        total >= 1729,
        "conformance total should be 1729+, got {total}"
    );
}
