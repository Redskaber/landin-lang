//! Stage 9.7 — Generics conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.2 (generic_params + type_bounds
//! + where_clause).
//!
//! Test dimensions covered:
//! - Generics conformance suite expanded (347 → 397 .lin files)
//! - All 6 generics sub-categories present (type-params/lifetime/bounds/
//!   where-clauses/generic-args/error-recovery)
//! - Parser limitations documented (?Sized, HRTB for<'a>)

#![cfg(test)]

use std::path::Path;

/// Verify generics conformance directory has 50+ .lin files
#[test]
fn test_stage9_7_generics_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");
    assert!(
        gen_dir.exists(),
        "tests/conformance/00-parse/06-generics/ must exist"
    );

    let lin_count = std::fs::read_dir(&gen_dir)
        .expect("read 06-generics/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 50,
        "06-generics/ should have at least 50 .lin files after Stage 9.7, got {lin_count}"
    );
}

/// Verify generic type parameter tests present (12 tests per plan-9.7.md §3.1)
#[test]
fn test_stage9_7_type_param_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");

    let param_tests = [
        "gen_param_single.lin",
        "gen_param_multi.lin",
        "gen_param_3.lin",
        "gen_param_fn.lin",
        "gen_param_impl.lin",
        "gen_param_trait.lin",
        "gen_param_enum.lin",
        "gen_param_type_alias.lin",
        "gen_param_method.lin",
        "gen_param_with_default.lin",
        "gen_param_nested.lin",
        "gen_param_mixed.lin",
    ];

    for name in &param_tests {
        let path = gen_dir.join(name);
        assert!(path.exists(), "type param test {name} must exist");
    }
}

/// Verify lifetime parameter tests present (8 tests per plan-9.7.md §3.2)
#[test]
fn test_stage9_7_lifetime_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");

    let lifetime_tests = [
        "gen_lifetime_basic.lin",
        "gen_lifetime_multi.lin",
        "gen_lifetime_struct.lin",
        "gen_lifetime_impl.lin",
        "gen_lifetime_trait.lin",
        "gen_lifetime_with_type.lin",
        "gen_lifetime_static.lin",
        "gen_lifetime_bounds.lin",
    ];

    for name in &lifetime_tests {
        let path = gen_dir.join(name);
        assert!(path.exists(), "lifetime test {name} must exist");
    }
}

/// Verify type bound tests present (10 tests per plan-9.7.md §3.3)
/// 2 should be FAIL (?Sized and HRTB — parser limitations)
#[test]
fn test_stage9_7_type_bound_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");

    let bound_tests = [
        "gen_bound_single.lin",
        "gen_bound_multi.lin",
        "gen_bound_3.lin",
        "gen_bound_lifetime.lin",
        "gen_bound_mixed.lin",
        "gen_bound_struct.lin",
        "gen_bound_impl.lin",
        "gen_bound_trait.lin",
        "gen_bound_question_sized.lin",
        "gen_bound_for_hrtb.lin",
    ];

    for name in &bound_tests {
        let path = gen_dir.join(name);
        assert!(path.exists(), "type bound test {name} must exist");
    }

    // ?Sized and HRTB should be FAIL (parser limitations)
    for name in &["gen_bound_question_sized.lin", "gen_bound_for_hrtb.lin"] {
        let path = gen_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read bound test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser limitation in Stage 0"
        );
    }
}

/// Verify where clause tests present (10 tests per plan-9.7.md §3.4)
#[test]
fn test_stage9_7_where_clause_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");

    let where_tests = [
        "gen_where_basic.lin",
        "gen_where_multi.lin",
        "gen_where_lifetime.lin",
        "gen_where_mixed.lin",
        "gen_where_struct.lin",
        "gen_where_impl.lin",
        "gen_where_trait.lin",
        "gen_where_multi_bound.lin",
        "gen_where_no_bounds.lin",
        "gen_where_complex.lin",
    ];

    for name in &where_tests {
        let path = gen_dir.join(name);
        assert!(path.exists(), "where clause test {name} must exist");
    }
}

/// Verify generic args tests present (5 tests per plan-9.7.md §3.5)
#[test]
fn test_stage9_7_generic_args_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");

    let args_tests = [
        "gen_args_basic.lin",
        "gen_args_multi.lin",
        "gen_args_nested.lin",
        "gen_args_lifetime.lin",
        "gen_args_mixed.lin",
    ];

    for name in &args_tests {
        let path = gen_dir.join(name);
        assert!(path.exists(), "generic args test {name} must exist");
    }
}

/// Verify error recovery tests present (5 tests per plan-9.7.md §3.6)
#[test]
fn test_stage9_7_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gen_dir = manifest.join("tests/conformance/00-parse/06-generics");

    let err_tests = [
        "err_gen_unclosed.lin",
        "err_gen_no_params.lin",
        "err_gen_bound_no_type.lin",
        "err_gen_where_no_colon.lin",
        "err_gen_double_comma.lin",
    ];

    for name in &err_tests {
        let path = gen_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // err_gen_where_no_colon and err_gen_double_comma should be FAIL (parser errors)
    for name in &["err_gen_where_no_colon.lin", "err_gen_double_comma.lin"] {
        let path = gen_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser must report error"
        );
    }

    // err_gen_unclosed, err_gen_no_params, err_gen_bound_no_type should be PASS (recovery)
    for name in &[
        "err_gen_unclosed.lin",
        "err_gen_no_params.lin",
        "err_gen_bound_no_type.lin",
    ] {
        let path = gen_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! PASS"),
            "{name} must be PASS — parser accepts via synthetic node recovery"
        );
    }
}

/// Verify Stage 9.7 docs created
#[test]
fn test_stage9_7_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_7 = manifest.join("docs/develop/v0/stage-9/plan-9.7.md");
    let gate_review_9_7 = manifest.join("docs/develop/v0/stage-9/gate-review-9.7.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/generics.md");

    assert!(plan_9_7.exists(), "plan-9.7.md must exist");
    assert!(gate_review_9_7.exists(), "gate-review-9.7.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/generics.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.6+
#[test]
fn test_stage9_7_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.16.6")
            || version_line.starts_with("version = \"0.16.7")
            || version_line.starts_with("version = \"0.16.8")
            || version_line.starts_with("version = \"0.16.9")
            || version_line.starts_with("version = \"0.16.10")
            || version_line.starts_with("version = \"0.16.11")
            || version_line.starts_with("version = \"0.16.12")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23."),
        "Cargo.toml version must be 0.16.6+ after Stage 9.7 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 397 (347 from 9.6 + 50 new from 9.7)
#[test]
fn test_stage9_7_conformance_total_reaches_397() {
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
        total >= 397,
        "conformance suite total should be at least 397 (347 + 50) after Stage 9.7, got {total}"
    );
}
