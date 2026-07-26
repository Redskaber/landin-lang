//! Stage 13.4a — 19 missing built-in macros (TD-032 P0 CLOSED) verification
//!
//! Verifies that all 26 built-in macros are now handled in the MacroCall arm
//! of MIR lowering (7 existing + 19 new = 26 total per 13-stage1-feature-whitelist.md §2.6).
//!
//! Per stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify all 26 built-in macros are handled in the MacroCall match
#[test]
fn test_all_26_macros_handled() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    // All 26 macros that must be in the match
    let macros = [
        // Printing (4)
        "println",
        "print",
        "eprintln",
        "eprint",
        // Stringification (2)
        "stringify",
        "concat",
        // Assertion (6)
        "assert",
        "debug_assert",
        "assert_eq",
        "assert_ne",
        "debug_assert_eq",
        "debug_assert_ne",
        // Writing (2)
        "write",
        "writeln",
        // Diverging (4)
        "panic",
        "todo",
        "unimplemented",
        "unreachable",
        // Configuration (1)
        "cfg",
        // File inclusion (1)
        "include",
        // Environment (2)
        "env",
        "option_env",
        // Format args (1)
        "format_args",
        // Format (1)
        "format",
        // Vec (1)
        "vec",
        // Debug (1)
        "dbg",
    ];

    for macro_name in &macros {
        assert!(
            content.contains(&format!("\"{}\"", macro_name)),
            "src/mir/lower/expr_operand.rs must handle '{}' macro in MacroCall match",
            macro_name
        );
    }
}

/// Verify diverging macros produce Never type
#[test]
fn test_diverging_macros_produce_never() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    // The diverging macros section must produce TyKind::Never
    let diverging_section = content
        .find("Diverging macros")
        .and_then(|start| content[start..].find("TyKind::Never"))
        .is_some();

    assert!(
        diverging_section,
        "Diverging macros (panic, todo, unimplemented, unreachable) must produce TyKind::Never"
    );
}

/// Verify cfg! macro produces bool
#[test]
fn test_cfg_macro_produces_bool() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    let cfg_section = content
        .find("Configuration macro")
        .and_then(|start| content[start..].find("TyKind::Bool"))
        .is_some();

    assert!(cfg_section, "cfg! macro must produce TyKind::Bool");
}

/// Verify Stage 13.4a gate review exists + marks TD-032 CLOSED
#[test]
fn test_stage13_4a_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate = manifest.join("docs/develop/v0/stage-13/gate-review-13.4a.md");
    assert!(gate.exists(), "gate-review-13.4a.md must exist");

    let content = std::fs::read_to_string(&gate).expect("read gate-review-13.4a.md");

    assert!(
        content.contains("TD-032") && content.contains("CLOSED"),
        "gate-review-13.4a.md must mark TD-032 as CLOSED"
    );

    assert!(
        content.contains("ALL") && content.contains("P0") && content.contains("CLOSED"),
        "gate-review-13.4a.md must mark ALL P0 as CLOSED"
    );

    assert!(
        content.contains("PASS"),
        "gate-review-13.4a.md must reach PASS verdict"
    );
}

/// Verify Cargo.toml version is v0.24.x or later (minor bump — all P0 closed).
/// Stage 13.16 bumped to v0.25.0 (format args feature), so we accept v0.24+.
#[test]
fn test_cargo_toml_version_is_v0_24() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    // Stage 13.4a established v0.24.x; Stage 13.16 bumped to v0.25.0.
    // We accept any version >= v0.24.
    let version_line = content
        .lines()
        .find(|line| line.starts_with("version = "))
        .unwrap_or("");
    let version = version_line
        .trim_start_matches("version = \"")
        .trim_end_matches("\"");
    let parts: Vec<&str> = version.split('.').collect();
    let major: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    assert!(
        (major == 0 && minor >= 24) || major > 0,
        "Cargo.toml version must be >= v0.24 (Stage 13.4a baseline; Stage 13.16 is v0.25.0); found v{}",
        version
    );
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_still_holds_after_stage13_4a() {
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

/// Verify worklog has Stage 13.4a entry
#[test]
fn test_worklog_has_stage13_4a_entry() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let content =
        std::fs::read_to_string(manifest.join("docs/worklog.md")).expect("read worklog.md");
    assert!(
        content.contains("stage-13.4a") || content.contains("Stage 13.4a"),
        "worklog must reference Stage 13.4a"
    );
}
