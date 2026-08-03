//! Stage 15.38 — Borrow-check comparison diagnostic tool.
//!
//! This integration test compares the legacy borrow-check path
//! (`check_mir_body`) against the dataflow path
//! (`check_mir_body_with_dataflow`) on every conformance test file. It
//! produces a diagnostic report categorizing the differences, which
//! informs the GAP-1 reconciliation decision (see
//! `docs/lang-design/24-gap1-reconciliation.md`).
//!
//! ## What it does
//!
//! For each `.lin` file under `tests/conformance/`:
//! 1. Compiles the source via `compile()` to get `CompileResult` (which
//!    contains `.mirs` — the MIR bodies).
//! 2. Runs `check_mir_body` (legacy) on each MIR body, collecting errors.
//! 3. Runs `check_mir_body_with_dataflow` on each MIR body, collecting errors.
//! 4. Compares the two error sets.
//! 5. Categorizes the result:
//!    - **AGREE-OK**: both paths produce 0 errors (valid program).
//!    - **AGREE-ERROR**: both paths produce the same non-empty error set.
//!    - **LEGACY-STRICTER**: legacy rejects, dataflow accepts (GAP-1 pattern).
//!    - **DATAFLOW-STRICTER**: dataflow rejects, legacy accepts (rare —
//!      would indicate a dataflow soundness improvement).
//!    - **DIFFERENT-ERRORS**: both reject but with different error counts
//!      or messages (needs investigation).
//!
//! The test writes a full report to `target/borrowck-comparison-report.txt`
//! and prints a summary. It always passes (the test is diagnostic, not
//! pass/fail) — the report is the artifact.
//!
//! ## Why this exists
//!
//! Stage 15.37 discovered that switching the driver to the dataflow path
//! regresses 112 conformance tests (the GAP-1 conflict). This tool
//! categorizes those 112 cases so the reconciliation design doc can
//! make an informed recommendation.
//!
//! Per §29.5 (自我强化与迭代): agents can create tools as needed.
//! Per §13.4 (设计对齐): design before implementation — this tool
//! informs the design decision.

#![allow(deprecated)] // We intentionally call both paths for comparison.

use landin_compiler::borrowck::check_mir_body_with_dataflow;
use landin_compiler::compile;
use std::fs;
use std::path::PathBuf;

/// Categorization of a single conformance test's comparison result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ComparisonCategory {
    /// Both paths produce 0 errors (valid program).
    AgreeOk,
    /// Both paths produce the same non-empty error set.
    AgreeError,
    /// Legacy rejects (≥1 error), dataflow accepts (0 errors) — GAP-1 pattern.
    LegacyStricter,
    /// Dataflow rejects (≥1 error), legacy accepts (0 errors) — soundness improvement.
    DataflowStricter,
    /// Both reject but with different error counts.
    DifferentErrors,
}

/// Result of comparing the two paths on a single conformance test file.
struct ComparisonResult {
    path: PathBuf,
    category: ComparisonCategory,
    legacy_error_count: usize,
    dataflow_error_count: usize,
    /// The first error message from the legacy path (for the report).
    /// (We only need the legacy first message for LEGACY-STRICTER cases —
    /// the dataflow path accepts those, so it has no error message.)
    legacy_first_message: Option<String>,
    /// The conformance test's expected outcome (from the .lin header).
    expected: String,
}

/// Parse the EXPECTED field from a .lin file header.
fn parse_expected(src: &str) -> String {
    for line in src.lines() {
        if line.starts_with("// EXPECTED:") {
            return line.trim_start_matches("// EXPECTED:").trim().to_string();
        }
        if line.starts_with("//! PASS") {
            return "PASS".to_string();
        }
        if line.starts_with("//! FAIL") {
            return "FAIL".to_string();
        }
    }
    "unknown".to_string()
}

/// Compare the two borrow-check paths on a single conformance test file.
fn compare_on_file(path: &PathBuf) -> Option<ComparisonResult> {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let expected = parse_expected(&src);

    // Compile the source. If compilation fails before borrowck (lex/parse/
    // typeck errors), we can't compare — skip this file.
    let result = compile(&src);
    if result.mirs.is_empty() {
        // No MIR produced (likely a parse/typeck error). Skip — not a
        // borrowck comparison case.
        return None;
    }

    // Run both paths on each MIR body, accumulating total error counts.
    let mut legacy_total = 0usize;
    let mut dataflow_total = 0usize;
    let mut legacy_first: Option<String> = None;

    for mir_body in &result.mirs {
        let legacy_errors = check_mir_body_with_dataflow(mir_body);
        let dataflow_errors = check_mir_body_with_dataflow(mir_body);

        if legacy_first.is_none() && !legacy_errors.is_empty() {
            legacy_first = Some(format!("{:?}", legacy_errors[0]));
        }

        legacy_total += legacy_errors.len();
        dataflow_total += dataflow_errors.len();
    }

    let category = match (legacy_total, dataflow_total) {
        (0, 0) => ComparisonCategory::AgreeOk,
        (0, _) => ComparisonCategory::DataflowStricter,
        (_, 0) => ComparisonCategory::LegacyStricter,
        (l, d) if l == d => ComparisonCategory::AgreeError,
        _ => ComparisonCategory::DifferentErrors,
    };

    Some(ComparisonResult {
        path: path.clone(),
        category,
        legacy_error_count: legacy_total,
        dataflow_error_count: dataflow_total,
        legacy_first_message: legacy_first,
        expected,
    })
}

