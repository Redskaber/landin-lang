//! Stage 13.3 — Closure call lowering (TD-030 P0) preparation phase verification
//!
//! Verifies that the §13.4 design alignment was performed, that the implementation
//! blueprint is documented, and that the gate review marks this as a preparation
//! phase (TD-030 remains OPEN, full implementation deferred to Stage 13.3a).
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.7 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify Stage 13.3 design alignment report exists
#[test]
fn test_stage13_3_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_alignment = manifest.join("docs/develop/v0/stage-13/stage-13.3-design-alignment.md");
    assert!(
        design_alignment.exists(),
        "docs/develop/v0/stage-13/stage-13.3-design-alignment.md must exist (§13.4 design alignment)"
    );

    let content = std::fs::read_to_string(&design_alignment).expect("read design-alignment.md");

    // Must reference §13.4
    assert!(
        content.contains("§13.4") || content.contains("13.4"),
        "design-alignment.md must reference §13.4"
    );

    // Must cover TD-030
    assert!(
        content.contains("TD-030"),
        "design-alignment.md must cover TD-030 (closure call lowering)"
    );

    // Must recommend Strategy A (Direct call function synthesis)
    assert!(
        content.contains("Strategy A") || content.contains("Direct call function synthesis"),
        "design-alignment.md must recommend Strategy A (Direct call function synthesis)"
    );

    // Must reference rustc approach
    assert!(
        content.contains("rustc"),
        "design-alignment.md must reference rustc approach"
    );
}

/// Verify Stage 13.3 design alignment covers implementation blueprint
#[test]
fn test_stage13_3_design_alignment_has_blueprint() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_alignment = manifest.join("docs/develop/v0/stage-13/stage-13.3-design-alignment.md");
    let content = std::fs::read_to_string(&design_alignment).expect("read design-alignment.md");

    // Must cover synthesized call function
    assert!(
        content.contains("synthesized") && content.contains("call"),
        "design-alignment.md must cover synthesized call function"
    );

    // Must cover Fn/FnMut/FnOnce strategy
    assert!(
        content.contains("Fn") && content.contains("FnMut") && content.contains("FnOnce"),
        "design-alignment.md must cover Fn/FnMut/FnOnce strategy"
    );

    // Must reference 07-codegen.md §8 (the design pre-sanctioning Strategy A)
    assert!(
        content.contains("07-codegen.md") || content.contains("§8"),
        "design-alignment.md must reference 07-codegen.md §8 (design pre-sanctioning)"
    );
}

/// Verify Stage 13.3 gate review exists + marks preparation phase
#[test]
fn test_stage13_3_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.3.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.3.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.3.md");

    // Must reference TD-030
    assert!(
        content.contains("TD-030"),
        "gate-review-13.3.md must reference TD-030"
    );

    // Must mark this as preparation phase (TD-030 NOT YET CLOSED)
    assert!(
        content.contains("PREPARATION") || content.contains("preparation"),
        "gate-review-13.3.md must mark this as preparation phase"
    );

    // Must reference Stage 13.3a (the full implementation phase)
    assert!(
        content.contains("13.3a") || content.contains("Stage 13.3a"),
        "gate-review-13.3.md must reference Stage 13.3a (full implementation phase)"
    );

    // Must include committee vote
    assert!(
        content.contains("委员会投票") || content.contains("Committee") || content.contains("Vote"),
        "gate-review-13.3.md must include committee vote"
    );

    // Must reach PASS verdict (for the preparation phase)
    assert!(
        content.contains("PASS"),
        "gate-review-13.3.md must reach PASS verdict for preparation phase"
    );
}

/// Verify Stage 13.3 gate review documents the implementation blueprint
#[test]
fn test_stage13_3_gate_review_has_blueprint() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.3.md");
    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.3.md");

    // Must document the 6-step implementation blueprint
    // Note: content uses backticks around "call" — match the actual text
    assert!(
        content.contains("synthesized") && content.contains("function MirBody"),
        "gate-review-13.3.md must document synthesized call function MirBody step"
    );

    assert!(
        content.contains("closure_call_bodies"),
        "gate-review-13.3.md must document closure_call_bodies side-table"
    );

    assert!(
        content.contains("Terminator::Call") && content.contains("closure dispatch"),
        "gate-review-13.3.md must document HirExprKind::Call closure dispatch"
    );

    assert!(
        content.contains("Codegen") && content.contains("synthesized"),
        "gate-review-13.3.md must document codegen for synthesized functions"
    );
}

/// Verify Stage 13.3 gate review has correct version policy
#[test]
fn test_stage13_3_gate_review_version_policy() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.3.md");
    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.3.md");

    // Must specify patch bump (preparation phase, no new features)
    assert!(
        content.contains("v0.22.1") || content.contains("patch bump"),
        "gate-review-13.3.md must specify patch bump (v0.22.1) for preparation phase"
    );

    // Must reserve v0.23.0 for Stage 13.3a
    assert!(
        content.contains("v0.23.0"),
        "gate-review-13.3.md must reserve v0.23.0 for Stage 13.3a (TD-030 closure)"
    );
}

/// Verify current closure call lowering state (placeholder — TD-030 still open)
#[test]
fn test_closure_call_lowering_current_state() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    // The closure call detection must exist (Stage 4.13 scaffold)
    assert!(
        content.contains("is_closure") && content.contains("TyKind::Closure"),
        "src/mir/lower/expr_operand.rs must have closure call detection (is_closure + TyKind::Closure)"
    );

    // The placeholder must still exist (TD-030 not yet closed)
    assert!(
        content.contains("fresh_infer_ty") && content.contains("placeholder"),
        "src/mir/lower/expr_operand.rs must still have placeholder (TD-030 not yet closed)"
    );
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_still_holds_after_stage13_3() {
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

/// Verify worklog has Stage 13.3 entry
#[test]
fn test_worklog_has_stage13_3_entry() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");

    assert!(
        content.contains("stage-13.3") || content.contains("Stage 13.3"),
        "worklog must reference Stage 13.3"
    );
    assert!(
        content.contains("TD-030") || content.contains("closure call"),
        "worklog must reference TD-030 or closure call lowering"
    );
}
