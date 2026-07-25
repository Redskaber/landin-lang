//! Stage 9.4 — Patterns conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.5 (Pattern).
//!
//! Test dimensions covered:
//! - Patterns conformance suite expanded (177 → 247 .lin files)
//! - All 12 pattern sub-categories present (wildcard/ident/literal/struct/
//!   tuple/or/range/array/ref/at-binding/path/error-recovery)
//! - Parser limitations documented (negative literal in match, nested ref)

#![cfg(test)]

use std::path::Path;

/// Verify patterns conformance directory has 70+ .lin files
#[test]
fn test_stage9_4_patterns_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");
    assert!(
        pat_dir.exists(),
        "tests/conformance/00-parse/03-patterns/ must exist"
    );

    let lin_count = std::fs::read_dir(&pat_dir)
        .expect("read 03-patterns/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 71,
        "03-patterns/ should have at least 71 .lin files (1 existing + 70 new) after Stage 9.4, got {lin_count}"
    );
}

/// Verify wildcard pattern tests present (5 tests per plan-9.4.md §3.1)
#[test]
fn test_stage9_4_wildcard_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let wild_tests = [
        "pat_wild_basic.lin",
        "pat_wild_in_match.lin",
        "pat_wild_in_fn_param.lin",
        "pat_wild_underscore_prefix.lin",
        "pat_wild_in_closure.lin",
    ];

    for name in &wild_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "wildcard test {name} must exist");
    }
}

/// Verify identifier pattern tests present (6 tests per plan-9.4.md §3.2)
#[test]
fn test_stage9_4_identifier_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let ident_tests = [
        "pat_ident_basic.lin",
        "pat_ident_in_match.lin",
        "pat_ident_in_fn_param.lin",
        "pat_mut_ident.lin",
        "pat_ref_ident.lin",
        "pat_ref_mut_ident.lin",
    ];

    for name in &ident_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "identifier test {name} must exist");
    }
}

/// Verify literal pattern tests present (10 tests per plan-9.4.md §3.3)
#[test]
fn test_stage9_4_literal_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let lit_tests = [
        "pat_lit_int.lin",
        "pat_lit_int_neg.lin",
        "pat_lit_float.lin",
        "pat_lit_bool.lin",
        "pat_lit_char.lin",
        "pat_lit_string.lin",
        "pat_lit_hex.lin",
        "pat_lit_oct.lin",
        "pat_lit_bin.lin",
        "pat_lit_multi.lin",
    ];

    for name in &lit_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "literal test {name} must exist");
    }

    // pat_lit_int_neg should be FAIL (parser limitation)
    let content = std::fs::read_to_string(pat_dir.join("pat_lit_int_neg.lin"))
        .expect("read pat_lit_int_neg.lin");
    assert!(
        content.contains("//! FAIL"),
        "pat_lit_int_neg.lin must be FAIL — parser limitation in Stage 0"
    );
}

/// Verify struct pattern tests present (8 tests per plan-9.4.md §3.4)
#[test]
fn test_stage9_4_struct_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let struct_tests = [
        "pat_struct_basic.lin",
        "pat_struct_with_type.lin",
        "pat_struct_partial.lin",
        "pat_struct_empty.lin",
        "pat_struct_nested.lin",
        "pat_struct_in_match.lin",
        "pat_struct_full.lin",
        "pat_struct_in_let.lin",
    ];

    for name in &struct_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "struct test {name} must exist");
    }
}

/// Verify tuple pattern tests present (8 tests per plan-9.4.md §3.5)
#[test]
fn test_stage9_4_tuple_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let tuple_tests = [
        "pat_tuple_basic.lin",
        "pat_tuple_3.lin",
        "pat_tuple_nested.lin",
        "pat_tuple_with_wild.lin",
        "pat_tuple_in_match.lin",
        "pat_tuple_empty.lin",
        "pat_tuple_single.lin",
        "pat_tuple_multi_wild.lin",
    ];

    for name in &tuple_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "tuple test {name} must exist");
    }
}

/// Verify or-pattern tests present (7 tests per plan-9.4.md §3.6)
#[test]
fn test_stage9_4_or_pattern_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let or_tests = [
        "pat_or_2.lin",
        "pat_or_3.lin",
        "pat_or_4.lin",
        "pat_or_idents.lin",
        "pat_or_mixed.lin",
        "pat_or_paths.lin",
        "pat_or_tuples.lin",
    ];

    for name in &or_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "or-pattern test {name} must exist");
    }
}

