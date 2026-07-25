//! Stage 9.11 — Realistic programs conformance expansion verification tests
//!
//! Per stage-committee-process.md v3.21 §17.1 + §13.4 design alignment
//! with `docs/lang-design/17-conformance-suite.md` §2 (10-realistic category).

#![cfg(test)]

use std::path::Path;

/// Verify realistic programs conformance directory has 54+ .lin files
#[test]
fn test_stage9_11_realistic_directory_populated() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");
    assert!(
        real_dir.exists(),
        "tests/conformance/00-parse/10-realistic/ must exist"
    );

    let lin_count = std::fs::read_dir(&real_dir)
        .expect("read 10-realistic/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lin").unwrap_or(false))
        .count();

    assert!(
        lin_count >= 54,
        "10-realistic/ should have at least 54 .lin files (2 existing + 52 new) after Stage 9.11, got {lin_count}"
    );
}

/// Verify classic algorithm tests present (12 tests per plan-9.11.md §3.1)
#[test]
fn test_stage9_11_algorithm_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");

    let algo_tests = [
        "realistic_fib_iterative.lin",
        "realistic_factorial.lin",
        "realistic_gcd.lin",
        "realistic_bubble_sort.lin",
        "realistic_binary_search.lin",
        "realistic_linear_search.lin",
        "realistic_power.lin",
        "realistic_is_prime.lin",
        "realistic_sum_array.lin",
        "realistic_max_array.lin",
        "realistic_reverse_array.lin",
        "realistic_countdown.lin",
    ];

    for name in &algo_tests {
        let path = real_dir.join(name);
        assert!(path.exists(), "algorithm test {name} must exist");
    }
}

/// Verify data structure tests present (10 tests per plan-9.11.md §3.2)
#[test]
fn test_stage9_11_data_structure_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");

    let ds_tests = [
        "realistic_linked_list.lin",
        "realistic_stack.lin",
        "realistic_queue.lin",
        "realistic_tree_node.lin",
        "realistic_tree_insert.lin",
        "realistic_hash_map_entry.lin",
        "realistic_vec_wrapper.lin",
        "realistic_option.lin",
        "realistic_result.lin",
        "realistic_point.lin",
    ];

    for name in &ds_tests {
        let path = real_dir.join(name);
        assert!(path.exists(), "data structure test {name} must exist");
    }
}

/// Verify trait pattern tests present (10 tests per plan-9.11.md §3.3)
#[test]
fn test_stage9_11_trait_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");

    let trait_tests = [
        "realistic_trait_display.lin",
        "realistic_trait_default.lin",
        "realistic_trait_iterator.lin",
        "realistic_trait_clone.lin",
        "realistic_trait_eq.lin",
        "realistic_trait_ord.lin",
        "realistic_trait_supertrait.lin",
        "realistic_trait_multi_impl.lin",
        "realistic_trait_associated_type.lin",
        "realistic_trait_static_method.lin",
    ];

    for name in &trait_tests {
        let path = real_dir.join(name);
        assert!(path.exists(), "trait pattern test {name} must exist");
    }
}

/// Verify closure tests present (8 tests per plan-9.11.md §3.4)
#[test]
fn test_stage9_11_closure_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");

    let closure_tests = [
        "realistic_closure_map.lin",
        "realistic_closure_filter.lin",
        "realistic_closure_reduce.lin",
        "realistic_closure_compose.lin",
        "realistic_closure_capture.lin",
        "realistic_closure_move_capture.lin",
        "realistic_closure_recursive.lin",
        "realistic_closure_callback.lin",
    ];

    for name in &closure_tests {
        let path = real_dir.join(name);
        assert!(path.exists(), "closure test {name} must exist");
    }
}

/// Verify pattern matching tests present (6 tests per plan-9.11.md §3.5)
#[test]
fn test_stage9_11_pattern_matching_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");

    let pm_tests = [
        "realistic_match_option.lin",
        "realistic_match_result.lin",
        "realistic_match_enum.lin",
        "realistic_match_nested.lin",
        "realistic_match_guard.lin",
        "realistic_match_or_pat.lin",
    ];

    for name in &pm_tests {
        let path = real_dir.join(name);
        assert!(path.exists(), "pattern matching test {name} must exist");
    }
}

/// Verify real-world snippet tests present (6 tests per plan-9.11.md §3.6)
#[test]
fn test_stage9_11_real_world_tests_present() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_dir = manifest.join("tests/conformance/00-parse/10-realistic");

    let rw_tests = [
        "realistic_calculator.lin",
        "realistic_string_ops.lin",
        "realistic_counter.lin",
        "realistic_config.lin",
        "realistic_state_machine.lin",
        "realistic_error_handling.lin",
    ];

    for name in &rw_tests {
        let path = real_dir.join(name);
        assert!(path.exists(), "real-world snippet test {name} must exist");
    }
}

/// Verify Stage 9.11 docs created
#[test]
fn test_stage9_11_docs_created() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan_9_11 = manifest.join("docs/develop/v0/stage-9/plan-9.11.md");
    let gate_review_9_11 = manifest.join("docs/develop/v0/stage-9/gate-review-9.11.md");
    let test_plan = manifest.join("docs/tests/v0/stage9/plan/realistic_programs.md");

    assert!(plan_9_11.exists(), "plan-9.11.md must exist");
    assert!(gate_review_9_11.exists(), "gate-review-9.11.md must exist");
    assert!(
        test_plan.exists(),
        "docs/tests/v0/stage9/plan/realistic_programs.md must exist"
    );
}

/// Verify Cargo.toml version bumped to 0.16.10+
#[test]
fn test_stage9_11_cargo_toml_version_bumped() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = manifest.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).expect("read Cargo.toml");

    let version_line = content
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("version line must exist");
    assert!(
        version_line.starts_with("version = \"0.16.10")
            || version_line.starts_with("version = \"0.16.11")
            || version_line.starts_with("version = \"0.16.12")
            || version_line.starts_with("version = \"0.17.")
            || version_line.starts_with("version = \"0.18.")
            || version_line.starts_with("version = \"0.19.")
            || version_line.starts_with("version = \"0.20.")
            || version_line.starts_with("version = \"0.21.")
            || version_line.starts_with("version = \"0.22.")
            || version_line.starts_with("version = \"0.23."),
        "Cargo.toml version must be 0.16.10+ after Stage 9.11 bump, got: {version_line}"
    );
}

/// Verify conformance suite total ≥ 599 (547 from 9.10 + 52 new from 9.11)
#[test]
fn test_stage9_11_conformance_total_reaches_599() {
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
        total >= 599,
        "conformance suite total should be at least 599 (547 + 52) after Stage 9.11, got {total}"
    );
}
