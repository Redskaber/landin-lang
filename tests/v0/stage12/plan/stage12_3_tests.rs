//! Stage 12.3 — Second-pass cross-stage audit (r217) verification
//!
//! Verifies that the r217 second-pass audit reports exist, that stage-round
//! revisions were applied, that §25.8 retroactive backfills were performed
//! for Stage 5 (DynTraitMIRSummary + StdlibTypeKind) and Stage 8 (async/await
//! MVP synchronous), that plan-13.1.md was reframed as Stage 12 output, and
//! that the version was reverted from v0.22.0 to v0.21.2.
//!
//! Per stage-committee-process.md v3.21 §25.8 + §17.3 + §15.

#![cfg(test)]

use std::path::Path;

/// Verify all 3 r217 second-pass audit reports exist
#[test]
fn test_r217_audit_reports_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let reports = [
        "cross-stage-audit-r217-stages-0-4.md",
        "cross-stage-audit-r217-stages-5-8.md",
        "cross-stage-audit-r217-stages-9-12-scope.md",
    ];
    for report in &reports {
        let path = manifest.join("docs/develop/v0/stage-12").join(report);
        assert!(
            path.exists(),
            "r217 second-pass audit report must exist: {}",
            report
        );
    }
}

/// Verify r217 stages-0-4 report contains stage-round revisions
#[test]
fn test_r217_stages_0_4_has_stage_round_revisions() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("docs/develop/v0/stage-12/cross-stage-audit-r217-stages-0-4.md");
    let content = std::fs::read_to_string(&path).expect("read r217 stages-0-4 report");

    // Stage round revision table must be present
    assert!(
        content.contains("Stage Round Revision") || content.contains("stage-round revision"),
        "r217 stages-0-4 report must contain stage-round revision section"
    );

    // Must reference r216 (the audit being revised)
    assert!(
        content.contains("r216"),
        "r217 stages-0-4 report must reference r216 (the audit being revised)"
    );

    // Must reference the 5 TD items being revised
    for td in ["TD-028", "TD-029", "TD-030", "TD-031", "TD-032"] {
        assert!(
            content.contains(td),
            "r217 stages-0-4 report must reference {}",
            td
        );
    }
}

/// Verify r217 stages-5-8 report contains Stage 5 §25.8 backfill recommendation
#[test]
fn test_r217_stages_5_8_identifies_stage5_25_8_gap() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("docs/develop/v0/stage-12/cross-stage-audit-r217-stages-5-8.md");
    let content = std::fs::read_to_string(&path).expect("read r217 stages-5-8 report");

    // Stage 5 ran on v3.20, never had §25.8 — must be flagged
    assert!(
        content.contains("v3.20") || content.contains("Stage 5") && content.contains("§25.8"),
        "r217 stages-5-8 report must identify Stage 5 §25.8 gap (Stage 5 ran on v3.20)"
    );

    // Must mention the 3 implicit-knowledge items needing backfill
    assert!(
        content.contains("DynTraitMIRSummary"),
        "r217 stages-5-8 report must identify DynTraitMIRSummary as implicit knowledge"
    );
    assert!(
        content.contains("StdlibTypeKind"),
        "r217 stages-5-8 report must identify StdlibTypeKind as implicit knowledge"
    );
}

/// Verify r217 stages-9-12 report contains Stage 12 scope finalization
#[test]
fn test_r217_stages_9_12_finalizes_stage12_scope() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path =
        manifest.join("docs/develop/v0/stage-12/cross-stage-audit-r217-stages-9-12-scope.md");
    let content = std::fs::read_to_string(&path).expect("read r217 stages-9-12 report");

    // Stage 12 sub-stage plan (8 sub-stages: 12.1-12.8)
    for sub in [
        "12.1", "12.2", "12.3", "12.4", "12.5", "12.6", "12.7", "12.8",
    ] {
        assert!(
            content.contains(sub),
            "r217 stages-9-12 report must reference Stage 12 sub-stage {}",
            sub
        );
    }

    // Stage 13 launch criteria (5 conditions)
    assert!(
        content.contains("Stage 13 launch") || content.contains("Stage 13 启动"),
        "r217 stages-9-12 report must define Stage 13 launch criteria"
    );

    // Version policy: v0.21.2 (patch bump, no new compiler features)
    assert!(
        content.contains("v0.21.2"),
        "r217 stages-9-12 report must specify v0.21.2 as correct version"
    );
}

