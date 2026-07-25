//! Stage 11.9 — final batch expansion (2766→5026, +2260) — v0.1 gate reached!
#![cfg(test)]
use std::path::Path;

#[test]
fn test_stage11_9_conformance_reaches_5000() {
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
        "v0.1 gate: conformance must be 5000+, got {total}"
    );
}

#[test]
fn test_stage11_9_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.9.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.9.md")
        .exists());
}

#[test]
fn test_stage11_9_all_categories_meet_target() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");
    let targets = [
        ("00-parse", 600),
        ("01-typecheck", 400),
        ("02-borrowck", 300),
        ("03-codegen", 200),
        ("04-e2e", 200),
        ("05-soundness", 200),
        ("06-stdlib", 200),
        ("07-integration", 200),
    ];
    for (cat, min) in &targets {
        let cat_dir = conf_dir.join(cat);
        let mut count = 0;
        for sub in std::fs::read_dir(cat_dir).expect("read category") {
            let sub = sub.expect("sub");
            if sub.path().is_dir() {
                count += std::fs::read_dir(sub.path())
                    .expect("read sub")
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                    .count();
            }
        }
        assert!(
            count >= *min,
            "category {cat} should have {min}+ tests, got {count}"
        );
    }
}
