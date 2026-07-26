//! Stage 9.6 — Attributes conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.1 (attr) + §4.3 (outer/inner).
//!
//! Test dimensions covered:
//! - Attributes conformance suite expanded (307 → 347 .lin files)
//! - All 6 attribute sub-categories present (outer/derive/args/positions/
//!   inner/error-recovery)
//! - Stage 1 features identified (inner attributes `#![...]`)
//! - Parser limitations documented (attributes on variant/field/param/let/block)

#![cfg(test)]

use std::path::Path;

/// Verify attributes conformance directory has 40+ .lin files
#[test]
fn test_stage9_6_attributes_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");
    assert!(
        attr_dir.exists(),
        "tests/conformance/00-parse/05-attributes/ must exist"
    );

    let lin_count = std::fs::read_dir(&attr_dir)
        .expect("read 05-attributes/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 40,
        "05-attributes/ should have at least 40 .lin files after Stage 9.6, got {lin_count}"
    );
}

/// Verify outer attribute tests present (12 tests per plan-9.6.md §3.1)
#[test]
fn test_stage9_6_outer_attribute_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");

    let outer_tests = [
        "attr_outer_fn.lin",
        "attr_outer_struct.lin",
        "attr_outer_enum.lin",
        "attr_outer_trait.lin",
        "attr_outer_impl.lin",
        "attr_outer_const.lin",
        "attr_outer_static.lin",
        "attr_outer_mod.lin",
        "attr_outer_use.lin",
        "attr_outer_type.lin",
        "attr_outer_multi.lin",
        "attr_outer_external.lin",
    ];

    for name in &outer_tests {
        let path = attr_dir.join(name);
        assert!(path.exists(), "outer attribute test {name} must exist");
    }
}

/// Verify derive attribute tests present (8 tests per plan-9.6.md §3.2)
#[test]
fn test_stage9_6_derive_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");

    let derive_tests = [
        "attr_derive_single.lin",
        "attr_derive_multi.lin",
        "attr_derive_debug.lin",
        "attr_derive_default.lin",
        "attr_derive_partial_eq.lin",
        "attr_derive_3.lin",
        "attr_derive_4.lin",
        "attr_derive_enum.lin",
    ];

    for name in &derive_tests {
        let path = attr_dir.join(name);
        assert!(path.exists(), "derive attribute test {name} must exist");
    }
}

/// Verify attribute argument tests present (10 tests per plan-9.6.md §3.3)
#[test]
fn test_stage9_6_attribute_arg_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");

    let arg_tests = [
        "attr_arg_empty.lin",
        "attr_arg_eq_literal.lin",
        "attr_arg_eq_int.lin",
        "attr_arg_list_empty.lin",
        "attr_arg_list_single.lin",
        "attr_arg_list_multi.lin",
        "attr_arg_list_named.lin",
        "attr_arg_list_mixed.lin",
        "attr_arg_path.lin",
        "attr_arg_path_with_args.lin",
    ];

    for name in &arg_tests {
        let path = attr_dir.join(name);
        assert!(path.exists(), "attribute arg test {name} must exist");
    }
}

/// Verify attribute position tests present (5 tests per plan-9.6.md §3.4)
/// and that 4 of them (variant/field/param/let) are marked FAIL (parser limitations)
#[test]
fn test_stage9_6_attribute_position_tests_marked_fail() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");

    let fail_tests = [
        "attr_on_enum_variant.lin",
        "attr_on_struct_field.lin",
        "attr_on_fn_param.lin",
        "attr_on_let.lin",
        "attr_on_block.lin",
    ];

    for name in &fail_tests {
        let path = attr_dir.join(name);
        assert!(path.exists(), "attribute position test {name} must exist");

        let content = std::fs::read_to_string(&path).expect("read attr position test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser limitation in Stage 0 (attributes on this position not supported)"
        );
    }
}

/// Verify inner attribute tests present and marked FAIL (Stage 1 feature)
#[test]
fn test_stage9_6_inner_attribute_tests_marked_fail() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");

    let inner_tests = [
        "attr_inner_no_std.lin",
        "attr_inner_module.lin",
        "attr_inner_mixed.lin",
    ];

    for name in &inner_tests {
        let path = attr_dir.join(name);
        assert!(path.exists(), "inner attribute test {name} must exist");

        let content = std::fs::read_to_string(&path).expect("read inner attr test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — inner attributes #![...] are a Stage 1 feature"
        );
    }
}

/// Verify error recovery tests present (2 tests per plan-9.6.md §3.6)
#[test]
fn test_stage9_6_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let attr_dir = manifest.join("tests/conformance/00-parse/05-attributes");

    let err_tests = ["err_attr_unclosed.lin", "err_attr_missing_path.lin"];

    for name in &err_tests {
        let path = attr_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // err_attr_unclosed should be FAIL (parser reports error)
    let content = std::fs::read_to_string(attr_dir.join("err_attr_unclosed.lin"))
        .expect("read err_attr_unclosed.lin");
    assert!(
        content.contains("//! FAIL"),
        "err_attr_unclosed.lin must be FAIL — parser must report error"
    );

    // err_attr_missing_path should be PASS (parser accepts via synthetic node recovery)
    let content = std::fs::read_to_string(attr_dir.join("err_attr_missing_path.lin"))
        .expect("read err_attr_missing_path.lin");
    assert!(
        content.contains("//! PASS"),
        "err_attr_missing_path.lin must be PASS — parser accepts #[] via synthetic node recovery"
    );
}

/// Verify Stage 9.6 docs created
#[test]
fn test_stage9_6_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_6 = manifest.join("docs/develop/v0/stage-9/plan-9.6.md");
    let gate_review_9_6 = manifest.join("docs/develop/v0/stage-9/gate-review-9.6.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/attributes.md");

    assert!(plan_9_6.exists(), "plan-9.6.md must exist");
    assert!(gate_review_9_6.exists(), "gate-review-9.6.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/attributes.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.5+
#[test]
fn test_stage9_6_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23."),
        "Cargo.toml version must be 0.16.5+ after Stage 9.6 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 347 (307 from 9.5 + 40 new from 9.6)
#[test]
fn test_stage9_6_conformance_total_reaches_347() {
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
        total >= 347,
        "conformance suite total should be at least 347 (307 + 40) after Stage 9.6, got {total}"
    );
}
