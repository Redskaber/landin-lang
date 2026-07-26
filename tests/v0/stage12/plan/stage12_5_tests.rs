//! Stage 12.9 — Polish backfill verification (deferred P2/P3 items from gate-review-12.8)
//!
//! Verifies that the 3 deferred P2/P3 polish items from gate-review-12.8.md were closed:
//! 1. Stage 5 develop-side README.md created
//! 2. Stage 6 plan-6.{4,5,6}.md retroactively backfilled
//! 3. api-naming-standard v2.36 record corrected (+10 → +12 tests)
//!
//! Per stage-committee-process.md v3.21 §25.7 + §15.

#![cfg(test)]

use std::path::Path;

/// Verify Stage 5 develop-side README.md exists (D7 gap from r217 stages-5-8 audit)
#[test]
fn test_stage5_develop_readme_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = manifest.join("docs/develop/v0/stage-5/README.md");
    assert!(
        readme.exists(),
        "docs/develop/v0/stage-5/README.md must exist (D7 gap from r217 stages-5-8 audit §5.5)"
    );

    let content = std::fs::read_to_string(&readme).expect("read stage-5 README");

    // Must reference Stage 5 sub-stages (99 sub-stages)
    assert!(
        content.contains("99") && content.contains("sub-stage"),
        "stage-5 README must reference 99 sub-stages"
    );

    // Must reference key tech decisions (TD-014, TD-016, TD-018)
    for td in ["TD-014", "TD-016", "TD-018"] {
        assert!(content.contains(td), "stage-5 README must reference {}", td);
    }

    // Must reference §25.8 retroactive backfill (Stage 12.4)
    assert!(
        content.contains("§25.8") || content.contains("25.8"),
        "stage-5 README must reference §25.8 retroactive backfill (Stage 12.4)"
    );

    // Must reference the 4-layer MIR architecture
    assert!(
        content.contains("DynTraitMIRSummary") || content.contains("4-layer"),
        "stage-5 README must reference DynTraitMIRSummary 4-layer MIR architecture"
    );
}

/// Verify Stage 6 plan-6.4.md retroactively backfilled
#[test]
fn test_stage6_plan_6_4_backfilled() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-6/plan-6.4.md");
    assert!(
        plan.exists(),
        "docs/develop/v0/stage-6/plan-6.4.md must exist (retroactive backfill per r217 stages-5-8 §7)"
    );

    let content = std::fs::read_to_string(&plan).expect("read plan-6.4.md");

    // Must be marked as retroactive backfill
    assert!(
        content.contains("Retroactive")
            || content.contains("retroactive")
            || content.contains("回填"),
        "plan-6.4.md must be marked as retroactive backfill"
    );

    // Must reference §14.4 (refactor governance)
    assert!(
        content.contains("§14.4") || content.contains("14.4"),
        "plan-6.4.md must reference §14.4 refactor governance"
    );

    // Must reference the source gate review
    assert!(
        content.contains("gate-review-6.4"),
        "plan-6.4.md must reference gate-review-6.4.md (backfill source)"
    );

    // Must reference Stage 12.9 (the backfill stage)
    assert!(
        content.contains("Stage 12.9") || content.contains("stage-12.9"),
        "plan-6.4.md must reference Stage 12.9 (backfill stage)"
    );
}

/// Verify Stage 6 plan-6.5.md retroactively backfilled
#[test]
fn test_stage6_plan_6_5_backfilled() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-6/plan-6.5.md");
    assert!(
        plan.exists(),
        "docs/develop/v0/stage-6/plan-6.5.md must exist (retroactive backfill per r217 stages-5-8 §7)"
    );

    let content = std::fs::read_to_string(&plan).expect("read plan-6.5.md");

    assert!(
        content.contains("Retroactive")
            || content.contains("retroactive")
            || content.contains("回填"),
        "plan-6.5.md must be marked as retroactive backfill"
    );
    assert!(
        content.contains("§14.4") || content.contains("14.4"),
        "plan-6.5.md must reference §14.4 refactor governance"
    );
    assert!(
        content.contains("gate-review-6.5"),
        "plan-6.5.md must reference gate-review-6.5.md (backfill source)"
    );
}

/// Verify Stage 6 plan-6.6.md retroactively backfilled
#[test]
fn test_stage6_plan_6_6_backfilled() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-6/plan-6.6.md");
    assert!(
        plan.exists(),
        "docs/develop/v0/stage-6/plan-6.6.md must exist (retroactive backfill per r217 stages-5-8 §7)"
    );

    let content = std::fs::read_to_string(&plan).expect("read plan-6.6.md");

    assert!(
        content.contains("Retroactive")
            || content.contains("retroactive")
            || content.contains("回填"),
        "plan-6.6.md must be marked as retroactive backfill"
    );
    assert!(
        content.contains("§14.4") || content.contains("14.4"),
        "plan-6.6.md must reference §14.4 refactor governance"
    );
    assert!(
        content.contains("gate-review-6.6"),
        "plan-6.6.md must reference gate-review-6.6.md (backfill source)"
    );
}

/// Verify Stage 6 plan file count is now 18 (was 15 — r217 stages-5-8 §3.1 finding)
#[test]
fn test_stage6_plan_count_now_18() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stage6_dir = manifest.join("docs/develop/v0/stage-6");

    let plan_count = std::fs::read_dir(&stage6_dir)
        .expect("read stage-6 dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("plan-6.") && n.ends_with(".md"))
                .unwrap_or(false)
        })
        .count();

    assert!(
        plan_count >= 18,
        "Stage 6 plan files must be ≥18 (was 15 before Stage 12.9 backfill), got {}",
        plan_count
    );
}

