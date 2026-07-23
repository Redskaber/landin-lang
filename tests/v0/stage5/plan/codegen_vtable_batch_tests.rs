//! Stage 5.45: Codegen vtable emission batch helper tests
//!
//! Tests `StdlibVtableGlobalSpec` struct + `emit_vtable_globals_batch()`.
//!
//! **Critical invariant**: batch output must be byte-for-byte identical
//! to calling `emit_vtable_global_text()` per-spec and collecting results.
//! `test_emit_vtable_globals_batch_matches_individual` verifies this.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::codegen::{
    emit_vtable_global_text, emit_vtable_globals_batch, StdlibVtableGlobalSpec,
};

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

/// Empty input → empty Vec.
#[test]
fn test_emit_vtable_globals_batch_empty() {
    let result = emit_vtable_globals_batch(&[]);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// Basic batch
// ---------------------------------------------------------------------------

/// Single spec → 1-element Vec.
#[test]
fn test_emit_vtable_globals_batch_single() {
    let specs = vec![StdlibVtableGlobalSpec {
        global_name: ".vtable.Drop.S".to_string(),
        method_symbols: vec!["landin_S_drop".to_string()],
    }];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        "@.vtable.Drop.S = private unnamed_addr constant [1 x ptr] [ptr @landin_S_drop]"
    );
}

/// Multi-spec → multi-element Vec, order preserved.
#[test]
fn test_emit_vtable_globals_batch_multi() {
    let specs = vec![
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Clone.S".to_string(),
            method_symbols: vec![
                "landin_S_clone".to_string(),
                "landin_S_clone_from".to_string(),
            ],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Drop.S".to_string(),
            method_symbols: vec!["landin_S_drop".to_string()],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Add.Vec".to_string(),
            method_symbols: vec!["landin_Vec_add".to_string()],
        },
    ];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 3);
    // Order preserved
    assert!(result[0].contains(".vtable.Clone.S"));
    assert!(result[1].contains(".vtable.Drop.S"));
    assert!(result[2].contains(".vtable.Add.Vec"));
}

// ---------------------------------------------------------------------------
// **Critical**: batch == individual calls
// ---------------------------------------------------------------------------

/// Batch output must match calling `emit_vtable_global_text()` per-spec
/// and collecting results.
#[test]
fn test_emit_vtable_globals_batch_matches_individual() {
    let specs = vec![
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Clone.S".to_string(),
            method_symbols: vec![
                "landin_S_clone".to_string(),
                "landin_S_clone_from".to_string(),
            ],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Copy.S".to_string(),
            method_symbols: vec![],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.PartialEq.S".to_string(),
            method_symbols: vec!["landin_S_eq".to_string(), "null".to_string()],
        },
    ];

    let batch_result = emit_vtable_globals_batch(&specs);
    let individual_result: Vec<String> = specs
        .iter()
        .map(|spec| emit_vtable_global_text(&spec.global_name, &spec.method_symbols))
        .collect();

    assert_eq!(batch_result, individual_result);
}

// ---------------------------------------------------------------------------
// Order preservation
// ---------------------------------------------------------------------------

/// Output order matches input order — no sorting.
#[test]
fn test_emit_vtable_globals_batch_order_preserved() {
    // Deliberately non-alphabetical order
    let specs = vec![
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Z.Last".to_string(),
            method_symbols: vec![],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.A.First".to_string(),
            method_symbols: vec![],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.M.Middle".to_string(),
            method_symbols: vec![],
        },
    ];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 3);
    // No sorting — input order preserved
    assert!(result[0].contains(".vtable.Z.Last"));
    assert!(result[1].contains(".vtable.A.First"));
    assert!(result[2].contains(".vtable.M.Middle"));
}

// ---------------------------------------------------------------------------
// Edge cases (marker / null / mixed)
// ---------------------------------------------------------------------------

/// Batch with marker spec (empty method_symbols → zeroinitializer).
#[test]
fn test_emit_vtable_globals_batch_with_marker() {
    let specs = vec![StdlibVtableGlobalSpec {
        global_name: ".vtable.Copy.S".to_string(),
        method_symbols: vec![],
    }];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains("zeroinitializer"));
    assert!(!result[0].contains("x ptr]")); // no array type for markers
}

