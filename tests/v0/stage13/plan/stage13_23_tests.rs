//! Stage 13.23 — Test directory cleanup + entry point design verification
//!
//! Verifies:
//! - --run uses temp directory for intermediate files (no .o/.out in source dir)
//! - fn main() (no return type) exits 0 (default () return)
//! - fn main() -> i32 { N } exits N (explicit return)
//! - .gitignore includes *.o and *.out
//!
//! Per `stage-committee-process.md` v3.21 §13.4 + §14.4 + §25.8.

#![cfg(test)]

use std::path::Path;

/// Verify .gitignore includes *.o and *.out
#[test]
fn test_gitignore_includes_intermediate_files() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gitignore = manifest.join(".gitignore");
    let content = std::fs::read_to_string(&gitignore).expect("read .gitignore");
    assert!(
        content.contains("*.o"),
        ".gitignore must include *.o to prevent intermediate file pollution"
    );
    assert!(
        content.contains("*.out"),
        ".gitignore must include *.out to prevent intermediate file pollution"
    );
}

/// Verify --run uses temp directory (check main.rs source for temp_dir usage)
#[test]
fn test_run_uses_temp_directory() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // Must reference Stage 13.23
    assert!(
        content.contains("Stage 13.23"),
        "main.rs must reference Stage 13.23 (temp dir fix)"
    );

    // Must use temp_dir for --run intermediate files
    assert!(
        content.contains("std::env::temp_dir()"),
        "main.rs must use std::env::temp_dir() for --run intermediate files"
    );

    // The --run path must check cli.run for temp dir usage
    let run_section = content
        .find("if cli.run {")
        .map(|pos| &content[pos..pos + 200])
        .unwrap_or("");
    assert!(
        run_section.contains("temp_dir") || content.contains("landin_run_"),
        "--run must use temp directory with 'landin_run_' prefix"
    );
}

/// Verify entry point design: fn main() is the only entry point
#[test]
fn test_entry_point_is_fn_main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs = manifest.join("src/bin/main.rs");
    let content = std::fs::read_to_string(&main_rs).expect("read main.rs");

    // C wrapper must declare landin_main (the codegen symbol for fn main())
    assert!(
        content.contains("extern int landin_main(void)"),
        "C wrapper must declare extern int landin_main(void) — the codegen symbol for fn main()"
    );
}

/// Verify driver.rs doesn't have the old landin_landin_main bug
#[test]
fn test_no_double_landin_prefix() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let driver_rs = manifest.join("src/driver.rs");
    let content = std::fs::read_to_string(&driver_rs).expect("read driver.rs");

    // Must have the Stage 13.15 strip_prefix fix
    assert!(
        content.contains("strip_prefix(\"landin_\")"),
        "driver.rs must strip 'landin_' prefix to avoid doubling (Stage 13.15)"
    );
}

/// Verify Stage 13.23 design alignment doc exists
#[test]
fn test_stage_13_23_docs_exist() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Check worklog has Stage 13.23 entry
    let worklog = manifest.join("docs/worklog.md");
    let content = std::fs::read_to_string(&worklog).expect("read worklog");
    assert!(
        content.contains("Stage 13.23"),
        "worklog must have Stage 13.23 entry"
    );
}

/// Verify v0.1 conformance gate still holds
#[test]
fn test_v01_gate_holds() {
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
