//! v0.1 Gap Analysis verification tests
//!
//! Per stage-committee-process.md v3.21 §25 deep review protocol.
//! Verifies the v0.1 gap analysis findings and current state.

#![cfg(test)]

use std::path::Path;

/// v0.1 gap: conformance suite has only 00-parse category (1 of 8 required)
#[test]
fn test_v01_gap_only_parse_category_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf_dir = manifest.join("tests/conformance");

    let parse_dir = conf_dir.join("00-parse");
    assert!(parse_dir.exists(), "00-parse/ must exist");

    // 7 categories missing per §5.1
    let missing_categories = [
        "01-typecheck",
        "02-borrowck",
        "03-codegen",
        "04-e2e",
        "05-soundness",
        "06-stdlib",
        "07-integration",
    ];

    for cat in &missing_categories {
        let path = conf_dir.join(cat);
        // These don't exist yet — this is the gap
        assert!(
            !path.exists(),
            "conformance category {cat} should NOT exist yet (Stage 10 will create it)"
        );
    }
}

/// v0.1 gap: current conformance count is 600, not 5000
#[test]
fn test_v01_gap_current_conformance_count() {
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

    // Current: 600 parse tests (12% of 5000 target)
    assert_eq!(
        total, 600,
        "current conformance count should be 600 (parse only), got {total}"
    );
}

/// v0.1 gap: .lin format uses //! instead of // per §3 spec
#[test]
fn test_v01_gap_lin_format_uses_bang_comment() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join("tests/conformance/00-parse/00-literals/int_dec_basic.lin");
    let content = std::fs::read_to_string(&sample).expect("read sample .lin");

    // Current format uses //! (Rust doc comment)
    assert!(
        content.starts_with("//! PASS"),
        "current .lin format uses //! PASS (should be // EXPECTED: compile_ok per §3 spec)"
    );
}

/// v0.1 gap: CLI only supports --emit-tokens and --emit-ast, no --compile
#[test]
fn test_v01_gap_cli_no_compile_option() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // Current CLI has --emit-tokens and --emit-ast
    assert!(
        content.contains("emit_tokens"),
        "CLI must have --emit-tokens"
    );
    assert!(content.contains("emit_ast"), "CLI must have --emit-ast");

    // But no --compile / --run / --emit-llvm-ir (the gap)
    assert!(
        !content.contains("--compile"),
        "CLI should NOT have --compile yet (Stage 10.0 will add it)"
    );
}

/// v0.1 gap: gap analysis document exists
#[test]
fn test_v01_gap_analysis_doc_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gap_doc = manifest.join("docs/develop/v0/stage-9/v0.1-gap-analysis.md");
    assert!(gap_doc.exists(), "v0.1-gap-analysis.md must exist");

    let content = std::fs::read_to_string(&gap_doc).expect("read gap analysis");
    assert!(
        content.contains("GO-WITH-CONDITIONS"),
        "gap analysis must conclude GO-WITH-CONDITIONS"
    );
    assert!(
        content.contains("5,000"),
        "gap analysis must reference 5,000 test target"
    );
}

/// v0.1 gap: Stage 10 plan document exists
#[test]
fn test_v01_gap_stage10_plan_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-9/plan-stage10.md");
    assert!(plan.exists(), "plan-stage10.md must exist");

    let content = std::fs::read_to_string(&plan).expect("read Stage 10 plan");
    assert!(
        content.contains("10.0") && content.contains("10.8"),
        "Stage 10 plan must cover sub-stages 10.0-10.8"
    );
}

/// v0.1 gap: gate-review-v0.1-gap.md exists
#[test]
fn test_v01_gap_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate = manifest.join("docs/develop/v0/stage-9/gate-review-v0.1-gap.md");
    assert!(gate.exists(), "gate-review-v0.1-gap.md must exist");
}

/// v0.1 gap: conformance suite spec defines 5000 tests in 8 categories
#[test]
fn test_v01_gap_spec_defines_5000_tests() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = manifest.join("docs/lang-design/17-conformance-suite.md");
    let content = std::fs::read_to_string(&spec).expect("read conformance spec");

    assert!(
        content.contains("5,000") || content.contains("5000"),
        "conformance spec must define 5,000 tests"
    );
    assert!(
        content.contains("00-parse") && content.contains("07-integration"),
        "conformance spec must define 8 categories (00-parse through 07-integration)"
    );
}

/// v0.1 gap: roadmap defines 5000 test requirement for v0.1
#[test]
fn test_v01_gap_roadmap_defines_5000_requirement() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roadmap = manifest.join("docs/lang-design/12-roadmap.md");
    let content = std::fs::read_to_string(&roadmap).expect("read roadmap");

    assert!(
        content.contains("5,000") || content.contains("5000"),
        "roadmap must define 5,000 conformance test requirement for v0.1"
    );
}

/// v0.1 gap: current state reclassified as "Parse conformance milestone" not "v0.1 RC"
#[test]
fn test_v01_gap_reclassified_as_parse_milestone() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate = manifest.join("docs/develop/v0/stage-9/gate-review-v0.1-gap.md");
    let content = std::fs::read_to_string(&gate).expect("read gate review");

    assert!(
        content.contains("Parse conformance milestone"),
        "gate review must reclassify current state as 'Parse conformance milestone'"
    );
}
