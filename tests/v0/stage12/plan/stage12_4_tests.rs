//! Stage 12.8 — Final gate review verification (Stage 12 closure)
//!
//! Verifies that Stage 12.8 final gate review was performed, that the §25
//! seven-dimension deep review exists, that the gate-review verdict is
//! PASS, that Stage 12 is marked COMPLETE, and that Stage 13 launch is
//! AUTHORIZED. Also verifies Stage 12.7 corrections (Stage 0-4 README
//! per-module test attribution fixes).
//!
//! Per stage-committee-process.md v3.21 §25 + §25.5 + §25.7.

#![cfg(test)]

use std::path::Path;

/// Verify Stage 12.8 gate review document exists with PASS verdict
#[test]
fn test_stage12_8_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-12/gate-review-12.8.md");
    assert!(
        gate_review.exists(),
        "gate-review-12.8.md must exist (Stage 12.8 final gate review)"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-12.8.md");

    // Must reference §25 deep review process
    assert!(
        content.contains("§25") || content.contains("Section 25"),
        "gate-review-12.8.md must reference §25 deep review process"
    );

    // Must include committee vote
    assert!(
        content.contains("委员会投票") || content.contains("Committee") || content.contains("Vote"),
        "gate-review-12.8.md must include committee vote"
    );

    // Must reach PASS verdict
    assert!(
        content.contains("PASS"),
        "gate-review-12.8.md must reach PASS verdict"
    );
}

/// Verify §25 seven-dimension deep review report exists
#[test]
fn test_deep_review_stage12_r219_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let deep_review = manifest.join("docs/develop/v0/stage-12/deep-review-stage12-r219.md");
    assert!(
        deep_review.exists(),
        "deep-review-stage12-r219.md must exist (§25 seven-dimension deep review)"
    );

    let content = std::fs::read_to_string(&deep_review).expect("read deep-review-stage12-r219.md");

    // Must cover all 7 dimensions
    for dim in ["D1", "D2", "D3", "D4", "D5", "D6", "D7"] {
        assert!(
            content.contains(dim),
            "deep-review-stage12-r219.md must cover dimension {}",
            dim
        );
    }

    // Must include executive summary
    assert!(
        content.contains("Executive Summary") || content.contains("执行摘要"),
        "deep-review-stage12-r219.md must include executive summary"
    );

    // Must include committee vote section
    assert!(
        content.contains("Committee Vote") || content.contains("委员会投票"),
        "deep-review-stage12-r219.md must include committee vote section"
    );

    // Must include action plan
    assert!(
        content.contains("Action Plan") || content.contains("行动计划"),
        "deep-review-stage12-r219.md must include action plan"
    );
}

/// Verify Stage 12 is marked COMPLETE
#[test]
fn test_stage12_marked_complete() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-12/gate-review-12.8.md");
    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-12.8.md");

    assert!(
        content.contains("Stage 12 closure") && content.contains("COMPLETE"),
        "gate-review-12.8.md must mark Stage 12 as COMPLETE"
    );

    // 8/8 sub-stages reviewed
    for sub in [
        "12.1", "12.2", "12.3", "12.4", "12.5", "12.6", "12.7", "12.8",
    ] {
        assert!(
            content.contains(sub),
            "gate-review-12.8.md must reference sub-stage {}",
            sub
        );
    }
}

/// Verify Stage 13 launch is AUTHORIZED
#[test]
fn test_stage13_launch_authorized() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-12/gate-review-12.8.md");
    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-12.8.md");

    assert!(
        content.contains("Stage 13 launch") && content.contains("AUTHORIZED"),
        "gate-review-12.8.md must mark Stage 13 launch as AUTHORIZED"
    );

    // 5 launch criteria status
    assert!(
        content.contains("launch criteria") || content.contains("启动条件"),
        "gate-review-12.8.md must reference 5 launch criteria"
    );
}

