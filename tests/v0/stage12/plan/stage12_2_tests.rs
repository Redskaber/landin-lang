//! Stage 12.2 — Cross-stage audit ratification + Stage 13 plan verification
//!
//! Verifies that the multi-agent group review (ARCH-A + QA-A + REV-A + PM-A)
//! produced the required audit reports, that §25.8 design write-back was
//! performed for the newly-discovered TyKind::Dynamic deviation, that all
//! 13 stage plan/README files exist (D7 backfill), and that Stage 13
//! planning documents are in place.
//!
//! Per stage-committee-process.md v3.21 §25.8 + §17.3.

#![cfg(test)]

use std::path::Path;

/// Verify cross-stage architecture audit (ARCH-A, D1 + D5) exists
#[test]
fn test_cross_stage_arch_audit_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let audit = manifest.join("docs/develop/v0/stage-12/cross-stage-audit-r216-architecture.md");
    assert!(
        audit.exists(),
        "cross-stage-audit-r216-architecture.md must exist"
    );

    let content = std::fs::read_to_string(&audit).expect("read audit");
    assert!(
        content.contains("D1") && content.contains("D5"),
        "audit must cover D1 (architecture) + D5 (design rationality)"
    );
    assert!(
        content.contains("§16")
            || content.contains("Section 16")
            || content.contains("interface isolation"),
        "audit must verify §16 interface isolation"
    );
    assert!(
        content.contains("§25.8") || content.contains("25.8"),
        "audit must perform §25.8 design deviation analysis"
    );
    assert!(
        content.contains("GO-WITH-CONDITIONS")
            || content.contains("GO")
            || content.contains("NO-GO"),
        "audit must include committee verdict"
    );
}

/// Verify cross-stage techdebt/tests/docs audit (D2+D3+D4+D6+D7) exists
#[test]
fn test_cross_stage_techdebt_audit_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let audit =
        manifest.join("docs/develop/v0/stage-12/cross-stage-audit-r216-techdebt-tests-docs.md");
    assert!(
        audit.exists(),
        "cross-stage-audit-r216-techdebt-tests-docs.md must exist"
    );

    let content = std::fs::read_to_string(&audit).expect("read audit");
    for dim in ["D2", "D3", "D4", "D6", "D7"] {
        assert!(content.contains(dim), "audit must cover dimension {dim}");
    }
    assert!(
        content.contains("Stage 13"),
        "audit must include Stage 13 recommendation"
    );
    assert!(
        content.contains("Option B") || content.contains("compile pipeline"),
        "audit must recommend Option B (compile pipeline fixes)"
    );
}

/// Verify §25.8 design write-back was performed for TyKind::Dynamic (TD-029)
#[test]
fn test_section_25_8_writeback_for_tykind_dynamic() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let type_system_doc = manifest.join("docs/lang-design/03-type-system.md");
    assert!(type_system_doc.exists(), "03-type-system.md must exist");

    let content = std::fs::read_to_string(&type_system_doc).expect("read type-system doc");
    // §25.8 write-back section for Stage 12 must be added
    assert!(
        content.contains("Stage 12") && content.contains("§25.8"),
        "type-system.md must include Stage 12 §25.8 write-back section"
    );
    // Specifically, the TyKind::Dynamic B1 deviation must be documented
    assert!(
        content.contains("TyKind::Dynamic") || content.contains("TraitObject"),
        "type-system.md must document the TyKind::Dynamic / TraitObject B1 deviation"
    );
    // And the v0.3 self-hosting prerequisites must be listed
    assert!(
        content.contains("TD-030") || content.contains("closure"),
        "type-system.md must reference TD-030 (closure call lowering) as v0.3 prerequisite"
    );
    assert!(
        content.contains("TD-031") || content.contains("if let"),
        "type-system.md must reference TD-031 (if-let/while-let) as v0.3 prerequisite"
    );
    assert!(
        content.contains("TD-032") || content.contains("macro_rules"),
        "type-system.md must reference TD-032 (macro_rules!) as v0.3 prerequisite"
    );
}

/// Verify all 13 stage plan/README.md files exist (D7 backfill — Stages 0-12)
#[test]
fn test_all_stage_plan_readmes_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Stages 0-12 (Stage 13's was just created in this commit)
    for stage in 0..=12u8 {
        let p = manifest
            .join(format!("docs/tests/v0/stage{}/plan", stage))
            .join("README.md");
        assert!(
            p.exists(),
            "docs/tests/v0/stage{}/plan/README.md must exist (D7 backfill)",
            stage
        );
    }
}

