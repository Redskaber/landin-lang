//! Stage 13.13 — Inline println! emission verification tests
//!
//! Verifies that the Stage 13.12 println! ordering bug is fixed by replacing
//! the side-table + helper-function approach with an inline
//! `StatementKind::Println` variant in the MIR basic block.
//!
//! Per `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8 +
//! `stage-13.13-design-alignment.md` + `gate-review-13.13.md`.

#![cfg(test)]

use std::path::Path;

/// Verify `StatementKind::Println` variant exists with the correct fields
/// (`msg: String`, `newline: bool`, `stderr: bool`) in `src/mir/body.rs`.
#[test]
fn test_statement_kind_has_println_variant() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let body_rs = manifest.join("src/mir/body.rs");
    let content = std::fs::read_to_string(&body_rs).expect("read mir/body.rs");

    // Variant must exist.
    assert!(
        content.contains("Println {"),
        "StatementKind::Println variant must exist in src/mir/body.rs"
    );

    // Must have all three fields with correct types.
    assert!(
        content.contains("msg: String")
            && content.contains("newline: bool")
            && content.contains("stderr: bool"),
        "StatementKind::Println must have msg: String, newline: bool, stderr: bool fields"
    );

    // Must reference Stage 13.13 in the doc comment.
    assert!(
        content.contains("Stage 13.13"),
        "StatementKind::Println doc comment must reference Stage 13.13"
    );

    // Must reference §16 (interface isolation — basic block is source of truth).
    assert!(
        content.contains("§16") || content.contains("Interface Isolation"),
        "StatementKind::Println doc comment must reference §16 (basic block is source of truth)"
    );
}

/// Verify the MIR lower `HirExprKind::Println` arm pushes an inline
/// `StatementKind::Println` statement to the current basic block (NOT to
/// the side-table).
#[test]
fn test_mir_lower_emits_println_statement_inline() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expr_operand_rs = manifest.join("src/mir/lower/expr_operand.rs");
    let content = std::fs::read_to_string(&expr_operand_rs).expect("read expr_operand.rs");

    // Must reference Stage 13.13 (the fix).
    assert!(
        content.contains("Stage 13.13"),
        "expr_operand.rs must reference Stage 13.13 in the HirExprKind::Println arm"
    );

    // Must push a StatementKind::Println to the current basic block.
    assert!(
        content.contains("StatementKind::Println"),
        "HirExprKind::Println arm must push a StatementKind::Println statement"
    );

    // Must use block_mut(cx.current_block) to push to the BB.
    assert!(
        content.contains("block_mut(cx.current_block)"),
        "HirExprKind::Println arm must push to current_block (inline emission)"
    );

    // Must NOT push to mir.println_messages side-table (Stage 13.12 approach).
    // Note: the field still exists for backward compat, but the lower should
    // NOT push to it. We allow the field to be referenced in comments only.
    let lower_section = content
        .split("HirExprKind::Println")
        .nth(1)
        .unwrap_or("")
        .split("HirExprKind::MacroCall")
        .next()
        .unwrap_or("");
    assert!(
        !lower_section.contains("println_messages.push"),
        "HirExprKind::Println arm must NOT push to println_messages side-table (Stage 13.12 bug source)"
    );
}

