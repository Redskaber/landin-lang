//! Stage 5.35: Stdlib type layout tests
//!
//! Tests `type_size_bytes()`, `type_alignment_bytes()`,
//! `is_zero_sized_type()`, `type_description()`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    is_zero_sized_type, type_alignment_bytes, type_description, type_size_bytes,
};

/// `type_size_bytes` should return correct sizes for integers.
#[test]
fn test_type_size_bytes_integers() {
    assert_eq!(type_size_bytes("i8"), Some(1));
    assert_eq!(type_size_bytes("u8"), Some(1));
    assert_eq!(type_size_bytes("i16"), Some(2));
    assert_eq!(type_size_bytes("u16"), Some(2));
    assert_eq!(type_size_bytes("i32"), Some(4));
    assert_eq!(type_size_bytes("u32"), Some(4));
    assert_eq!(type_size_bytes("i64"), Some(8));
    assert_eq!(type_size_bytes("u64"), Some(8));
    assert_eq!(type_size_bytes("i128"), Some(16));
    assert_eq!(type_size_bytes("u128"), Some(16));
}

/// `type_size_bytes` should return correct sizes for floats and bool.
#[test]
fn test_type_size_bytes_floats_bool() {
    assert_eq!(type_size_bytes("f32"), Some(4));
    assert_eq!(type_size_bytes("f64"), Some(8));
    assert_eq!(type_size_bytes("bool"), Some(1));
    assert_eq!(type_size_bytes("char"), Some(4));
}

/// `type_size_bytes` should return 0 for ZSTs.
#[test]
fn test_type_size_bytes_zst() {
    assert_eq!(type_size_bytes("()"), Some(0));
    assert_eq!(type_size_bytes("Never"), Some(0));
}

/// `type_size_bytes` should return None for unsized/unknown types.
#[test]
fn test_type_size_bytes_none() {
    assert_eq!(type_size_bytes("str"), None);
    assert_eq!(type_size_bytes("Box"), None);
    assert_eq!(type_size_bytes("File"), None);
    assert_eq!(type_size_bytes("MyType"), None);
}

/// `type_alignment_bytes` should match `type_size_bytes` for primitives.
#[test]
fn test_type_alignment_bytes() {
    assert_eq!(type_alignment_bytes("i32"), Some(4));
    assert_eq!(type_alignment_bytes("f64"), Some(8));
    assert_eq!(type_alignment_bytes("bool"), Some(1));
    assert_eq!(type_alignment_bytes("str"), None);
}

/// `is_zero_sized_type` should return true for () and Never.
#[test]
fn test_is_zero_sized_type() {
    assert!(is_zero_sized_type("()"));
    assert!(is_zero_sized_type("Never"));
    assert!(!is_zero_sized_type("i32"));
    assert!(!is_zero_sized_type("bool"));
    assert!(!is_zero_sized_type("Box"));
}

/// `type_description` should return human-readable descriptions.
#[test]
fn test_type_description() {
    assert_eq!(type_description("i32"), Some("32-bit signed integer"));
    assert_eq!(type_description("u64"), Some("64-bit unsigned integer"));
    assert_eq!(type_description("f64"), Some("64-bit floating point"));
    assert_eq!(type_description("bool"), Some("boolean"));
    assert_eq!(
        type_description("str"),
        Some("UTF-8 string slice (unsized)")
    );
    assert_eq!(type_description("()"), Some("unit type (zero-sized)"));
    assert_eq!(type_description("Box"), Some("alloc-layer heap type"));
    assert_eq!(
        type_description("File"),
        Some("std-layer OS-dependent type")
    );
    assert_eq!(type_description("MyType"), None);
}
