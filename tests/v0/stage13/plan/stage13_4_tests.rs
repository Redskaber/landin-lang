//! Stage 13.4 — macro_rules! / built-in macros (TD-032 P0) preparation phase verification
//!
//! Verifies that the §13.4 design alignment was performed, TD-032 was reframed
//! (macro_rules! is NOT a v0.3 blocker; actual blocker is 19 missing built-in macros),
//! and the gate review marks this as a preparation phase.
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.7 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify Stage 13.4 design alignment report exists
#[test]
fn test_stage13_4_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let report = manifest.join("docs/develop/v0/stage-13/stage-13.4-design-alignment.md");
    assert!(report.exists(), "stage-13.4-design-alignment.md must exist");

    let content = std::fs::read_to_string(&report).expect("read design-alignment.md");
    assert!(
        content.contains("§13.4") || content.contains("13.4"),
        "must reference §13.4"
    );
    assert!(content.contains("TD-032"), "must cover TD-032");
    assert!(
        content.contains("Strategy"),
        "must discuss strategy options"
    );
}

/// Verify TD-032 reframe: macro_rules! is NOT the blocker; 19 missing built-in macros is
#[test]
fn test_td_032_reframe_documented() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let report = manifest.join("docs/develop/v0/stage-13/stage-13.4-design-alignment.md");
    let content = std::fs::read_to_string(&report).expect("read design-alignment.md");

    // Must reframe TD-032 as 19 missing built-in macros (not macro_rules!)
    assert!(
        content.contains("19") && content.contains("built-in"),
        "design alignment must reframe TD-032 as 19 missing built-in macros"
    );

    // Must note macro_rules! is design-forbidden for v0.1/v0.3
    assert!(
        content.contains("forbid") || content.contains("forbidden") || content.contains("REJECTED"),
        "design alignment must note macro_rules! is design-forbidden for v0.1/v0.3"
    );
}

/// Verify Stage 13.4 gate review exists + marks preparation phase
#[test]
fn test_stage13_4_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate = manifest.join("docs/develop/v0/stage-13/gate-review-13.4.md");
    assert!(gate.exists(), "gate-review-13.4.md must exist");

    let content = std::fs::read_to_string(&gate).expect("read gate-review-13.4.md");
    assert!(content.contains("TD-032"), "must reference TD-032");
    assert!(
        content.contains("PREPARATION") || content.contains("preparation"),
        "must mark preparation phase"
    );
    assert!(content.contains("13.4a"), "must reference Stage 13.4a");
    assert!(content.contains("PASS"), "must reach PASS verdict");
}

/// Verify gate review documents the TD-032 reframe
#[test]
fn test_gate_review_documents_reframe() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate = manifest.join("docs/develop/v0/stage-13/gate-review-13.4.md");
    let content = std::fs::read_to_string(&gate).expect("read gate-review-13.4.md");

    assert!(
        content.contains("reframe") || content.contains("Reframe") || content.contains("REFRAME"),
        "gate review must document the TD-032 reframe"
    );
    assert!(
        content.contains("19") && content.contains("built-in"),
        "gate review must reference 19 missing built-in macros"
    );
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_still_holds_after_stage13_4() {
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

/// Verify worklog has Stage 13.4 entry
#[test]
fn test_worklog_has_stage13_4_entry() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");
    assert!(
        content.contains("stage-13.4") || content.contains("Stage 13.4"),
        "worklog must reference Stage 13.4"
    );
}