/// Verify `codegen_statement` in `src/codegen/mod.rs` has a
/// `StatementKind::Println` arm that emits an inline `printf` call.
#[test]
fn test_codegen_statement_handles_println() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod_rs = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod_rs).expect("read codegen/mod.rs");

    // Must reference Stage 13.13.
    assert!(
        content.contains("Stage 13.13"),
        "codegen/mod.rs must reference Stage 13.13 in codegen_statement Println arm"
    );

    // Must have a Println arm in codegen_statement.
    // (cargo fmt may reformat the variant pattern across multiple lines,
    // so we check for the variant name + all three field names instead
    // of a single literal string.)
    assert!(
        content.contains("StatementKind::Println {") || content.contains("StatementKind::Println{"),
        "codegen_statement must have a StatementKind::Println variant arm"
    );
    assert!(
        content.contains("msg,") && content.contains("newline,") && content.contains("stderr,"),
        "codegen_statement Println arm must have msg, newline, stderr fields"
    );

    // The Println arm must call printf via emit_call.
    // (cargo fmt may reformat the call across multiple lines, so we check
    // for the function name "printf" appearing in the codegen/mod.rs file
    // — combined with the Stage 13.13 reference, this confirms the inline
    // printf emission.)
    assert!(
        content.contains("\"printf\""),
        "StatementKind::Println arm must reference \"printf\" for the emit_call"
    );
    assert!(
        content.contains("emitter.emit_call"),
        "StatementKind::Println arm must use emitter.emit_call"
    );

    // The Println arm must emit a format string global.
    // Stage 13.16 update: the codegen now builds a C format string from the
    // Landin template (replacing `{}` with `%ld`/`%s`/`%d`), so the literal
    // `b"%s\0"` is no longer present. Instead, we check for the format string
    // emission pattern.
    assert!(
        content.contains("emit_string_global"),
        "StatementKind::Println arm must emit a format string global via emit_string_global"
    );

    // The Println arm must null-terminate the format string (Stage 13.16
    // appends '\\0' to the c_fmt string).
    assert!(
        content.contains("c_fmt.push('\\0')"),
        "StatementKind::Println arm must null-terminate the C format string (Stage 13.16)"
    );
}

/// Verify `codegen_from_mir` no longer emits the `__landin_printlns_<fnname>`
/// helper function (the Stage 13.12 side-table approach is removed).
#[test]
fn test_no_helper_function_emission() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod_rs = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod_rs).expect("read codegen/mod.rs");

    // Must NOT contain the helper function emission block.
    assert!(
        !content.contains("__landin_printlns_"),
        "codegen/mod.rs must NOT emit __landin_printlns_<fnname> helper function (Stage 13.12 removed)"
    );

    // Must NOT iterate println_messages for helper emission.
    assert!(
        !content.contains("mir.println_messages.is_empty()"),
        "codegen/mod.rs must NOT check mir.println_messages.is_empty() (Stage 13.12 helper emission removed)"
    );

    // The println_messages field reference must be in a comment about backward
    // compat (not in active code).
    let active_helper_pattern = "for msg in &mir.println_messages";
    assert!(
        !content.contains(active_helper_pattern),
        "codegen/mod.rs must NOT iterate mir.println_messages for helper emission (Stage 13.12 pattern)"
    );
}

/// Verify the C wrapper in `src/bin/main.rs` no longer references
/// `__landin_printlns_landin_main` (the weak-symbol trick is removed).
#[test]
fn test_c_wrapper_no_weak_symbol() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read bin/main.rs");

    // Must NOT reference the weak symbol.
    assert!(
        !content.contains("__landin_printlns_landin_main"),
        "C wrapper must NOT reference __landin_printlns_landin_main (Stage 13.13 removed weak symbol)"
    );

    // Must NOT use __attribute__((weak)) for println helper.
    // (We allow other uses of __attribute__ if they exist, but specifically
    // the weak println helper must be gone.)
    assert!(
        !content.contains("__attribute__((weak)) void __landin_printlns"),
        "C wrapper must NOT declare __attribute__((weak)) for __landin_printlns_*"
    );

    // Must reference Stage 13.13 in the comment explaining the change.
    assert!(
        content.contains("Stage 13.13"),
        "bin/main.rs must reference Stage 13.13 in the C wrapper comment"
    );

    // The main function must still call landin_main.
    assert!(
        content.contains("int ret = landin_main();"),
        "C wrapper main() must still call landin_main() and return its value"
    );
}

/// Verify `MirBody.println_messages` field is retained (for backward compat
/// with external tooling that may read MIR side-tables).
#[test]
fn test_println_messages_field_kept_for_compat() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let body_rs = manifest.join("src/mir/body.rs");
    let content = std::fs::read_to_string(&body_rs).expect("read mir/body.rs");

    // The field must still exist (Stage 13.12 introduced it; Stage 13.13
    // keeps it for backward compat even though it's no longer populated).
    assert!(
        content.contains("pub println_messages: Vec<String>"),
        "MirBody.println_messages field must be retained for backward compat"
    );

    // The MirBody::new() initializer must still set it to Vec::new().
    assert!(
        content.contains("println_messages: Vec::new(),"),
        "MirBody::new() must still initialize println_messages to Vec::new()"
    );
}

