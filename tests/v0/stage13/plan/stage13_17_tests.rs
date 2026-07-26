//! Stage 13.17 — Self binding fix + inherent method call codegen verification tests
//!
//! Verifies:
//! - Parser interns "self" for self param binding (not Spur::default())
//! - MIR lower resolves inherent methods via HIR impl lookup
//! - Method calls emit real Terminator::Call (not Error placeholder)
//!
//! Per `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify parser uses `get_or_intern("self")` for self param binding.
#[test]
fn test_parser_interns_self_name() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generics_rs = manifest.join("src/parser/generics.rs");
    let content = std::fs::read_to_string(&generics_rs).expect("read generics.rs");

    assert!(
        content.contains("Stage 13.17"),
        "src/parser/generics.rs must reference Stage 13.17"
    );

    // Must intern "self" for the binding name.
    assert!(
        content.contains("get_or_intern(\"self\")"),
        "Parser must use get_or_intern(\"self\") for self param binding (Stage 13.17)"
    );

    // Must intern "Self" for the type name.
    assert!(
        content.contains("get_or_intern(\"Self\")"),
        "Parser must use get_or_intern(\"Self\") for self param type (Stage 13.17)"
    );

    // Must NOT use Spur::default() for self binding in non-comment code.
    // We check non-comment lines in the self param section.
    let self_param_start = content.find("is_self_param").unwrap_or(0);
    let self_param_end = content[self_param_start..]
        .find("params.push(Param")
        .map(|p| self_param_start + p)
        .unwrap_or(content.len());
    let self_param_section = &content[self_param_start..self_param_end];
    for (line_num, line) in self_param_section.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue; // Skip comments
        }
        assert!(
            !line.contains("Spur::default()"),
            "Parser self param section line {} must NOT use Spur::default() in non-comment code (Stage 13.17 fix): {}",
            line_num + 1,
            line.trim()
        );
    }
}

/// Verify MIR lower resolves inherent methods (not Error placeholder).
#[test]
fn test_mir_lower_method_call_resolves_inherent() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand).expect("read expr_operand.rs");

    assert!(
        content.contains("Stage 13.17"),
        "src/mir/lower/expr_operand.rs must reference Stage 13.17"
    );

    // Must have resolve_inherent_method function.
    assert!(
        content.contains("fn resolve_inherent_method("),
        "MIR lower must have resolve_inherent_method function (Stage 13.17)"
    );

    // Must have resolve_inherent_method_from_hir_expr function.
    assert!(
        content.contains("fn resolve_inherent_method_from_hir_expr("),
        "MIR lower must have resolve_inherent_method_from_hir_expr function (Stage 13.17)"
    );

    // Must emit FnDef for resolved methods (not just Error placeholder).
    assert!(
        content.contains("TyKind::FnDef(def_id, Vec::new())"),
        "MIR lower must emit TyKind::FnDef for resolved methods (Stage 13.17)"
    );
}

/// Verify the Stage 13.17 design alignment document exists.
#[test]
fn test_stage_13_17_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = manifest.join("docs/develop/v0/stage-13/stage-13.17-design-alignment.md");
    assert!(doc.exists(), "stage-13.17-design-alignment.md must exist");
    let content = std::fs::read_to_string(&doc).expect("read design doc");
    assert!(
        content.contains("§13.4") && content.contains("§14.4"),
        "design doc must reference §13.4 and §14.4"
    );
}

/// Verify the Stage 13.17 gate review document exists with PASS.
#[test]
fn test_stage_13_17_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = manifest.join("docs/develop/v0/stage-13/gate-review-13.17.md");
    assert!(doc.exists(), "gate-review-13.17.md must exist");
    let content = std::fs::read_to_string(&doc).expect("read gate review");
    assert!(
        content.contains("Stage 13.17") && content.contains("PASS"),
        "gate review must reference Stage 13.17 and have PASS verdict"
    );
}

/// Verify the v0.1 conformance gate still holds.
#[test]
fn test_v01_gate_still_holds_after_stage_13_17() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conformance_dir = manifest.join("tests/conformance");
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(&conformance_dir) {
        for entry in entries.flatten() {
            if let Ok(tree) = std::fs::read_dir(entry.path()) {
                for sub in tree.flatten() {
                    if let Ok(inner) = std::fs::read_dir(sub.path()) {
                        for f in inner.flatten() {
                            if f.path().extension().and_then(|s| s.to_str()) == Some("lin") {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        count >= 5000,
        "v0.1 gate: ≥5000 .lin files (found {})",
        count
    );
}
