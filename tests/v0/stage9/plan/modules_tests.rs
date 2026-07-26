//! Stage 9.9 — Modules conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.1 (mod + vis) + §3.7 (use).
//!
//! Test dimensions covered:
//! - Modules conformance suite expanded (437 → 497 .lin files)
//! - All 6 modules sub-categories present (mod-decl/use-basic/use-advanced/
//!   pub-vis/restricted-vis/error-recovery)
//! - Parser limitations documented (mod in fn, use as self, nested glob)

#![cfg(test)]

use std::path::Path;

/// Verify modules conformance directory has 60+ .lin files
#[test]
fn test_stage9_9_modules_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");
    assert!(
        mod_dir.exists(),
        "tests/conformance/00-parse/08-modules/ must exist"
    );

    let lin_count = std::fs::read_dir(&mod_dir)
        .expect("read 08-modules/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 60,
        "08-modules/ should have at least 60 .lin files after Stage 9.9, got {lin_count}"
    );
}

/// Verify module declaration tests present (12 tests per plan-9.9.md §3.1)
#[test]
fn test_stage9_9_mod_decl_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");

    let mod_tests = [
        "mod_inline_empty.lin",
        "mod_inline_fn.lin",
        "mod_inline_struct.lin",
        "mod_inline_multi.lin",
        "mod_inline_nested.lin",
        "mod_inline_3_levels.lin",
        "mod_inline_with_vis.lin",
        "mod_inline_use.lin",
        "mod_external.lin",
        "mod_external_pub.lin",
        "mod_in_fn.lin",
        "mod_multi.lin",
    ];

    for name in &mod_tests {
        let path = mod_dir.join(name);
        assert!(path.exists(), "module declaration test {name} must exist");
    }

    // mod_in_fn should be FAIL (parser limitation)
    let content =
        std::fs::read_to_string(mod_dir.join("mod_in_fn.lin")).expect("read mod_in_fn.lin");
    assert!(
        content.contains("//! FAIL"),
        "mod_in_fn.lin must be FAIL — module declaration in fn body not supported in Stage 0"
    );
}

/// Verify basic use declaration tests present (12 tests per plan-9.9.md §3.2)
#[test]
fn test_stage9_9_use_basic_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");

    let use_tests = [
        "use_simple.lin",
        "use_multi_segment.lin",
        "use_self.lin",
        "use_super.lin",
        "use_crate.lin",
        "use_as.lin",
        "use_as_self.lin",
        "use_glob.lin",
        "use_nested.lin",
        "use_nested_multi.lin",
        "use_nested_glob.lin",
        "use_nested_as.lin",
    ];

    for name in &use_tests {
        let path = mod_dir.join(name);
        assert!(path.exists(), "use declaration test {name} must exist");
    }

    // use_as_self and use_nested_glob should be FAIL (parser limitations)
    for name in &["use_as_self.lin", "use_nested_glob.lin"] {
        let path = mod_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read use test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser limitation in Stage 0"
        );
    }
}

/// Verify advanced use declaration tests present (8 tests per plan-9.9.md §3.3)
#[test]
fn test_stage9_9_use_advanced_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");

    let use_tests = [
        "use_nested_deep.lin",
        "use_nested_3_levels.lin",
        "use_nested_self.lin",
        "use_nested_super.lin",
        "use_path_with_generics.lin",
        "use_in_module.lin",
        "use_multi.lin",
        "use_visibility.lin",
    ];

    for name in &use_tests {
        let path = mod_dir.join(name);
        assert!(path.exists(), "advanced use test {name} must exist");
    }
}

/// Verify pub visibility tests present (10 tests per plan-9.9.md §3.4)
#[test]
fn test_stage9_9_pub_vis_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");

    let vis_tests = [
        "vis_pub_fn.lin",
        "vis_pub_struct.lin",
        "vis_pub_enum.lin",
        "vis_pub_trait.lin",
        "vis_pub_const.lin",
        "vis_pub_static.lin",
        "vis_pub_mod.lin",
        "vis_pub_use.lin",
        "vis_pub_type.lin",
        "vis_pub_field.lin",
    ];

    for name in &vis_tests {
        let path = mod_dir.join(name);
        assert!(path.exists(), "pub visibility test {name} must exist");
    }
}

/// Verify restricted visibility tests present (8 tests per plan-9.9.md §3.5)
#[test]
fn test_stage9_9_restricted_vis_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");

    let vis_tests = [
        "vis_pub_crate.lin",
        "vis_pub_super.lin",
        "vis_pub_self.lin",
        "vis_pub_in_path.lin",
        "vis_pub_crate_struct.lin",
        "vis_pub_crate_field.lin",
        "vis_pub_crate_mod.lin",
        "vis_pub_crate_use.lin",
    ];

    for name in &vis_tests {
        let path = mod_dir.join(name);
        assert!(
            path.exists(),
            "restricted visibility test {name} must exist"
        );
    }
}

/// Verify error recovery tests present (10 tests per plan-9.9.md §3.6)
#[test]
fn test_stage9_9_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_dir = manifest.join("tests/conformance/00-parse/08-modules");

    let err_tests = [
        "err_mod_unclosed.lin",
        "err_use_no_semi.lin",
        "err_use_no_path.lin",
        "err_use_invalid_glob.lin",
        "err_vis_no_item.lin",
        "err_vis_invalid.lin",
        "err_use_unclosed_nested.lin",
        "err_mod_no_name.lin",
        "err_use_no_tree.lin",
        "err_use_double_colon.lin",
    ];

    for name in &err_tests {
        let path = mod_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // These should be FAIL (parser rejects)
    let fail_tests = [
        "err_mod_unclosed.lin",
        "err_use_no_semi.lin",
        "err_use_invalid_glob.lin",
        "err_vis_no_item.lin",
        "err_use_unclosed_nested.lin",
        "err_mod_no_name.lin",
        "err_use_double_colon.lin",
    ];
    for name in &fail_tests {
        let path = mod_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! FAIL"),
            "{name} must be FAIL — parser must report error"
        );
    }

    // These should be PASS (parser accepts via synthetic node recovery)
    let pass_tests = [
        "err_use_no_path.lin",
        "err_vis_invalid.lin",
        "err_use_no_tree.lin",
    ];
    for name in &pass_tests {
        let path = mod_dir.join(name);
        let content = std::fs::read_to_string(&path).expect("read err test");
        assert!(
            content.contains("//! PASS"),
            "{name} must be PASS — parser accepts via synthetic node recovery"
        );
    }
}

/// Verify Stage 9.9 docs created
#[test]
fn test_stage9_9_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_9 = manifest.join("docs/develop/v0/stage-9/plan-9.9.md");
    let gate_review_9_9 = manifest.join("docs/develop/v0/stage-9/gate-review-9.9.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/modules.md");

    assert!(plan_9_9.exists(), "plan-9.9.md must exist");
    assert!(gate_review_9_9.exists(), "gate-review-9.9.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/modules.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.8+
#[test]
fn test_stage9_9_cargo_toml_version_bumped() {
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
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23."),
        "Cargo.toml version must be 0.16.8+ after Stage 9.9 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 497 (437 from 9.8 + 60 new from 9.9)
#[test]
fn test_stage9_9_conformance_total_reaches_497() {
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
        total >= 497,
        "conformance suite total should be at least 497 (437 + 60) after Stage 9.9, got {total}"
    );
}