/// Verify Stage 13 plan documents are in place
#[test]
fn test_stage13_plan_documents_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Stage 13 develop directory
    assert!(
        manifest.join("docs/develop/v0/stage-13").exists(),
        "docs/develop/v0/stage-13/ must exist"
    );
    assert!(
        manifest.join("docs/develop/v0/stage-13/README.md").exists(),
        "docs/develop/v0/stage-13/README.md must exist"
    );
    assert!(
        manifest
            .join("docs/develop/v0/stage-13/plan-13.1.md")
            .exists(),
        "docs/develop/v0/stage-13/plan-13.1.md must exist"
    );

    // Stage 13 test doc directory
    assert!(
        manifest.join("docs/tests/v0/stage13/plan").exists(),
        "docs/tests/v0/stage13/plan/ must exist"
    );
    assert!(
        manifest
            .join("docs/tests/v0/stage13/plan/README.md")
            .exists(),
        "docs/tests/v0/stage13/plan/README.md must exist"
    );

    // Stage 13 test directory
    assert!(
        manifest.join("tests/v0/stage13/plan").exists(),
        "tests/v0/stage13/plan/ must exist"
    );
}

/// Verify Stage 13 plan references required process sections
#[test]
fn test_stage13_plan_process_compliance() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = manifest.join("docs/develop/v0/stage-13/plan-13.1.md");
    let content = std::fs::read_to_string(&plan).expect("read plan-13.1.md");

    // §13.4 design alignment
    assert!(
        content.contains("§13.4") && content.contains("design alignment"),
        "plan-13.1.md must reference §13.4 (design alignment)"
    );
    // §14.4 refactor governance
    assert!(
        content.contains("§14.4"),
        "plan-13.1.md must reference §14.4 (refactor governance)"
    );
    // §15 long-term > short-term
    assert!(
        content.contains("§15"),
        "plan-13.1.md must reference §15 (long-term > short-term)"
    );
    // §25.8 design write-back
    assert!(
        content.contains("§25.8"),
        "plan-13.1.md must reference §25.8 (design write-back)"
    );
    // MUV (Minimum Verifiable Unit) breakdown
    assert!(
        content.contains("MUV"),
        "plan-13.1.md must include MUV (Minimum Verifiable Unit) breakdown"
    );
    // TD-028..TD-033 tech debt references
    for td in ["TD-028", "TD-029", "TD-030", "TD-031", "TD-032", "TD-033"] {
        assert!(content.contains(td), "plan-13.1.md must reference {}", td);
    }
}

/// Verify all 14 stage develop directories exist (stage-0 through stage-13)
#[test]
fn test_all_14_stage_develop_directories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for stage in 0..=13 {
        let dir = manifest.join(format!("docs/develop/v0/stage-{}", stage));
        assert!(dir.exists(), "docs/develop/v0/stage-{} must exist", stage);
    }
}

/// Verify all 14 stage test-doc directories exist (stage0 through stage13)
#[test]
fn test_all_14_stage_testdoc_directories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for stage in 0..=13 {
        let dir = manifest.join(format!("docs/tests/v0/stage{}", stage));
        assert!(dir.exists(), "docs/tests/v0/stage{} must exist", stage);
    }
}

/// Verify all 14 stage test directories exist (tests/v0/stage{0..13})
#[test]
fn test_all_14_stage_test_directories_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Note: stage6 has no test directory (pure refactoring stage); skip it
    let expected = [
        "stage0", "stage1", "stage2", "stage3", "stage4", "stage5", "stage7", "stage8", "stage9",
        "stage10", "stage11", "stage12", "stage13",
    ];
    for s in &expected {
        assert!(
            manifest.join(format!("tests/v0/{}", s)).exists(),
            "tests/v0/{} must exist",
            s
        );
    }
}

/// Verify v0.1 conformance gate still holds after Stage 13.1 plan creation
#[test]
fn test_v01_gate_still_holds_after_stage13_plan() {
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

/// Verify README.md mentions Stage 13 / v0.3 prep / cross-stage audit
#[test]
fn test_readme_mentions_stage13_and_audit() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(manifest.join("README.md")).expect("read README.md");
    assert!(
        content.contains("Stage 13") || content.contains("stage-13") || content.contains("v0.3"),
        "README must mention Stage 13 or v0.3 prep"
    );
    assert!(
        content.contains("cross-stage") || content.contains("audit") || content.contains("r216"),
        "README must mention cross-stage audit (r216)"
    );
}

/// Verify worklog has Stage 13.1 / r216 audit entries
#[test]
fn test_worklog_has_audit_entries() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");
    assert!(
        content.contains("r216") || content.contains("cross-stage-audit-r216"),
        "worklog must reference r216 cross-stage audit"
    );
    assert!(
        content.contains("ARCH-A") || content.contains("QA-A") || content.contains("REV-A"),
        "worklog must reference agent roles (ARCH-A / QA-A / REV-A)"
    );
}
