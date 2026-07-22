//! Stage 5.11: Primitive Copy auto-detection tests
//!
//! Tests that `is_primitive_copy_kind()` correctly identifies MIR `TyKind`
//! variant names that are always Copy (Bool, Char, Int, Uint, Float, Never,
//! Ref, RawPtr, FnDef, FnPtr) — without consulting the trait resolver.
//!
//! Also verifies that non-Copy kinds (Str, Slice, Closure, Param, Adt,
//! Tuple, Array, Infer, Error, Foreign) are correctly rejected.
//!
//! Per §16: tests use the public `is_primitive_copy_kind()` API directly.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::is_primitive_copy_kind;
use landin_compiler::BUILTIN_PRIMITIVE_COPY_KINDS;

/// All kinds in BUILTIN_PRIMITIVE_COPY_KINDS should be Copy.
#[test]
fn test_all_primitive_copy_kinds_are_copy() {
    for &kind in BUILTIN_PRIMITIVE_COPY_KINDS {
        assert!(
            is_primitive_copy_kind(kind),
            "{} should be a primitive Copy kind",
            kind
        );
    }
}

/// Integer variants (with tuple fields) should be detected as Copy.
#[test]
fn test_int_variants_are_copy() {
    // The function strips "(...)" suffix, so "Int(I32)" → "Int" → Copy.
    assert!(
        is_primitive_copy_kind("Int(I32)"),
        "Int(I32) should be Copy"
    );
    assert!(
        is_primitive_copy_kind("Int(I64)"),
        "Int(I64) should be Copy"
    );
    assert!(
        is_primitive_copy_kind("Uint(U32)"),
        "Uint(U32) should be Copy"
    );
    assert!(
        is_primitive_copy_kind("Float(F64)"),
        "Float(F64) should be Copy"
    );
}

/// Non-Copy kinds should be rejected.
#[test]
fn test_non_copy_kinds_rejected() {
    let non_copy = [
        "Str", "Slice", "Closure", "Param", "Adt", "Tuple", "Array", "Infer", "Error", "Foreign",
    ];
    for kind in &non_copy {
        assert!(
            !is_primitive_copy_kind(kind),
            "{} should NOT be a primitive Copy kind",
            kind
        );
    }
}

/// Adt with tuple fields should be rejected (Adt is not primitive Copy).
#[test]
fn test_adt_with_fields_rejected() {
    assert!(!is_primitive_copy_kind("Adt"), "Adt should NOT be Copy");
    assert!(
        !is_primitive_copy_kind("Adt(DefId(0))"),
        "Adt(DefId(0)) should NOT be Copy"
    );
}

/// Empty string and unknown kinds should be rejected.
#[test]
fn test_unknown_kinds_rejected() {
    assert!(
        !is_primitive_copy_kind(""),
        "empty string should NOT be Copy"
    );
    assert!(
        !is_primitive_copy_kind("Unknown"),
        "Unknown should NOT be Copy"
    );
    assert!(
        !is_primitive_copy_kind("Vector"),
        "Vector should NOT be Copy"
    );
}

/// The constant should have exactly 10 primitive Copy kinds.
#[test]
fn test_primitive_copy_kinds_count() {
    assert_eq!(
        BUILTIN_PRIMITIVE_COPY_KINDS.len(),
        10,
        "should have 10 primitive Copy kinds"
    );
}