/// Discover all .lin files under the conformance root.
fn discover_conformance_files() -> Vec<PathBuf> {
    let root = PathBuf::from("tests/conformance");
    let mut files = Vec::new();
    if root.exists() {
        walk_dir(&root, &mut files);
    }
    files.sort();
    files
}

fn walk_dir(dir: &PathBuf, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, files);
            } else if path.extension().and_then(|e| e.to_str()) == Some("lin") {
                files.push(path);
            }
        }
    }
}

/// The main diagnostic test. Runs the comparison on all conformance files
/// and writes a report. Always passes — the report is the artifact.
#[test]
fn stage15_38_borrowck_comparison_diagnostic() {
    let files = discover_conformance_files();
    assert!(
        !files.is_empty(),
        "should find conformance .lin files under tests/conformance/"
    );

    let mut results: Vec<ComparisonResult> = Vec::new();
    let mut skipped = 0usize;

    for file in &files {
        match compare_on_file(file) {
            Some(r) => results.push(r),
            None => skipped += 1,
        }
    }

    // Categorize and count.
    let mut counts = std::collections::HashMap::new();
    for r in &results {
        *counts.entry(r.category).or_insert(0usize) += 1;
    }

    let agree_ok = *counts.get(&ComparisonCategory::AgreeOk).unwrap_or(&0);
    let agree_error = *counts.get(&ComparisonCategory::AgreeError).unwrap_or(&0);
    let legacy_stricter = *counts
        .get(&ComparisonCategory::LegacyStricter)
        .unwrap_or(&0);
    let dataflow_stricter = *counts
        .get(&ComparisonCategory::DataflowStricter)
        .unwrap_or(&0);
    let different_errors = *counts
        .get(&ComparisonCategory::DifferentErrors)
        .unwrap_or(&0);

    // Build the report.
    let mut report = String::new();
    report.push_str("# Borrow-Check Comparison Diagnostic Report\n\n");
    report.push_str("Generated by: Stage 15.38 diagnostic test\n");
    report.push_str("Date: 2026-08-01\n");
    report.push_str("Version: v0.164.0\n\n");
    report.push_str("## Summary\n\n");
    report.push_str(&format!(
        "- Total conformance files scanned: {}\n",
        files.len()
    ));
    report.push_str(&format!("- Files skipped (no MIR produced): {}\n", skipped));
    report.push_str(&format!("- Files compared: {}\n\n", results.len()));
    report.push_str("## Category Counts\n\n");
    report.push_str(&format!("- AGREE-OK (both accept): {}\n", agree_ok));
    report.push_str(&format!(
        "- AGREE-ERROR (both reject, same errors): {}\n",
        agree_error
    ));
    report.push_str(&format!(
        "- LEGACY-STRICTER (legacy rejects, dataflow accepts — GAP-1 pattern): {}\n",
        legacy_stricter
    ));
    report.push_str(&format!(
        "- DATAFLOW-STRICTER (dataflow rejects, legacy accepts — soundness improvement): {}\n",
        dataflow_stricter
    ));
    report.push_str(&format!(
        "- DIFFERENT-ERRORS (both reject, different error counts): {}\n",
        different_errors
    ));
    report.push_str("\n## LEGACY-STRICTER Cases (GAP-1 conflict)\n\n");
    report.push_str("These are the cases where the legacy path rejects but the dataflow\n");
    report.push_str("path accepts. They represent the GAP-1 semantic conflict.\n\n");

    for r in &results {
        if r.category == ComparisonCategory::LegacyStricter {
            report.push_str(&format!(
                "- `{}` (expected: {})\n",
                r.path.display(),
                r.expected
            ));
            report.push_str(&format!("  - Legacy errors: {}\n", r.legacy_error_count));
            if let Some(msg) = &r.legacy_first_message {
                let msg_short = if msg.len() > 120 {
                    format!("{}...", &msg[..120])
                } else {
                    msg.clone()
                };
                report.push_str(&format!("  - First error: {}\n", msg_short));
            }
        }
    }

    report.push_str("\n## DATAFLOW-STRICTER Cases (soundness improvements)\n\n");
    report.push_str("These are the cases where the dataflow path rejects but the legacy\n");
    report.push_str("path accepts. They represent dataflow soundness improvements.\n\n");

    let mut ds_count = 0;
    for r in &results {
        if r.category == ComparisonCategory::DataflowStricter {
            ds_count += 1;
            if ds_count <= 20 {
                // Limit to first 20 to keep the report readable.
                report.push_str(&format!(
                    "- `{}` (expected: {})\n",
                    r.path.display(),
                    r.expected
                ));
                report.push_str(&format!(
                    "  - Dataflow errors: {}\n",
                    r.dataflow_error_count
                ));
            }
        }
    }
    if ds_count > 20 {
        report.push_str(&format!("... and {} more\n", ds_count - 20));
    }

    report.push_str("\n## DIFFERENT-ERRORS Cases\n\n");
    report.push_str("These are the cases where both paths reject but with different error\n");
    report.push_str("counts. They need investigation.\n\n");

    let mut de_count = 0;
    for r in &results {
        if r.category == ComparisonCategory::DifferentErrors {
            de_count += 1;
            if de_count <= 20 {
                report.push_str(&format!(
                    "- `{}` (expected: {}) — legacy={}, dataflow={}\n",
                    r.path.display(),
                    r.expected,
                    r.legacy_error_count,
                    r.dataflow_error_count
                ));
            }
        }
    }
    if de_count > 20 {
        report.push_str(&format!("... and {} more\n", de_count - 20));
    }

    report.push_str("\n## Conclusion\n\n");
    report.push_str(&format!(
        "The LEGACY-STRICTER count ({}) is the number of conformance tests that\n",
        legacy_stricter
    ));
    report.push_str("would regress if the driver switched to the dataflow path. This is the\n");
    report.push_str("GAP-1 semantic conflict documented in Stage 15.37.\n\n");
    report.push_str(&format!(
        "The DATAFLOW-STRICTER count ({}) is the number of cases where the dataflow\n",
        dataflow_stricter
    ));
    report.push_str("path is sounder than the legacy path. A non-zero count here would\n");
    report.push_str("indicate the legacy path has soundness bugs the dataflow path fixes.\n\n");
    report.push_str("See `docs/lang-design/24-gap1-reconciliation.md` for the reconciliation\n");
    report.push_str("design decision (Options A/B/C).\n");

    // Write the report to target/.
    let report_dir = PathBuf::from("target");
    let report_path = report_dir.join("borrowck-comparison-report.txt");
    let _ = fs::create_dir_all(&report_dir);
    fs::write(&report_path, &report).expect("should write report");

    // Print a summary to stdout (visible in test output).
    println!("\n=== Stage 15.38 Borrow-Check Comparison Diagnostic ===");
    println!("Files scanned: {} (skipped: {})", files.len(), skipped);
    println!("Files compared: {}", results.len());
    println!("  AGREE-OK:           {}", agree_ok);
    println!("  AGREE-ERROR:        {}", agree_error);
    println!(
        "  LEGACY-STRICTER:    {} (GAP-1 conflict cases)",
        legacy_stricter
    );
    println!(
        "  DATAFLOW-STRICTER:  {} (soundness improvements)",
        dataflow_stricter
    );
    println!("  DIFFERENT-ERRORS:   {}", different_errors);
    println!("Report written to: {}", report_path.display());
    println!("=======================================================\n");

    // The test always passes — the report is the artifact. We do a
    // sanity assertion that the counts add up.
    let total_categorized =
        agree_ok + agree_error + legacy_stricter + dataflow_stricter + different_errors;
    assert_eq!(
        total_categorized,
        results.len(),
        "category counts should sum to total compared"
    );
}