/// Batch with null symbol → `ptr null`.
#[test]
fn test_emit_vtable_globals_batch_with_null() {
    let specs = vec![StdlibVtableGlobalSpec {
        global_name: ".vtable.Clone.S".to_string(),
        method_symbols: vec!["landin_S_clone".to_string(), "null".to_string()],
    }];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains("ptr null"));
    assert!(!result[0].contains("ptr @null"));
}

/// Mixed batch: marker + null + real.
#[test]
fn test_emit_vtable_globals_batch_mixed() {
    let specs = vec![
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Copy.S".to_string(),
            method_symbols: vec![],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Clone.S".to_string(),
            method_symbols: vec!["landin_S_clone".to_string(), "null".to_string()],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Drop.S".to_string(),
            method_symbols: vec!["landin_S_drop".to_string()],
        },
    ];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 3);
    assert!(result[0].contains("zeroinitializer"));
    assert!(result[1].contains("ptr null"));
    assert!(result[2].contains("ptr @landin_S_drop"));
}

// ---------------------------------------------------------------------------
// Struct semantics
// ---------------------------------------------------------------------------

/// `StdlibVtableGlobalSpec` field access.
#[test]
fn test_stdlib_vtable_global_spec_struct() {
    let spec = StdlibVtableGlobalSpec {
        global_name: ".vtable.Foo.S".to_string(),
        method_symbols: vec!["landin_S_bar".to_string()],
    };
    assert_eq!(spec.global_name, ".vtable.Foo.S");
    assert_eq!(spec.method_symbols, vec!["landin_S_bar".to_string()]);
}

/// `StdlibVtableGlobalSpec` derives PartialEq/Eq.
#[test]
fn test_stdlib_vtable_global_spec_eq() {
    let s1 = StdlibVtableGlobalSpec {
        global_name: ".vtable.Foo.S".to_string(),
        method_symbols: vec!["landin_S_bar".to_string()],
    };
    let s2 = StdlibVtableGlobalSpec {
        global_name: ".vtable.Foo.S".to_string(),
        method_symbols: vec!["landin_S_bar".to_string()],
    };
    assert_eq!(s1, s2);

    let s3 = StdlibVtableGlobalSpec {
        global_name: ".vtable.Foo.T".to_string(), // different
        method_symbols: vec!["landin_S_bar".to_string()],
    };
    assert_ne!(s1, s3);
}

// ---------------------------------------------------------------------------
// Real-world scenario simulation
// ---------------------------------------------------------------------------

/// Simulate the real `emit_vtables()` scenario: multiple (trait, type)
/// pairs → multiple specs → batch output.
#[test]
fn test_emit_vtable_globals_batch_real_vtables() {
    // Simulate: struct S impls Clone + Drop + Add
    let specs = vec![
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Clone.S".to_string(),
            method_symbols: vec![
                "landin_S_clone".to_string(),
                "landin_S_clone_from".to_string(),
            ],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Drop.S".to_string(),
            method_symbols: vec!["landin_S_drop".to_string()],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Add.S".to_string(),
            method_symbols: vec!["landin_S_add".to_string()],
        },
    ];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 3);
    // Each line should be valid LLVM IR
    for line in &result {
        assert!(line.starts_with("@.vtable."));
        assert!(line.contains("private unnamed_addr constant"));
    }
}

/// Batch does NOT dedup — caller is responsible. Two identical specs →
/// two identical IR lines.
#[test]
fn test_emit_vtable_globals_batch_dedup_not_required() {
    let specs = vec![
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Clone.S".to_string(),
            method_symbols: vec!["landin_S_clone".to_string()],
        },
        StdlibVtableGlobalSpec {
            global_name: ".vtable.Clone.S".to_string(), // duplicate
            method_symbols: vec!["landin_S_clone".to_string()],
        },
    ];
    let result = emit_vtable_globals_batch(&specs);
    assert_eq!(result.len(), 2); // not deduped
    assert_eq!(result[0], result[1]);
}