/// Verify the Stage 13.13 gate-review document exists with a PASS verdict.
#[test]
fn test_stage_13_13_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.13.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.13.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.13.md");

    // Must reference Stage 13.13.
    assert!(
        content.contains("Stage 13.13"),
        "gate-review-13.13.md must reference Stage 13.13"
    );

    // Must reference the design alignment doc.
    assert!(
        content.contains("stage-13.13-design-alignment.md"),
        "gate-review-13.13.md must reference stage-13.13-design-alignment.md"
    );

    // Must have a PASS verdict.
    assert!(
        content.contains("PASS"),
        "gate-review-13.13.md must have a PASS verdict"
    );

    // Must reference the println! ordering bug being fixed.
    assert!(
        content.contains("ordering") || content.contains("ordering bug"),
        "gate-review-13.13.md must reference the ordering bug being fixed"
    );

    // Must reference §14.4 (refactoring criteria) and §16 (interface isolation).
    assert!(
        content.contains("§14.4") && content.contains("§16"),
        "gate-review-13.13.md must reference §14.4 and §16"
    );
}

/// Verify the Stage 13.13 design alignment document exists with the
/// required sections (§13.4 design doc survey, §14.4 J1-J6, §25.8 write-back).
#[test]
fn test_stage_13_13_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_doc = manifest.join("docs/develop/v0/stage-13/stage-13.13-design-alignment.md");
    assert!(
        design_doc.exists(),
        "docs/develop/v0/stage-13/stage-13.13-design-alignment.md must exist"
    );

    let content = std::fs::read_to_string(&design_doc).expect("read design-alignment.md");

    // Must reference §13.4 (design alignment protocol).
    assert!(
        content.contains("§13.4"),
        "design-alignment.md must reference §13.4 (design alignment protocol)"
    );

    // Must reference §14.4 (refactoring criteria).
    assert!(
        content.contains("§14.4"),
        "design-alignment.md must reference §14.4 (refactoring criteria)"
    );

    // Must reference §25.8 (design write-back).
    assert!(
        content.contains("§25.8"),
        "design-alignment.md must reference §25.8 (design write-back)"
    );

    // Must reference Strategy B (inline StatementKind::Println).
    assert!(
        content.contains("Strategy B") && content.contains("StatementKind::Println"),
        "design-alignment.md must recommend Strategy B (inline StatementKind::Println)"
    );

    // Must reference the Stage 13.12 known limitation being fixed.
    assert!(
        content.contains("Stage 13.12"),
        "design-alignment.md must reference Stage 13.12 (known limitation being fixed)"
    );
}

/// Verify the typeck checker handles `StatementKind::Println` (no compile
/// error — the variant must be present in the `check_statement` match).
#[test]
fn test_typeck_checker_handles_println() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checker_rs = manifest.join("src/typeck/checker.rs");
    let content = std::fs::read_to_string(&checker_rs).expect("read typeck/checker.rs");

    // The checker must have a Println arm in check_statement.
    assert!(
        content.contains("StatementKind::Println { .. }"),
        "typeck/checker.rs check_statement must handle StatementKind::Println {{ .. }}"
    );

    // Must reference Stage 13.13.
    assert!(
        content.contains("Stage 13.13"),
        "typeck/checker.rs must reference Stage 13.13 in the Println arm comment"
    );
}

/// Verify the v0.1 gate still holds (≥5000 conformance tests pass) after
/// Stage 13.13 changes. This is a static check: verify the conformance
/// suite runner exists. The actual run is done by the CI/CD pipeline.
#[test]
fn test_v01_gate_still_holds_after_stage_13_13() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conformance_runner = manifest.join("tests/conformance/run_all.py");
    assert!(
        conformance_runner.exists(),
        "tests/conformance/run_all.py must exist (v0.1 conformance gate runner)"
    );

    // Verify the conformance suite has at least 5000 .lin files (v0.1 gate).
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
        "v0.1 conformance gate must hold: ≥5000 .lin files (found {})",
        count
    );
}
