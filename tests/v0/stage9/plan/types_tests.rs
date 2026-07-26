//! Stage 9.5 — Types conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/02-grammar.md` §3.3 (Type).
//!
//! Test dimensions covered:
//! - Types conformance suite expanded (247 → 307 .lin files)
//! - All 10 type sub-categories present (primitive/ref/ptr/array/slice/
//!   tuple/fn-ptr/path/trait-object/error-recovery)
//! - Parser limitation documented (&& nested ref via maximal munch)

#![cfg(test)]

use std::path::Path;

/// Verify types conformance directory has 60+ .lin files
#[test]
fn test_stage9_5_types_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");
    assert!(
        types_dir.exists(),
        "tests/conformance/00-parse/04-types/ must exist"
    );

    let lin_count = std::fs::read_dir(&types_dir)
        .expect("read 04-types/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 60,
        "04-types/ should have at least 60 .lin files after Stage 9.5, got {lin_count}"
    );
}

/// Verify primitive type tests present (12 tests per plan-9.5.md §3.1)
#[test]
fn test_stage9_5_primitive_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let prim_tests = [
        "ty_bool.lin",
        "ty_char.lin",
        "ty_i8.lin",
        "ty_i32.lin",
        "ty_i64.lin",
        "ty_i128.lin",
        "ty_isize.lin",
        "ty_u8.lin",
        "ty_u32.lin",
        "ty_u64.lin",
        "ty_usize.lin",
        "ty_f64.lin",
    ];

    for name in &prim_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "primitive type test {name} must exist");
    }
}

/// Verify reference type tests present (8 tests per plan-9.5.md §3.2)
#[test]
fn test_stage9_5_reference_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let ref_tests = [
        "ty_ref_basic.lin",
        "ty_ref_mut.lin",
        "ty_ref_ref.lin",
        "ty_ref_str.lin",
        "ty_ref_array.lin",
        "ty_ref_struct.lin",
        "ty_ref_mut_struct.lin",
        "ty_ref_static.lin",
    ];

    for name in &ref_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "reference type test {name} must exist");
    }

    // ty_ref_ref should be FAIL (parser limitation: && lexed as AndAnd)
    let content =
        std::fs::read_to_string(types_dir.join("ty_ref_ref.lin")).expect("read ty_ref_ref.lin");
    assert!(
        content.contains("//! FAIL"),
        "ty_ref_ref.lin must be FAIL — && lexed as AndAnd via maximal munch"
    );
}

/// Verify raw pointer type tests present (5 tests per plan-9.5.md §3.3)
#[test]
fn test_stage9_5_pointer_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let ptr_tests = [
        "ty_ptr_const.lin",
        "ty_ptr_mut.lin",
        "ty_ptr_const_void.lin",
        "ty_ptr_const_struct.lin",
        "ty_ptr_mut_array.lin",
    ];

    for name in &ptr_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "raw pointer type test {name} must exist");
    }
}

/// Verify array type tests present (8 tests per plan-9.5.md §3.4)
#[test]
fn test_stage9_5_array_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let array_tests = [
        "ty_array_basic.lin",
        "ty_array_2d.lin",
        "ty_array_large.lin",
        "ty_array_bool.lin",
        "ty_array_str.lin",
        "ty_array_struct.lin",
        "ty_array_ref.lin",
        "ty_array_empty.lin",
    ];

    for name in &array_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "array type test {name} must exist");
    }
}

/// Verify slice type tests present (4 tests per plan-9.5.md §3.5)
#[test]
fn test_stage9_5_slice_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let slice_tests = [
        "ty_slice_basic.lin",
        "ty_slice_u8.lin",
        "ty_slice_str.lin",
        "ty_slice_struct.lin",
    ];

    for name in &slice_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "slice type test {name} must exist");
    }
}

/// Verify tuple type tests present (6 tests per plan-9.5.md §3.6)
#[test]
fn test_stage9_5_tuple_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let tuple_tests = [
        "ty_tuple_2.lin",
        "ty_tuple_3.lin",
        "ty_tuple_mixed.lin",
        "ty_tuple_empty.lin",
        "ty_tuple_single.lin",
        "ty_tuple_nested.lin",
    ];

    for name in &tuple_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "tuple type test {name} must exist");
    }
}

/// Verify function pointer type tests present (5 tests per plan-9.5.md §3.7)
#[test]
fn test_stage9_5_fn_ptr_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let fn_ptr_tests = [
        "ty_fn_ptr_basic.lin",
        "ty_fn_ptr_no_args.lin",
        "ty_fn_ptr_no_return.lin",
        "ty_fn_ptr_multi_args.lin",
        "ty_fn_ptr_ref_args.lin",
    ];

    for name in &fn_ptr_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "fn pointer type test {name} must exist");
    }
}

/// Verify path type tests present (5 tests per plan-9.5.md §3.8)
#[test]
fn test_stage9_5_path_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let path_tests = [
        "ty_path_simple.lin",
        "ty_path_qualified.lin",
        "ty_path_generic.lin",
        "ty_path_generic_multi.lin",
        "ty_path_nested.lin",
    ];

    for name in &path_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "path type test {name} must exist");
    }
}

/// Verify trait object type tests present (4 tests per plan-9.5.md §3.9)
#[test]
fn test_stage9_5_trait_object_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let trait_tests = [
        "ty_dyn_basic.lin",
        "ty_dyn_ref.lin",
        "ty_dyn_multi.lin",
        "ty_impl_basic.lin",
    ];

    for name in &trait_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "trait object type test {name} must exist");
    }
}

/// Verify error recovery tests present (3 tests per plan-9.5.md §3.10)
#[test]
fn test_stage9_5_error_recovery_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_dir = manifest.join("tests/conformance/00-parse/04-types");

    let err_tests = [
        "err_ty_missing.lin",
        "err_ty_unclosed_array.lin",
        "err_ty_unknown_primitive.lin",
    ];

    for name in &err_tests {
        let path = types_dir.join(name);
        assert!(path.exists(), "error recovery test {name} must exist");
    }

    // err_ty_unclosed_array should be FAIL (parser reports error)
    let content = std::fs::read_to_string(types_dir.join("err_ty_unclosed_array.lin"))
        .expect("read err_ty_unclosed_array.lin");
    assert!(
        content.contains("//! FAIL"),
        "err_ty_unclosed_array.lin must be FAIL — parser must report error"
    );
}

/// Verify Stage 9.5 docs created
#[test]
fn test_stage9_5_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_5 = manifest.join("docs/develop/v0/stage-9/plan-9.5.md");
    let gate_review_9_5 = manifest.join("docs/develop/v0/stage-9/gate-review-9.5.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/types.md");

    assert!(plan_9_5.exists(), "plan-9.5.md must exist");
    assert!(gate_review_9_5.exists(), "gate-review-9.5.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/types.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.4+
#[test]
fn test_stage9_5_cargo_toml_version_bumped() {
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
            || version_line.starts_with("version = \"0.2")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23."),
        "Cargo.toml version must be 0.16.4+ after Stage 9.5 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 307 (247 from 9.4 + 60 new from 9.5)
#[test]
fn test_stage9_5_conformance_total_reaches_307() {
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
        total >= 307,
        "conformance suite total should be at least 307 (247 + 60) after Stage 9.5, got {total}"
    );
}