/// Verify api-naming-standard v2.36 record corrected (+10 → +12 tests)
#[test]
fn test_api_naming_v2_36_record_corrected() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let api_doc = manifest.join("docs/develop/v0/api-naming-standard.md");
    let content = std::fs::read_to_string(&api_doc).expect("read api-naming-standard.md");

    // The corrected record must say +12 (not +10) — this is the primary impact line
    assert!(
        content.contains("+12 rust (2325 → 2337)"),
        "api-naming-standard v2.36 record must be corrected to '+12 rust (2325 → 2337)'"
    );

    // The correction note must be present (explains the +10 → +12 delta)
    assert!(
        content.contains("Stage 12.9 correction"),
        "api-naming-standard must include Stage 12.9 correction note explaining +10 → +12"
    );

    // Find v2.36 section and verify the PRIMARY impact line (the one with "**Test impact**:")
    // is the corrected +12 version, not the incorrect +10 version
    let v236_section_start = content.find("### v2.36").expect("v2.36 section must exist");
    let v237_section_start = content.find("### v2.37").expect("v2.37 section must exist");
    let v236_section = &content[v236_section_start..v237_section_start];

    // The primary impact line in v2.36 must be +12, not +10
    assert!(
        v236_section.contains("**Test impact**: +12 rust (2325 → 2337)"),
        "v2.36 section primary '**Test impact**:' line must be '+12 rust (2325 → 2337)' (corrected)"
    );

    // The v2.36 section must NOT have the incorrect primary impact line
    assert!(
        !v236_section.contains("**Test impact**: +10 rust (2325 → 2335)"),
        "v2.36 section must NOT contain the incorrect '**Test impact**: +10 rust (2325 → 2335)' primary record"
    );
}

/// Verify Stage 12.9 plan + gate review + polish report all exist
#[test]
fn test_stage12_9_documents_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

    let plan = manifest.join("docs/develop/v0/stage-12/plan-12.9.md");
    assert!(
        plan.exists(),
        "docs/develop/v0/stage-12/plan-12.9.md must exist"
    );

    let gate_review = manifest.join("docs/develop/v0/stage-12/gate-review-12.9.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-12/gate-review-12.9.md must exist"
    );

    let polish_report =
        manifest.join("docs/develop/v0/stage-12/stage-12.9-polish-backfill-report.md");
    assert!(
        polish_report.exists(),
        "docs/develop/v0/stage-12/stage-12.9-polish-backfill-report.md must exist"
    );

    // gate-review-12.9.md must mark Stage 12.9 as PASS
    let gate_content = std::fs::read_to_string(&gate_review).expect("read gate-review-12.9.md");
    assert!(
        gate_content.contains("PASS"),
        "gate-review-12.9.md must reach PASS verdict"
    );
}

/// Verify Stage 12.9 plan references the 3 deferred items
#[test]
fn test_stage12_9_plan_references_deferred_items() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-12/plan-12.9.md");
    let content = std::fs::read_to_string(&plan).expect("read plan-12.9.md");

    // Must reference gate-review-12.8 (source of deferred items)
    assert!(
        content.contains("gate-review-12.8"),
        "plan-12.9.md must reference gate-review-12.8.md (source of deferred items)"
    );

    // Must reference the 3 MUV items
    assert!(
        content.contains("MUV-1") && content.contains("Stage 5 develop README"),
        "plan-12.9.md must reference MUV-1 (Stage 5 develop README)"
    );
    assert!(
        content.contains("MUV-2") && content.contains("plan-6"),
        "plan-12.9.md must reference MUV-2 (plan-6.{{4,5,6}}.md backfill)"
    );
    assert!(
        content.contains("MUV-3") && content.contains("v2.36"),
        "plan-12.9.md must reference MUV-3 (api-naming-standard v2.36 correction)"
    );

    // Must reference §15 (long-term > short-term)
    assert!(
        content.contains("§15"),
        "plan-12.9.md must reference §15 (long-term > short-term)"
    );

    // Must reference §25.7 (P2/P3 problem handling)
    assert!(
        content.contains("§25.7") || content.contains("25.7"),
        "plan-12.9.md must reference §25.7 (P2/P3 problem handling)"
    );
}

/// Verify v0.1 conformance gate still holds after Stage 12.9
#[test]
fn test_v01_gate_still_holds_after_stage12_9() {
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

/// Verify worklog has Stage 12.9 entry
#[test]
fn test_worklog_has_stage12_9_entry() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");

    assert!(
        content.contains("stage-12.9") || content.contains("Stage 12.9"),
        "worklog must reference Stage 12.9"
    );
    assert!(
        content.contains("polish-backfill") || content.contains("polish"),
        "worklog must reference polish backfill work"
    );
}

/// Verify README mentions Stage 12.9 (or Stage 12 with 9 sub-stages)
#[test]
fn test_readme_mentions_stage12_9() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(manifest.join("README.md")).expect("read README.md");

    // README must mention Stage 12.9 OR the polish backfill work
    assert!(
        content.contains("12") || content.contains("Stage 12") || content.contains("polish"),
        "README must reference Stage 12.9 or polish backfill work"
    );
}
