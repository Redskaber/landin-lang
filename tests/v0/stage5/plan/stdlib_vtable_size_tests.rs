//! Stage 5.38: Stdlib vtable byte size + pointer-width-aware layout tests
//!
//! Tests `StdlibPointerWidth` + `byte_size()` + `stdlib_pointer_width_bytes()`
//! + `stdlib_vtable_byte_size()` + `stdlib_vtable_method_offset()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.

use landin_compiler::stdlib::{
    stdlib_pointer_width_bytes, stdlib_vtable_byte_size, stdlib_vtable_method_offset,
    StdlibPointerWidth,
};

// ---------------------------------------------------------------------------
// StdlibPointerWidth + byte_size
// ---------------------------------------------------------------------------

/// Pointer32 → 4 bytes/slot.
#[test]
fn test_stdlib_pointer_width_byte_size_32() {
    assert_eq!(StdlibPointerWidth::Pointer32.byte_size(), 4);
}

/// Pointer64 → 8 bytes/slot.
#[test]
fn test_stdlib_pointer_width_byte_size_64() {
    assert_eq!(StdlibPointerWidth::Pointer64.byte_size(), 8);
}

/// `stdlib_pointer_width_bytes` free fn matches method form.
#[test]
fn test_stdlib_pointer_width_bytes_free_fn() {
    assert_eq!(stdlib_pointer_width_bytes(StdlibPointerWidth::Pointer32), 4);
    assert_eq!(stdlib_pointer_width_bytes(StdlibPointerWidth::Pointer64), 8);
    assert_eq!(
        stdlib_pointer_width_bytes(StdlibPointerWidth::Pointer32),
        StdlibPointerWidth::Pointer32.byte_size()
    );
}

/// `StdlibPointerWidth` derives PartialEq/Eq.
#[test]
fn test_stdlib_pointer_width_eq() {
    assert_eq!(StdlibPointerWidth::Pointer32, StdlibPointerWidth::Pointer32);
    assert_ne!(StdlibPointerWidth::Pointer32, StdlibPointerWidth::Pointer64);
}

// ---------------------------------------------------------------------------
// stdlib_vtable_byte_size
// ---------------------------------------------------------------------------

/// Clone@32bit → 8 bytes (2 slots × 4 bytes).
#[test]
fn test_stdlib_vtable_byte_size_clone_32() {
    assert_eq!(
        stdlib_vtable_byte_size("Clone", StdlibPointerWidth::Pointer32),
        Some(8)
    );
}

/// Clone@64bit → 16 bytes (2 slots × 8 bytes).
#[test]
fn test_stdlib_vtable_byte_size_clone_64() {
    assert_eq!(
        stdlib_vtable_byte_size("Clone", StdlibPointerWidth::Pointer64),
        Some(16)
    );
}

/// Drop → 4 bytes (32-bit) / 8 bytes (64-bit).
#[test]
fn test_stdlib_vtable_byte_size_drop() {
    assert_eq!(
        stdlib_vtable_byte_size("Drop", StdlibPointerWidth::Pointer32),
        Some(4)
    );
    assert_eq!(
        stdlib_vtable_byte_size("Drop", StdlibPointerWidth::Pointer64),
        Some(8)
    );
}

/// PartialEq → 8 bytes (32-bit, 2 slots) / 16 bytes (64-bit).
#[test]
fn test_stdlib_vtable_byte_size_partial_eq() {
    assert_eq!(
        stdlib_vtable_byte_size("PartialEq", StdlibPointerWidth::Pointer32),
        Some(8)
    );
    assert_eq!(
        stdlib_vtable_byte_size("PartialEq", StdlibPointerWidth::Pointer64),
        Some(16)
    );
}

/// Single-method arithmetic trait (Add) → 4/8 bytes.
#[test]
fn test_stdlib_vtable_byte_size_arith() {
    assert_eq!(
        stdlib_vtable_byte_size("Add", StdlibPointerWidth::Pointer32),
        Some(4)
    );
    assert_eq!(
        stdlib_vtable_byte_size("Add", StdlibPointerWidth::Pointer64),
        Some(8)
    );
}

/// Markers (Copy/Send/Sync/Sized/Unpin/Eq) → Some(0) at both widths.
#[test]
fn test_stdlib_vtable_byte_size_marker() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        assert_eq!(
            stdlib_vtable_byte_size(trait_name, StdlibPointerWidth::Pointer32),
            Some(0),
            "{trait_name} should have 0-byte vtable at 32-bit"
        );
        assert_eq!(
            stdlib_vtable_byte_size(trait_name, StdlibPointerWidth::Pointer64),
            Some(0),
            "{trait_name} should have 0-byte vtable at 64-bit"
        );
    }
}

/// Unknown traits → None at both widths.
#[test]
fn test_stdlib_vtable_byte_size_unknown() {
    assert_eq!(
        stdlib_vtable_byte_size("BogusTrait", StdlibPointerWidth::Pointer32),
        None
    );
    assert_eq!(
        stdlib_vtable_byte_size("BogusTrait", StdlibPointerWidth::Pointer64),
        None
    );
    assert_eq!(
        stdlib_vtable_byte_size("From", StdlibPointerWidth::Pointer64),
        None
    );
    assert_eq!(
        stdlib_vtable_byte_size("", StdlibPointerWidth::Pointer64),
        None
    );
}

