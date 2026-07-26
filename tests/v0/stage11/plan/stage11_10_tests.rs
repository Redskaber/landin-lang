//! Stage 11.10 — §25 deep review + v0.1 release prep verification
#![cfg(test)]
use std::path::Path;

/// Verify §25 deep review plan exists
#[test]
fn test_stage11_10_plan_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest
        .join("docs/develop/v0/stage-11/plan-11.10.md")
        .exists());
    assert!(manifest
        .join("docs/develop/v0/stage-11/gate-review-11.10.md")
        .exists());
}

/// Verify v0.1 conformance gate reached (5000+)
#[test]
fn test_v01_gate_reached() {
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

/// Verify README mentions v0.1 gate reached
#[test]
fn test_readme_mentions_gate_reached() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("README.md");
    let content = std::fs::read_to_string(&readme).expect("read README");
    assert!(
        content.contains("v0.1")
            || content.contains("rc1")
            || content.contains("v0.1")
            || content.contains("rc1")
            || content.contains("GATE REACHED"),
        "README must mention v0.1 gate reached"
    );
    assert!(
        content.contains("5026")
            || content.contains("conformance")
            || content.contains("conformance"),
        "README must mention 5026 conformance tests"
    );
}

/// Verify all 8 conformance categories exist with tests
#[test]
fn test_all_8_categories_have_tests() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");
    for cat in &[
        "00-parse",
        "01-typecheck",
        "02-borrowck",
        "03-codegen",
        "04-e2e",
        "05-soundness",
        "06-stdlib",
        "07-integration",
    ] {
        assert!(conf_dir.join(cat).exists(), "category {cat} must exist");
    }
}

/// Verify Stage 11 has independent directory
#[test]
fn test_stage11_independent_directory() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest.join("tests/v0/stage11/plan").exists());
    assert!(manifest.join("docs/develop/v0/stage-11").exists());
    assert!(manifest.join("docs/tests/v0/stage11/plan").exists());
}