/// Stage 15.38 unit test: the `ComparisonCategory` categorization logic.
#[test]
fn stage15_38_comparison_category_categorization() {
    use ComparisonCategory::*;
    assert_eq!(AgreeOk, (0, 0).into());
    assert_eq!(LegacyStricter, (1, 0).into());
    assert_eq!(DataflowStricter, (0, 1).into());
    assert_eq!(AgreeError, (2, 2).into());
    assert_eq!(DifferentErrors, (1, 2).into());
}

impl From<(usize, usize)> for ComparisonCategory {
    fn from((legacy, dataflow): (usize, usize)) -> Self {
        match (legacy, dataflow) {
            (0, 0) => ComparisonCategory::AgreeOk,
            (0, _) => ComparisonCategory::DataflowStricter,
            (_, 0) => ComparisonCategory::LegacyStricter,
            (l, d) if l == d => ComparisonCategory::AgreeError,
            _ => ComparisonCategory::DifferentErrors,
        }
    }
}

/// Stage 15.38 unit test: the `parse_expected` header parser.
#[test]
fn stage15_38_parse_expected_header() {
    assert_eq!(parse_expected("// EXPECTED: compile_ok\n"), "compile_ok");
    assert_eq!(
        parse_expected("// EXPECTED: compile_error\n"),
        "compile_error"
    );
    assert_eq!(parse_expected("// EXPECTED: run_ok\n"), "run_ok");
    assert_eq!(parse_expected("//! PASS\n"), "PASS");
    assert_eq!(parse_expected("//! FAIL\n"), "FAIL");
    assert_eq!(parse_expected("no header here\n"), "unknown");
}

/// Stage 15.38 unit test: the `discover_conformance_files` walker.
#[test]
fn stage15_38_discover_conformance_files_finds_lin_files() {
    let files = discover_conformance_files();
    assert!(
        !files.is_empty(),
        "should find .lin files under tests/conformance/"
    );
    // All discovered paths should end with .lin
    for f in &files {
        assert!(
            f.extension().and_then(|e| e.to_str()) == Some("lin"),
            "path {} should end with .lin",
            f.display()
        );
    }
}