// ---------------------------------------------------------------------------
// stdlib_vtable_method_offset
// ---------------------------------------------------------------------------

/// Clone::clone@0, clone_from@offset=width (4 or 8).
#[test]
fn test_stdlib_vtable_method_offset_clone() {
    assert_eq!(
        stdlib_vtable_method_offset("Clone", "clone", StdlibPointerWidth::Pointer32),
        Some(0)
    );
    assert_eq!(
        stdlib_vtable_method_offset("Clone", "clone_from", StdlibPointerWidth::Pointer32),
        Some(4)
    );
    assert_eq!(
        stdlib_vtable_method_offset("Clone", "clone", StdlibPointerWidth::Pointer64),
        Some(0)
    );
    assert_eq!(
        stdlib_vtable_method_offset("Clone", "clone_from", StdlibPointerWidth::Pointer64),
        Some(8)
    );
}

/// Drop::drop@0 at both widths.
#[test]
fn test_stdlib_vtable_method_offset_drop() {
    assert_eq!(
        stdlib_vtable_method_offset("Drop", "drop", StdlibPointerWidth::Pointer32),
        Some(0)
    );
    assert_eq!(
        stdlib_vtable_method_offset("Drop", "drop", StdlibPointerWidth::Pointer64),
        Some(0)
    );
}

/// PartialEq@64bit: eq@0, ne@8.
#[test]
fn test_stdlib_vtable_method_offset_partial_eq_64() {
    assert_eq!(
        stdlib_vtable_method_offset("PartialEq", "eq", StdlibPointerWidth::Pointer64),
        Some(0)
    );
    assert_eq!(
        stdlib_vtable_method_offset("PartialEq", "ne", StdlibPointerWidth::Pointer64),
        Some(8)
    );
}

/// PartialEq@32bit: eq@0, ne@4.
#[test]
fn test_stdlib_vtable_method_offset_partial_eq_32() {
    assert_eq!(
        stdlib_vtable_method_offset("PartialEq", "eq", StdlibPointerWidth::Pointer32),
        Some(0)
    );
    assert_eq!(
        stdlib_vtable_method_offset("PartialEq", "ne", StdlibPointerWidth::Pointer32),
        Some(4)
    );
}

/// Arith op offset: Add::add@0 at both widths.
#[test]
fn test_stdlib_vtable_method_offset_arith() {
    assert_eq!(
        stdlib_vtable_method_offset("Add", "add", StdlibPointerWidth::Pointer64),
        Some(0)
    );
    assert_eq!(
        stdlib_vtable_method_offset("Sub", "sub", StdlibPointerWidth::Pointer64),
        Some(0)
    );
}

/// Marker traits → method_offset returns None (no slots at all).
#[test]
fn test_stdlib_vtable_method_offset_marker() {
    for trait_name in &["Copy", "Send", "Sync", "Sized", "Unpin", "Eq"] {
        assert_eq!(
            stdlib_vtable_method_offset(trait_name, "clone", StdlibPointerWidth::Pointer64),
            None,
            "{trait_name} is a marker — should have no method offset"
        );
    }
}

/// Known trait + unknown method → None.
#[test]
fn test_stdlib_vtable_method_offset_unknown_method() {
    assert_eq!(
        stdlib_vtable_method_offset("Clone", "bogus", StdlibPointerWidth::Pointer64),
        None
    );
    assert_eq!(
        stdlib_vtable_method_offset("Clone", "next", StdlibPointerWidth::Pointer64),
        None
    );
    // Add doesn't have `sub` (different op)
    assert_eq!(
        stdlib_vtable_method_offset("Add", "sub", StdlibPointerWidth::Pointer64),
        None
    );
}

/// Unknown trait → None.
#[test]
fn test_stdlib_vtable_method_offset_unknown_trait() {
    assert_eq!(
        stdlib_vtable_method_offset("Bogus", "x", StdlibPointerWidth::Pointer64),
        None
    );
    assert_eq!(
        stdlib_vtable_method_offset("From", "from", StdlibPointerWidth::Pointer64),
        None
    );
    assert_eq!(
        stdlib_vtable_method_offset("", "x", StdlibPointerWidth::Pointer64),
        None
    );
}

// ---------------------------------------------------------------------------
// Cross-checks: byte_size and method_offset are consistent
// ---------------------------------------------------------------------------

/// For any (trait, method) pair, method_offset < vtable_byte_size when both
/// are Some.
#[test]
fn test_stdlib_vtable_offset_within_bounds() {
    let cases = [
        ("Clone", "clone"),
        ("Clone", "clone_from"),
        ("Drop", "drop"),
        ("PartialEq", "eq"),
        ("PartialEq", "ne"),
        ("Add", "add"),
        ("Iterator", "next"),
    ];
    for (trait_name, method_name) in cases {
        for width in [StdlibPointerWidth::Pointer32, StdlibPointerWidth::Pointer64] {
            let offset = stdlib_vtable_method_offset(trait_name, method_name, width);
            let total = stdlib_vtable_byte_size(trait_name, width);
            assert!(
                offset.is_some(),
                "{trait_name}::{method_name} should have offset"
            );
            assert!(total.is_some(), "{trait_name} should have byte_size");
            let offset = offset.unwrap();
            let total = total.unwrap();
            assert!(
                offset < total,
                "{trait_name}::{method_name} offset {offset} >= total {total} (width={width:?})"
            );
        }
    }
}
