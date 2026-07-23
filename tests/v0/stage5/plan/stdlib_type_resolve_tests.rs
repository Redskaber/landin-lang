//! Stage 5.34: Stdlib type resolution tests
//!
//! Tests `resolve_stdlib_type()`, `StdlibTypeKind`, `is_primitive_type()`,
//! `integer_bit_width()`, `is_signed_integer()`, `is_unsigned_integer()`,
//! `is_float_type()`.
//!
//! Per §16: tests use the public API only.
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    integer_bit_width, is_float_type, is_primitive_type, is_signed_integer, is_unsigned_integer,
    resolve_stdlib_type, StdlibTypeKind,
};

/// `resolve_stdlib_type` should resolve core integer types.
#[test]
fn test_resolve_integers() {
    assert_eq!(resolve_stdlib_type("i8"), StdlibTypeKind::I8);
    assert_eq!(resolve_stdlib_type("i32"), StdlibTypeKind::I32);
    assert_eq!(resolve_stdlib_type("i64"), StdlibTypeKind::I64);
    assert_eq!(resolve_stdlib_type("u8"), StdlibTypeKind::U8);
    assert_eq!(resolve_stdlib_type("u128"), StdlibTypeKind::U128);
}

/// `resolve_stdlib_type` should resolve float types.
#[test]
fn test_resolve_floats() {
    assert_eq!(resolve_stdlib_type("f32"), StdlibTypeKind::F32);
    assert_eq!(resolve_stdlib_type("f64"), StdlibTypeKind::F64);
}

/// `resolve_stdlib_type` should resolve other primitives.
#[test]
fn test_resolve_other_primitives() {
    assert_eq!(resolve_stdlib_type("bool"), StdlibTypeKind::Bool);
    assert_eq!(resolve_stdlib_type("char"), StdlibTypeKind::Char);
    assert_eq!(resolve_stdlib_type("str"), StdlibTypeKind::Str);
    assert_eq!(resolve_stdlib_type("()"), StdlibTypeKind::Unit);
    assert_eq!(resolve_stdlib_type("Never"), StdlibTypeKind::Never);
}

/// `resolve_stdlib_type` should resolve alloc types as AllocType.
#[test]
fn test_resolve_alloc_types() {
    assert_eq!(resolve_stdlib_type("Box"), StdlibTypeKind::AllocType);
    assert_eq!(resolve_stdlib_type("Vec"), StdlibTypeKind::AllocType);
    assert_eq!(resolve_stdlib_type("String"), StdlibTypeKind::AllocType);
    assert_eq!(resolve_stdlib_type("HashMap"), StdlibTypeKind::AllocType);
}

/// `resolve_stdlib_type` should resolve std types as StdType.
#[test]
fn test_resolve_std_types() {
    assert_eq!(resolve_stdlib_type("File"), StdlibTypeKind::StdType);
    assert_eq!(resolve_stdlib_type("Path"), StdlibTypeKind::StdType);
    assert_eq!(resolve_stdlib_type("Result"), StdlibTypeKind::StdType);
    assert_eq!(resolve_stdlib_type("Option"), StdlibTypeKind::StdType);
}

/// `resolve_stdlib_type` should return Unknown for non-stdlib names.
#[test]
fn test_resolve_unknown() {
    assert_eq!(resolve_stdlib_type("MyType"), StdlibTypeKind::Unknown);
    assert_eq!(resolve_stdlib_type(""), StdlibTypeKind::Unknown);
}

/// `is_primitive_type` should return true for core types only.
#[test]
fn test_is_primitive_type() {
    assert!(is_primitive_type("i32"));
    assert!(is_primitive_type("bool"));
    assert!(is_primitive_type("f64"));
    assert!(!is_primitive_type("Box"));
    assert!(!is_primitive_type("File"));
    assert!(!is_primitive_type("MyType"));
}

/// `integer_bit_width` should return correct widths.
#[test]
fn test_integer_bit_width() {
    assert_eq!(integer_bit_width("i8"), Some(8));
    assert_eq!(integer_bit_width("u16"), Some(16));
    assert_eq!(integer_bit_width("i32"), Some(32));
    assert_eq!(integer_bit_width("u64"), Some(64));
    assert_eq!(integer_bit_width("i128"), Some(128));
    assert_eq!(integer_bit_width("bool"), None);
    assert_eq!(integer_bit_width("f32"), None);
    assert_eq!(integer_bit_width("Box"), None);
}

/// `is_signed_integer` should return true only for signed integers.
#[test]
fn test_is_signed_integer() {
    assert!(is_signed_integer("i8"));
    assert!(is_signed_integer("i32"));
    assert!(!is_signed_integer("u32"));
    assert!(!is_signed_integer("f64"));
    assert!(!is_signed_integer("bool"));
}

/// `is_unsigned_integer` should return true only for unsigned integers.
#[test]
fn test_is_unsigned_integer() {
    assert!(is_unsigned_integer("u8"));
    assert!(is_unsigned_integer("u32"));
    assert!(!is_unsigned_integer("i32"));
    assert!(!is_unsigned_integer("f64"));
}

/// `is_float_type` should return true only for float types.
#[test]
fn test_is_float_type() {
    assert!(is_float_type("f32"));
    assert!(is_float_type("f64"));
    assert!(!is_float_type("i32"));
    assert!(!is_float_type("bool"));
}