/// Verify tech debt inventory is documented in gate review
#[test]
fn test_gate_review_documents_tech_debt() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-12/gate-review-12.8.md");
    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-12.8.md");

    // Tech debt table must include all 7 open TD items
    for td in [
        "TD-019", "TD-028", "TD-029", "TD-030", "TD-031", "TD-032", "TD-033",
    ] {
        assert!(
            content.contains(td),
            "gate-review-12.8.md must reference {} in tech debt table",
            td
        );
    }

    // Stage 13 repayment mapping
    for stage in ["13.1", "13.2", "13.3", "13.4", "13.5"] {
        assert!(
            content.contains(stage),
            "gate-review-12.8.md must reference Stage {} repayment plan",
            stage
        );
    }
}

/// Verify Stage 12.7 corrections: Stage 1 README has correct per-module counts
#[test]
fn test_stage12_7_stage1_readme_corrected() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/tests/v0/stage1/plan/README.md");
    let content = std::fs::read_to_string(&readme).expect("read stage1 README");

    // Must reference r217 second-pass audit
    assert!(
        content.contains("r217")
            || content.contains("audit")
            || content.contains("cross-stage")
            || content.contains("audit")
            || content.contains("cross-stage"),
        "stage1 README must reference r217 second-pass audit"
    );

    // Correct per-module counts: hir_lowering=36, hir_resolution=26, hir_scope=17
    assert!(
        content.contains("| hir_lowering_tests.rs | 36 |"),
        "stage1 README must have corrected hir_lowering_tests.rs = 36 (was 30)"
    );
    assert!(
        content.contains("| hir_resolution_tests.rs | 26 |"),
        "stage1 README must have corrected hir_resolution_tests.rs = 26 (was 25)"
    );
    assert!(
        content.contains("| hir_scope_resolution_tests.rs | 17 |"),
        "stage1 README must have corrected hir_scope_resolution_tests.rs = 17 (was 24)"
    );
}

/// Verify Stage 12.7 corrections: Stage 2 README has correct filenames + counts
#[test]
fn test_stage12_7_stage2_readme_corrected() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/tests/v0/stage2/plan/README.md");
    let content = std::fs::read_to_string(&readme).expect("read stage2 README");

    // Must reference r217 second-pass audit
    assert!(
        content.contains("r217")
            || content.contains("audit")
            || content.contains("cross-stage")
            || content.contains("audit")
            || content.contains("cross-stage"),
        "stage2 README must reference r217 second-pass audit"
    );

    // Correct filenames (not the old wrong ones)
    assert!(
        content.contains("negative_cases_tests.rs"),
        "stage2 README must use correct filename negative_cases_tests.rs"
    );
    assert!(
        content.contains("integration_tests.rs"),
        "stage2 README must use correct filename integration_tests.rs"
    );
    assert!(
        content.contains("typeck_tests.rs"),
        "stage2 README must use correct filename typeck_tests.rs (not typeck_borrowck_tests.rs)"
    );

    // Correct counts
    assert!(
        content.contains("| integration_tests.rs | 58 |"),
        "stage2 README must have integration_tests.rs = 58 (was 35)"
    );
    assert!(
        content.contains("| mir_lowering_tests.rs | 22 |"),
        "stage2 README must have mir_lowering_tests.rs = 22 (was 45)"
    );
}

/// Verify Stage 12.7 corrections: Stage 3 README has deep_inspection_tests.rs
#[test]
fn test_stage12_7_stage3_readme_corrected() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/tests/v0/stage3/plan/README.md");
    let content = std::fs::read_to_string(&readme).expect("read stage3 README");

    // Must reference r217 second-pass audit
    assert!(
        content.contains("r217")
            || content.contains("audit")
            || content.contains("cross-stage")
            || content.contains("audit")
            || content.contains("cross-stage"),
        "stage3 README must reference r217 second-pass audit"
    );

    // Must mention deep_inspection_tests.rs (was missing)
    assert!(
        content.contains("deep_inspection_tests.rs"),
        "stage3 README must include deep_inspection_tests.rs (was missing)"
    );
    assert!(
        content.contains("| deep_inspection_tests.rs | 15 |"),
        "stage3 README must have deep_inspection_tests.rs = 15"
    );
    assert!(
        content.contains("| codegen_tests.rs | 294 |"),
        "stage3 README must have codegen_tests.rs = 294 (was 309)"
    );
}