/// Verify range pattern tests present (7 tests per plan-9.4.md §3.7)
#[test]
fn test_stage9_4_range_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let range_tests = [
        "pat_range_inclusive.lin",
        "pat_range_exclusive.lin",
        "pat_range_char.lin",
        "pat_range_neg.lin",
        "pat_range_multi.lin",
        "pat_range_or.lin",
        "pat_range_with_at.lin",
    ];

    for name in &range_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "range test {name} must exist");
    }

    // pat_range_neg should be FAIL (parser limitation)
    let content =
        std::fs::read_to_string(pat_dir.join("pat_range_neg.lin")).expect("read pat_range_neg.lin");
    assert!(
        content.contains("//! FAIL"),
        "pat_range_neg.lin must be FAIL — parser limitation in Stage 0"
    );
}

/// Verify array pattern tests present (5 tests per plan-9.4.md §3.8)
#[test]
fn test_stage9_4_array_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let array_tests = [
        "pat_array_basic.lin",
        "pat_array_with_wild.lin",
        "pat_array_rest.lin",
        "pat_array_empty.lin",
        "pat_array_nested.lin",
    ];

    for name in &array_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "array test {name} must exist");
    }
}

/// Verify reference pattern tests present (5 tests per plan-9.4.md §3.9)
#[test]
fn test_stage9_4_reference_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let ref_tests = [
        "pat_ref_basic.lin",
        "pat_ref_mut_basic.lin",
        "pat_ref_nested.lin",
        "pat_ref_tuple.lin",
        "pat_ref_struct.lin",
    ];

    for name in &ref_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "reference test {name} must exist");
    }

    // pat_ref_nested should be FAIL (parser limitation)
    let content = std::fs::read_to_string(pat_dir.join("pat_ref_nested.lin"))
        .expect("read pat_ref_nested.lin");
    assert!(
        content.contains("//! FAIL"),
        "pat_ref_nested.lin must be FAIL — parser only supports single & in Stage 0"
    );
}

/// Verify at-binding pattern tests present (3 tests per plan-9.4.md §3.10)
#[test]
fn test_stage9_4_at_binding_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let at_tests = ["pat_at_basic.lin", "pat_at_range.lin", "pat_at_or.lin"];

    for name in &at_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "at-binding test {name} must exist");
    }
}

/// Verify path pattern tests present (3 tests per plan-9.4.md §3.11)
#[test]
fn test_stage9_4_path_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let path_tests = [
        "pat_path_enum.lin",
        "pat_path_enum_with_data.lin",
        "pat_path_enum_struct.lin",
    ];

    for name in &path_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "path test {name} must exist");
    }
}

/// Verify error recovery tests present (3 tests per plan-9.4.md §3.12)
#[test]
fn test_stage9_4_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pat_dir = manifest.join("tests/conformance/00-parse/03-patterns");

    let err_tests = [
        "err_pat_missing_pattern.lin",
        "err_pat_at_no_pat.lin",
        "err_pat_unclosed_paren.lin",
    ];

    for name in &err_tests {
        let path = pat_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser must report error"
        );
    }
}

/// Verify Stage 9.4 docs created
#[test]
fn test_stage9_4_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_4 = manifest.join("docs/develop/v0/stage-9/plan-9.4.md");
    let gate_review_9_4 = manifest.join("docs/develop/v0/stage-9/gate-review-9.4.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/patterns.md");

    assert!(plan_9_4.exists(), "plan-9.4.md must exist");
    assert!(gate_review_9_4.exists(), "gate-review-9.4.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/patterns.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.3+
#[test]
fn test_stage9_4_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.16.3")
            || version_line.starts_with("version = \"0.16.4")
            || version_line.starts_with("version = \"0.16.5")
            || version_line.starts_with("version = \"0.16.6")
            || version_line.starts_with("version = \"0.16.7")
            || version_line.starts_with("version = \"0.16.8")
            || version_line.starts_with("version = \"0.16.9")
            || version_line.starts_with("version = \"0.16.10")
            || version_line.starts_with("version = \"0.16.11")
            || version_line.starts_with("version = \"0.16.12")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20."),
        "Cargo.toml version must be 0.16.3+ after Stage 9.4 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 247 (177 from 9.3 + 70 new from 9.4)
#[test]
fn test_stage9_4_conformance_total_reaches_247() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parse_dir = manifest.join("tests/conformance/00-parse");

    let mut total = 0;
    for entry in std::fs::read_dir(&parse_dir).expect("read 00-parse/") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            total += std::fs::read_dir(&path)
                .expect("read category dir")
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
                .count();
        }
    }

    assert!(
        total >= 247,
        "conformance suite total should be at least 247 (177 + 70) after Stage 9.4, got {total}"
    );
}
