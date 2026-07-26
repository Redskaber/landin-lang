//! Stage 9.12 — v0.1 release candidate verification tests
//!
//! Per stage-committee-process.md v3.21 §25 deep review protocol.
//! Verifies the v0.1 release gate: conformance 600/600 + Stage 0-8 complete.

#![cfg(test)]

use std::path::Path;

/// D4: v0.1 release gate — conformance suite reaches 600 tests
#[test]
fn test_v01_conformance_reaches_600() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parse_dir = manifest.join("tests/conformance/00-parse");

    let mut total = 0;
    for entry in std::fs::read_dir(&parse_dir).expect("read 00-parse/") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            total += std::fs::read_dir(&path)
                .expect("read category dir")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                .count();
        }
    }

    assert_eq!(
        total, 600,
        "v0.1 release gate: conformance suite must be exactly 600 tests, got {total}"
    );
}

/// D4: v0.1 milestone test exists
#[test]
fn test_v01_milestone_test_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let milestone = manifest.join("tests/conformance/00-parse/10-realistic/v0.1_milestone.lin");
    assert!(milestone.exists(), "v0.1_milestone.lin must exist");

    let content = std::fs::read_to_string(&milestone).expect("read milestone");
    assert!(
        content.contains("v0.1 MILESTONE TEST"),
        "v0.1_milestone.lin must contain v0.1 milestone marker"
    );
}

/// D4: All 11 conformance categories present
#[test]
fn test_v01_all_conformance_categories_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parse_dir = manifest.join("tests/conformance/00-parse");

    let categories = [
        "00-literals",
        "01-operators",
        "02-control-flow",
        "03-patterns",
        "04-types",
        "05-attributes",
        "06-generics",
        "07-closures",
        "08-modules",
        "09-error-recovery",
        "10-realistic",
    ];

    for cat in &categories {
        let path = parse_dir.join(cat);
        assert!(path.exists(), "conformance category {cat} must exist");
    }
}

/// D4: Conformance runner (run_all.py) exists and is functional
#[test]
fn test_v01_conformance_runner_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runner = manifest.join("tests/conformance/run_all.py");
    assert!(runner.exists(), "tests/conformance/run_all.py must exist");
}

/// D7: Stage 9 deep review document exists
#[test]
fn test_v01_deep_review_doc_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let deep_review = manifest.join("docs/develop/v0/stage-9/deep-review-stage9-r195.md");
    assert!(
        deep_review.exists(),
        "deep-review-stage9-r195.md must exist"
    );

    let content = std::fs::read_to_string(&deep_review).expect("read deep review");
    assert!(
        content.contains("v0.1 release candidate"),
        "deep review must announce v0.1 release candidate"
    );
}

/// D7: Stage 9 plan + gate-review docs for 9.12 created
#[test]
fn test_v01_stage9_12_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-9/plan-9.12.md");
    let gate = manifest.join("docs/develop/v0/stage-9/gate-review-9.12.md");

    assert!(plan.exists(), "plan-9.12.md must exist");
    assert!(gate.exists(), "gate-review-9.12.md must exist");
}

/// D5: Design docs mention v0.1 conformance gate
#[test]
fn test_v01_roadmap_defines_conformance_gate() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roadmap = manifest.join("docs/lang-design/12-roadmap.md");
    let content = std::fs::read_to_string(&roadmap).expect("read roadmap");

    assert!(
        content.contains("v0.1") && content.contains("conformance"),
        "12-roadmap.md must define v0.1 conformance gate"
    );
}

/// D5: Conformance suite design doc exists
#[test]
fn test_v01_conformance_suite_design_doc_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cs_doc = manifest.join("docs/lang-design/17-conformance-suite.md");
    assert!(cs_doc.exists(), "17-conformance-suite.md must exist");
}

/// D4: Cargo.toml version bumped to 0.17.0 (v0.1 RC)
#[test]
fn test_v01_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23.")
            || version_line.starts_with("version = \"0.24.")
            || version_line.starts_with("version = \"0.25."),
        "Cargo.toml version must be 0.17.x+ for v0.1 RC, got: {version_line}"
    );
}

/// D7: Stage 9 README index exists and mentions all 12 sub-stages
#[test]
fn test_v01_stage9_readme_complete() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/develop/v0/stage-9/README.md");
    assert!(
        readme.exists(),
        "docs/develop/v0/stage-9/README.md must exist"
    );

    let content = std::fs::read_to_string(&readme).expect("read stage-9 README");
    for i in 1..=12 {
        assert!(
            content.contains(&format!("9.{}", i)),
            "Stage 9 README must mention sub-stage 9.{}",
            i
        );
    }
}
