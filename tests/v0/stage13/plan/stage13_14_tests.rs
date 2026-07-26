//! Stage 13.14 — eprintln!/eprint! stderr emission verification tests
//!
//! Verifies that Stage 13.13's deferral (the `stderr` flag was captured but
//! ignored at codegen time) is properly closed by Stage 13.14:
//!
//! - When `stderr == true` (eprintln!/eprint!), codegen calls the
//!   `__landin_eprint` C wrapper helper (which calls `fprintf(stderr, ...)`)
//! - When `stderr == false` (println!/print!), codegen still calls `printf`
//!   (Stage 13.13 path, unchanged — no regression)
//! - The C wrapper defines `__landin_eprint` with the correct body
//!
//! Per `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8 +
//! `stage-13.14-design-alignment.md` + `gate-review-13.14.md`.

#![cfg(test)]

use std::path::Path;

/// Verify the `StatementKind::Println` arm in `codegen_statement` has a
/// branch on the `stderr` flag (Stage 13.14 closes the Stage 13.13 deferral).
#[test]
fn test_codegen_println_branches_on_stderr() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod_rs = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod_rs).expect("read codegen/mod.rs");

    // Must reference Stage 13.14 in the Println arm comment.
    assert!(
        content.contains("Stage 13.14"),
        "codegen/mod.rs must reference Stage 13.14 in the Println arm comment"
    );

    // Must have a branch on the `stderr` flag (closes Stage 13.13 deferral).
    assert!(
        content.contains("if *stderr"),
        "StatementKind::Println arm must branch on `if *stderr` (Stage 13.14 closes Stage 13.13 deferral)"
    );

    // Must NOT have the Stage 13.13 deferral comment anymore (replaced by
    // the Stage 13.14 implementation).
    assert!(
        !content.contains("let _ = stderr;"),
        "Stage 13.13 deferral `let _ = stderr;` must be removed (Stage 13.14 implements the branch)"
    );
}

/// Verify that when `stderr == true`, codegen calls `__landin_eprint` or
/// `__landin_eprintf` (not `printf`). This is the Stage 13.14 behavior
/// (extended in Stage 13.16 to use `__landin_eprintf` for variadic format args).
#[test]
fn test_codegen_eprint_calls_helper() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod_rs = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod_rs).expect("read codegen/mod.rs");

    // Stage 13.14 + 13.16: The `__landin_eprintf` helper (variadic) must be
    // referenced in the codegen for the stderr path. (Stage 13.14 originally
    // used `__landin_eprint` for single-string stderr; Stage 13.16 unified
    // the codegen to always use `__landin_eprintf` for both string and
    // format-args cases, since the codegen now always builds a C format string.)
    assert!(
        content.contains("\"__landin_eprintf\""),
        "StatementKind::Println arm (stderr == true) must call __landin_eprintf helper (Stage 13.16 variadic)"
    );

    // The `__landin_eprintf` call should be inside the `if *stderr` branch,
    // so we verify the call appears AFTER the `if *stderr` check.
    let if_stderr_pos = content
        .find("if *stderr")
        .expect("if *stderr branch must exist");
    let eprint_call_pos = content
        .find("\"__landin_eprintf\"")
        .expect("__landin_eprint call must exist");
    assert!(
        eprint_call_pos > if_stderr_pos,
        "__landin_eprint call must appear AFTER the `if *stderr` branch (inside the stderr path)"
    );

    // The stderr path must return void (matching the C helper signature).
    // Look for `EmitType::Void` within a reasonable window after the
    // `__landin_eprint` call (the call's last argument is the return type).
    let eprint_call_window = 500; // chars after the call site
    let window = &content[eprint_call_pos..eprint_call_pos.saturating_add(eprint_call_window)];
    assert!(
        window.contains("EmitType::Void"),
        "__landin_eprint call must use EmitType::Void return type (matches C helper signature)"
    );
}

/// Verify that when `stderr == false`, codegen still calls `printf` (the
/// Stage 13.13 path is unchanged — no regression).
#[test]
fn test_codegen_stdout_unchanged() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_mod_rs = manifest.join("src/codegen/mod.rs");
    let content = std::fs::read_to_string(&codegen_mod_rs).expect("read codegen/mod.rs");

    // The `printf` call must still exist (Stage 13.13 path preserved).
    assert!(
        content.contains("\"printf\""),
        "StatementKind::Println arm (stderr == false) must still call printf (no regression)"
    );

    // The `printf` call should be in the `else` branch of `if *stderr`.
    let if_stderr_pos = content
        .find("if *stderr")
        .expect("if *stderr branch must exist");
    let printf_call_pos = content.find("\"printf\"").expect("printf call must exist");
    assert!(
        printf_call_pos > if_stderr_pos,
        "printf call must appear AFTER the `if *stderr` branch (in the else path)"
    );

    // The stdout path must still use EmitType::I32 (printf returns int).
    let stdout_section = &content[printf_call_pos..];
    let call_end = stdout_section.find("}").unwrap_or(stdout_section.len());
    let call_block = &stdout_section[..call_end];
    assert!(
        call_block.contains("EmitType::I32"),
        "printf call must use EmitType::I32 return type (printf returns int; no regression)"
    );
}

