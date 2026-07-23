//! Stage 5.40: Stdlib vtable symbol name planner tests
//!
//! Tests `stdlib_vtable_global_name()` + `stdlib_dynptr_global_name()` +
//! `stdlib_data_global_name()` + `stdlib_impl_method_symbol()` +
//! `stdlib_vtable_method_symbols()`.
//!
//! Per §16: tests use the public API only (no driver/hir/mir access).
//! Per §17.3: tests live under `tests/v0/stage5/plan/`.
//!
//! Critical invariant: the strings produced by these functions must match
//! byte-for-byte what codegen currently emits via inline `format!()` calls.
//! The `*_match_codegen_format` tests verify this.

use landin_compiler::stdlib::{
    stdlib_data_global_name, stdlib_dynptr_global_name, stdlib_impl_method_symbol,
    stdlib_vtable_global_name, stdlib_vtable_method_symbols,
};

// ---------------------------------------------------------------------------
// stdlib_vtable_global_name
// ---------------------------------------------------------------------------

/// `.vtable.Foo.S` — matches codegen `format!(".vtable.{}.{}", trait, type)`.
#[test]
fn test_stdlib_vtable_global_name() {
    assert_eq!(stdlib_vtable_global_name("Foo", "S"), ".vtable.Foo.S");
    assert_eq!(
        stdlib_vtable_global_name("Display", "Vec"),
        ".vtable.Display.Vec"
    );
}

/// Matches codegen's `format!` byte-for-byte.
#[test]
fn test_stdlib_vtable_global_name_match_codegen() {
    let trait_str = "Foo";
    let type_str = "S";
    let codegen_format = format!(".vtable.{trait_str}.{type_str}");
    assert_eq!(
        stdlib_vtable_global_name(trait_str, type_str),
        codegen_format
    );
}

// ---------------------------------------------------------------------------
// stdlib_dynptr_global_name
// ---------------------------------------------------------------------------

/// `.dynptr.Foo.S` — matches codegen `format!(".dynptr.{}.{}", trait, type)`.
#[test]
fn test_stdlib_dynptr_global_name() {
    assert_eq!(stdlib_dynptr_global_name("Foo", "S"), ".dynptr.Foo.S");
    assert_eq!(
        stdlib_dynptr_global_name("Display", "Vec"),
        ".dynptr.Display.Vec"
    );
}

// ---------------------------------------------------------------------------
// stdlib_data_global_name
// ---------------------------------------------------------------------------

/// `.data.S` — matches codegen `format!(".data.{}", type)`.
#[test]
fn test_stdlib_data_global_name() {
    assert_eq!(stdlib_data_global_name("S"), ".data.S");
    assert_eq!(stdlib_data_global_name("Vec"), ".data.Vec");
}

// ---------------------------------------------------------------------------
// stdlib_impl_method_symbol
// ---------------------------------------------------------------------------

/// `landin_S_bar` — matches `format!("landin_{}_{}", type, method)`.
#[test]
fn test_stdlib_impl_method_symbol() {
    assert_eq!(stdlib_impl_method_symbol("S", "bar"), "landin_S_bar");
    assert_eq!(stdlib_impl_method_symbol("Vec", "push"), "landin_Vec_push");
}

/// Multi-part type/method names work correctly.
#[test]
fn test_stdlib_impl_method_symbol_multi_part() {
    assert_eq!(
        stdlib_impl_method_symbol("MyType", "my_method"),
        "landin_MyType_my_method"
    );
}

// ---------------------------------------------------------------------------
// stdlib_vtable_method_symbols
// ---------------------------------------------------------------------------

/// Clone + S + [clone, clone_from] → 2 symbols, both provided.
#[test]
fn test_stdlib_vtable_method_symbols_clone_complete() {
    let symbols = stdlib_vtable_method_symbols("Clone", "S", &["clone", "clone_from"])
        .expect("Clone should be registered");
    assert_eq!(symbols, vec!["landin_S_clone", "landin_S_clone_from"]);
}

/// Clone + S + [clone] → clone_from is "null" (not provided).
#[test]
fn test_stdlib_vtable_method_symbols_clone_partial() {
    let symbols = stdlib_vtable_method_symbols("Clone", "S", &["clone"]).unwrap();
    assert_eq!(symbols, vec!["landin_S_clone", "null"]);
}

/// Drop + S + [drop] → 1 symbol.
#[test]
fn test_stdlib_vtable_method_symbols_drop() {
    let symbols = stdlib_vtable_method_symbols("Drop", "S", &["drop"]).unwrap();
    assert_eq!(symbols, vec!["landin_S_drop"]);
}

/// PartialEq + S + [eq] → ne is "null".
#[test]
fn test_stdlib_vtable_method_symbols_partial_eq() {
    let symbols = stdlib_vtable_method_symbols("PartialEq", "S", &["eq"]).unwrap();
    assert_eq!(symbols, vec!["landin_S_eq", "null"]);
}

/// Marker trait → empty Vec.
#[test]
fn test_stdlib_vtable_method_symbols_marker() {
    let symbols = stdlib_vtable_method_symbols("Copy", "S", &[]).unwrap();
    assert!(symbols.is_empty());
}

/// Unknown trait → None.
#[test]
fn test_stdlib_vtable_method_symbols_unknown_trait() {
    assert_eq!(stdlib_vtable_method_symbols("BogusTrait", "S", &[]), None);
    assert_eq!(stdlib_vtable_method_symbols("From", "S", &["from"]), None);
    assert_eq!(stdlib_vtable_method_symbols("", "S", &[]), None);
}

/// Symbols are ordered by slot_index ascending.
#[test]
fn test_stdlib_vtable_method_symbols_ordered() {
    let symbols = stdlib_vtable_method_symbols("PartialEq", "S", &["eq", "ne"]).unwrap();
    // eq → slot 0, ne → slot 1
    assert_eq!(symbols[0], "landin_S_eq");
    assert_eq!(symbols[1], "landin_S_ne");
}

/// Generated strings match codegen's `format!` byte-for-byte (cross-check).
#[test]
fn test_stdlib_vtable_method_symbols_match_codegen_format() {
    let type_str = "S";
    let symbols =
        stdlib_vtable_method_symbols("Clone", type_str, &["clone", "clone_from"]).unwrap();
    // Each provided entry should equal format!("landin_{}_{}", type, method)
    assert_eq!(symbols[0], format!("landin_{type_str}_clone"));
    assert_eq!(symbols[1], format!("landin_{type_str}_clone_from"));
}

/// Arith op symbol matches codegen convention.
#[test]
fn test_stdlib_vtable_method_symbols_arith() {
    let symbols = stdlib_vtable_method_symbols("Add", "Vec", &["add"]).unwrap();
    assert_eq!(symbols, vec!["landin_Vec_add"]);
}

/// Extra method names in `provided` that don't match trait methods are
/// silently ignored (consistent with Stage 5.39 plan behavior).
#[test]
fn test_stdlib_vtable_method_symbols_extra_ignored() {
    let symbols = stdlib_vtable_method_symbols(
        "Clone",
        "S",
        &["clone", "bogus_extra_method", "another_extra"],
    )
    .unwrap();
    // bogus/another_extra are not in Clone's method table — ignored.
    // clone_from is NOT in provided → "null".
    assert_eq!(symbols, vec!["landin_S_clone", "null"]);
}