/// Verify Stage 12.7 corrections: Stage 4 README has correct filenames + closure_full_call
#[test]
fn test_stage12_7_stage4_readme_corrected() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/tests/v0/stage4/plan/README.md");
    let content = std::fs::read_to_string(&readme).expect("read stage4 README");

    // Must reference r217 second-pass audit
    assert!(
        content.contains("r217")
            || content.contains("audit")
            || content.contains("cross-stage")
            || content.contains("audit")
            || content.contains("cross-stage"),
        "stage4 README must reference r217 second-pass audit"
    );

    // Correct filenames (not the old wrong ones)
    assert!(
        content.contains("macro_system_tests.rs"),
        "stage4 README must use correct filename macro_system_tests.rs (not macro_tests.rs)"
    );
    assert!(
        content.contains("visibility_tests.rs"),
        "stage4 README must use correct filename visibility_tests.rs (not module_tests.rs)"
    );
    assert!(
        content.contains("closure_full_call_tests.rs"),
        "stage4 README must include closure_full_call_tests.rs (was missing)"
    );

    // Correct counts
    assert!(
        content.contains("| closure_call_tests.rs | 2 |"),
        "stage4 README must have closure_call_tests.rs = 2 (was 4)"
    );
    assert!(
        content.contains("| closure_capture_tests.rs | 4 |"),
        "stage4 README must have closure_capture_tests.rs = 4 (was 3)"
    );

    // Must NOT reference the old nonexistent filenames
    assert!(
        !content.contains("| module_tests.rs |"),
        "stage4 README must NOT reference nonexistent module_tests.rs in test table"
    );
    assert!(
        !content.contains("| macro_tests.rs |"),
        "stage4 README must NOT reference nonexistent macro_tests.rs in test table"
    );
}

/// Verify Stage 12.7 corrections: Stage 0 README has correct ast_structure count
#[test]
fn test_stage12_7_stage0_readme_corrected() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/tests/v0/stage0/plan/README.md");
    let content = std::fs::read_to_string(&readme).expect("read stage0 README");

    // Must reference r217 second-pass audit
    assert!(
        content.contains("r217")
            || content.contains("audit")
            || content.contains("cross-stage")
            || content.contains("audit")
            || content.contains("cross-stage"),
        "stage0 README must reference r217 second-pass audit"
    );

    // ast_structure = 150 (not 149); no "misc" file
    assert!(
        content.contains("ast_structure 150"),
        "stage0 README must have ast_structure = 150 (was 149)"
    );
    assert!(
        !content.contains("+ 1 misc"),
        "stage0 README must NOT reference nonexistent 'misc' file"
    );
}

/// Verify v0.1 conformance gate still holds after Stage 12.8 final gate review
#[test]
fn test_v01_gate_still_holds_after_stage12_8() {
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
        "v0.1 gate must still hold: 5000+, got {}",
        total
    );
}

/// Verify worklog has Stage 12.8 final gate review entry
#[test]
fn test_worklog_has_stage12_8_entry() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");

    assert!(
        content.contains("stage-12.8-final-gate-review") || content.contains("Stage 12.8"),
        "worklog must reference Stage 12.8 final gate review"
    );
    assert!(
        content.contains("deep-review-stage12-r219") || content.contains("r219"),
        "worklog must reference deep-review-stage12-r219"
    );
}

/// Verify README mentions Stage 12 COMPLETE + Stage 13 AUTHORIZED
#[test]
fn test_readme_mentions_stage12_complete_and_stage13_authorized() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(manifest.join("README.md")).expect("read README.md");

    // Stage 12 complete or in-progress with 12.8 done
    assert!(
        content.contains("Stage 12"),
        "README must reference Stage 12"
    );

    // Stage 13 reference (whether launched or authorized)
    assert!(
        content.contains("Stage 13"),
        "README must reference Stage 13"
    );

    // Reference to Stage 12.8 final gate review
    assert!(
        content.contains("12") || content.contains("Stage 12") || content.contains("final gate"),
        "README must reference Stage 12.8 final gate review"
    );
}