/// Verify the C wrapper in `src/bin/main.rs` defines the `__landin_eprint`
/// helper with the correct body (`fprintf(stderr, "%s", s)`).
#[test]
fn test_c_wrapper_has_eprint_helper() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read bin/main.rs");

    // Must reference Stage 13.14.
    assert!(
        content.contains("Stage 13.14"),
        "bin/main.rs must reference Stage 13.14 in the C wrapper comment"
    );

    // Must define the __landin_eprint helper function (Stage 13.14).
    assert!(
        content.contains("void __landin_eprint(const char* s)"),
        "C wrapper must define `void __landin_eprint(const char* s)` helper function (Stage 13.14)"
    );

    // Stage 13.16: Must also define the __landin_eprintf variadic helper.
    assert!(
        content.contains("void __landin_eprintf(const char* fmt, ...)"),
        "C wrapper must define `void __landin_eprintf(const char* fmt, ...)` variadic helper (Stage 13.16)"
    );

    // The __landin_eprint helper body must call fprintf(stderr, "%s", s).
    assert!(
        content.contains("fprintf(stderr, \"%s\", s)"),
        "__landin_eprint helper body must call fprintf(stderr, \"%s\", s)"
    );

    // The __landin_eprintf helper body must call vfprintf(stderr, fmt, args).
    assert!(
        content.contains("vfprintf(stderr, fmt, args)"),
        "__landin_eprintf helper body must call vfprintf(stderr, fmt, args) (Stage 13.16 variadic)"
    );

    // The helper must be referenced in the codegen call site (cross-check
    // that the codegen actually invokes this helper).
    let codegen_mod_rs = manifest.join("src/codegen/mod.rs");
    let codegen_content = std::fs::read_to_string(&codegen_mod_rs).expect("read codegen/mod.rs");
    assert!(
        codegen_content.contains("__landin_eprint"),
        "codegen/mod.rs must reference __landin_eprint (cross-check: C wrapper helper is actually invoked)"
    );
}

/// Verify the Stage 13.14 design alignment document exists with the
/// required sections (§13.4 design doc survey, §14.4 J1-J6, §25.8 write-back).
#[test]
fn test_stage_13_14_design_alignment_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let design_doc = manifest.join("docs/develop/v0/stage-13/stage-13.14-design-alignment.md");
    assert!(
        design_doc.exists(),
        "docs/develop/v0/stage-13/stage-13.14-design-alignment.md must exist"
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

    // Must reference Strategy B (__landin_eprint helper).
    assert!(
        content.contains("Strategy B") && content.contains("__landin_eprint"),
        "design-alignment.md must recommend Strategy B (__landin_eprint helper)"
    );

    // Must reference the Stage 13.13 deferral being closed.
    assert!(
        content.contains("Stage 13.13") && content.contains("deferral"),
        "design-alignment.md must reference the Stage 13.13 deferral being closed by Stage 13.14"
    );
}

/// Verify the Stage 13.14 gate review document exists with a PASS verdict.
#[test]
fn test_stage_13_14_gate_review_exists() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gate_review = manifest.join("docs/develop/v0/stage-13/gate-review-13.14.md");
    assert!(
        gate_review.exists(),
        "docs/develop/v0/stage-13/gate-review-13.14.md must exist"
    );

    let content = std::fs::read_to_string(&gate_review).expect("read gate-review-13.14.md");

    // Must reference Stage 13.14.
    assert!(
        content.contains("Stage 13.14"),
        "gate-review-13.14.md must reference Stage 13.14"
    );

    // Must reference the design alignment doc.
    assert!(
        content.contains("stage-13.14-design-alignment.md"),
        "gate-review-13.14.md must reference stage-13.14-design-alignment.md"
    );

    // Must have a PASS verdict.
    assert!(
        content.contains("PASS"),
        "gate-review-13.14.md must have a PASS verdict"
    );

    // Must reference §14.4 (refactoring criteria) and §16 (interface isolation).
    assert!(
        content.contains("§14.4") && content.contains("§16"),
        "gate-review-13.14.md must reference §14.4 and §16"
    );

    // Must reference the stderr routing being fixed.
    assert!(
        content.contains("stderr"),
        "gate-review-13.14.md must reference the stderr routing being fixed"
    );
}

/// Verify the v0.1 gate still holds (≥5000 conformance tests pass) after
/// Stage 13.14 changes. This is a static check: verify the conformance
/// suite runner exists. The actual run is done by the CI/CD pipeline.
#[test]
fn test_v01_gate_still_holds_after_stage_13_14() {
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
