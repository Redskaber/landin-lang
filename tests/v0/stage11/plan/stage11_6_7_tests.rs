//! Stage 11.6+11.7 — stdlib + integration expansion verification
#![cfg(test)]
use std::path::Path;

#[test]
fn test_stage11_6_stdlib_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let st_dir = manifest.join("tests/conformance/06-stdlib");
    let mut total = 0;
    for entry in std::fs::read_dir(&st_dir).expect("read 06-stdlib/") {
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
        "06-stdlib/ should have 200+ .lin files, got {total}"
    );
}

#[test]
fn test_stage11_7_integration_expanded() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let int_dir = manifest.join("tests/conformance/07-integration");
    let mut total = 0;
    for entry in std::fs::read_dir(&int_dir).expect("read 07-integration/") {
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
        "07-integration/ should have 200+ .lin files, got {total}"
    );
}

#[test]
fn test_stage11_6_7_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.6.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.6.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.7.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.7.md")
        .exists());
}

#[test]
fn test_stage11_6_7_conformance_total() {
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
        total >= 2294,
        "conformance total should be 2294+, got {total}"
    );
}