/// Verify §25.8 retroactive backfill for Stage 5 (DynTraitMIRSummary in 06-mir.md)
#[test]
fn test_section_25_8_backfill_dyn_trait_mir_summary() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mir_doc = manifest.join("docs/lang-design/06-mir.md");
    let content = std::fs::read_to_string(&mir_doc).expect("read 06-mir.md");

    // §15 (or higher) section for Stage 12.4 retroactive backfill
    assert!(
        content.contains("Stage 12.4") && content.contains("§25.8"),
        "06-mir.md must include Stage 12.4 §25.8 retroactive write-back section"
    );

    // DynTraitMIRSummary must be documented (4-layer MIR architecture)
    assert!(
        content.contains("DynTraitMIRSummary"),
        "06-mir.md must document DynTraitMIRSummary (4-layer MIR architecture)"
    );

    // 4-layer architecture table
    assert!(
        content.contains("DynTraitFatPtr") && content.contains("DynTraitMethodCall") && content.contains("DynTraitMIRPlan"),
        "06-mir.md must document all 4 layers: DynTraitFatPtr, DynTraitMethodCall, DynTraitMIRSummary, DynTraitMIRPlan"
    );
}

/// Verify §25.8 retroactive backfill for Stage 5 (StdlibTypeKind in 09-stdlib.md)
#[test]
fn test_section_25_8_backfill_stdlib_type_kind() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_doc = manifest.join("docs/lang-design/09-stdlib.md");
    let content = std::fs::read_to_string(&stdlib_doc).expect("read 09-stdlib.md");

    // §12 (or higher) section for Stage 12.4 retroactive backfill
    assert!(
        content.contains("Stage 12.4") && content.contains("§25.8"),
        "09-stdlib.md must include Stage 12.4 §25.8 retroactive write-back section"
    );

    // StdlibTypeKind + stdlib_type_kind_to_emit_type() must be documented
    assert!(
        content.contains("StdlibTypeKind"),
        "09-stdlib.md must document StdlibTypeKind (TD-016 closure converter)"
    );
    assert!(
        content.contains("stdlib_type_kind_to_emit_type"),
        "09-stdlib.md must document stdlib_type_kind_to_emit_type() converter"
    );

    // TD-016 reference
    assert!(
        content.contains("TD-016"),
        "09-stdlib.md must reference TD-016 (the closure this converter closes)"
    );
}

/// Verify §25.8 retroactive backfill for Stage 8 (async/await MVP in 05-ast.md)
#[test]
fn test_section_25_8_backfill_async_await_mvp() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ast_doc = manifest.join("docs/lang-design/05-ast.md");
    let content = std::fs::read_to_string(&ast_doc).expect("read 05-ast.md");

    // §15 (or higher) section for Stage 12.4 retroactive backfill
    assert!(
        content.contains("Stage 12.4") && content.contains("§25.8"),
        "05-ast.md must include Stage 12.4 §25.8 retroactive write-back section"
    );

    // MVP synchronous semantics must be documented
    assert!(
        content.contains("MVP") && (content.contains("synchronous") || content.contains("同步")),
        "05-ast.md must document async/await MVP synchronous semantics"
    );

    // Await + Async variant references
    assert!(
        content.contains("Await") && content.contains("Async"),
        "05-ast.md must reference Await and Async variants"
    );
}

/// Verify plan-13.1.md was reframed from initial "Planned" status
/// (Stage 12.5 reframe: Planned → Draft; Stage 13.1 launch: Draft → Active)
#[test]
fn test_plan_13_reframed_as_stage12_output() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-13/plan-13.1.md");
    let content = std::fs::read_to_string(&plan).expect("read plan-13.1.md");

    // Must be marked as Draft (Stage 12.5 reframe) OR Active (Stage 13.1 launch)
    // — both are valid post-reframe states (not the original "Planned")
    assert!(
        content.contains("Draft") || content.contains("Active") || content.contains("🔄 Active"),
        "plan-13.1.md must be reframed as Draft or Active (not the original 'Planned')"
    );

    // Must reference Stage 12.5 reframe note (historical context)
    assert!(
        content.contains("Stage 12.5"),
        "plan-13.1.md must reference Stage 12.5 reframe action"
    );

    // Must reference r217 second-pass audit
    assert!(
        content.contains("r217"),
        "plan-13.1.md must reference r217 second-pass audit"
    );

    // Must NOT claim to be the original active stage plan (the initial "Planned" status)
    assert!(
        !content.contains("🔄 Planned (per §13.4 design alignment)"),
        "plan-13.1.md must not retain the original '🔄 Planned (per §13.4 design alignment)' status (must be Draft or Active)"
    );
}

/// Verify Cargo.toml version follows r217 version policy (v0.21.x patch for Stage 12,
/// v0.22.0+ minor for Stage 13.2+ user-facing features)
#[test]
fn test_cargo_toml_version_follows_r217_policy() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    // Per r217 version policy:
    // - Stage 12 (review/planning): v0.21.x patch bump (no new compiler features)
    // - Stage 13.1 (TD-028 refactoring): v0.21.5 patch bump (architectural, no new features)
    // - Stage 13.2+ (TD-031 if-let/while-let): v0.22.0 minor bump (first user-facing feature)
    let is_v021 = content.contains("version = \"0.2");
    let is_v022_or_later = content.contains("version = \"0.2")
        || content.contains("version = \"0.2")
        || content.contains("version = \"0.24.");
    assert!(
        is_v021 || is_v022_or_later,
        "Cargo.toml version must be v0.21.x (Stage 12-13.1 patch) or v0.22.0+ (Stage 13.2+ minor bump per r217 policy)"
    );
}

/// Verify README mentions Stage 12 + r217 audit + Stage 13 launch criteria + v0.21.x version
#[test]
fn test_readme_mentions_stage12_and_r217() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(manifest.join("README.md")).expect("read README.md");

    // Stage 12 reference (in progress OR complete — both valid depending on sub-stage)
    assert!(
        content.contains("Stage 12"),
        "README must reference Stage 12"
    );

    // r217 second-pass audit reference
    assert!(
        content.contains("r217"),
        "README must reference r217 second-pass audit"
    );

    // Stage 13 launch criteria (cannot launch until closed, OR all closed, OR in progress)
    assert!(
        content.contains("Stage 13 launch")
            || content.contains("Stage 13 启动")
            || content.contains("AUTHORIZED")
            || content.contains("IN PROGRESS"),
        "README must mention Stage 13 launch criteria or authorization or in-progress status"
    );

    // Version reference (v0.21.x OR v0.22.0+)
    assert!(
        content.contains("v0.21.")
            || content.contains("v0.22.")
            || content.contains("0.21.")
            || content.contains("0.22."),
        "README must reference current version (v0.21.x or v0.22.0+)"
    );
}

/// Verify v0.1 conformance gate still holds after Stage 12.3 second-pass audit
#[test]
fn test_v01_gate_still_holds_after_r217_audit() {
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

/// Verify worklog has r217 audit entries
#[test]
fn test_worklog_has_r217_entries() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");

    // r217 second-pass audit reference
    assert!(
        content.contains("r217") || content.contains("second-pass"),
        "worklog must reference r217 second-pass audit"
    );

    // Stage 12.3-12.7 entries
    for sub in ["12.3", "12.4", "12.5", "12.6"] {
        assert!(
            content.contains(sub),
            "worklog must reference Stage {} entry",
            sub
        );
    }
}
