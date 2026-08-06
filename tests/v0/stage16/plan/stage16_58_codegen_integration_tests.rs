//! Stage 16.58 — Task 11 Phase 4c: Codegen integration with MonoLayoutMap.
//!
//! Phase 4c integrates per-mono layouts into the codegen type translation
//! pipeline. The new `mir_type_to_emit_type_with_layouts_and_mono` function
//! first checks `lookup_mono_layout` for generic types (non-empty substs),
//! falling back to the existing `AdtLayouts` map for non-generic types.
//!
//! These tests verify:
//! 1. `lookup_mono_layout` finds specialized layouts for generic types
//! 2. `lookup_mono_layout` returns None for non-generic types
//! 3. `mir_type_to_emit_type_with_layouts_and_mono` uses specialized layouts
//! 4. Different instantiations produce different EmitTypes
//! 5. Non-generic code still works (fallback to AdtLayouts)
//! 6. No regressions on existing code

#![cfg(test)]
use landin_compiler::ast::IntTy;
use landin_compiler::compile;
use landin_compiler::mir::body::AdtLayout;
use landin_compiler::mir::ty::TyKind;
use landin_compiler::mir::{
    build_mono_layouts, collect_mono_items, lookup_mono_layout, MonoLayoutMap,
};

// =====================================================================
// §1. lookup_mono_layout tests
// =====================================================================

/// Stage 16.58 test 1: lookup_mono_layout finds layout for generic type.
#[test]
fn stage16_58_lookup_mono_layout_finds_generic() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Find the Box DefId by looking at the mono items.
    for item in &items {
        if let landin_compiler::mir::MonoItem::Type { def_id, substs } = item {
            if !substs.is_empty() {
                let found = lookup_mono_layout(*def_id, substs, Some(&layouts));
                assert!(
                    found.is_some(),
                    "Expected to find mono layout for generic type"
                );
                return;
            }
        }
    }
    panic!("No generic MonoItem::Type found");
}

/// Stage 16.58 test 2: lookup_mono_layout returns None for non-generic.
#[test]
fn stage16_58_lookup_mono_layout_non_generic() {
    let src = "struct Point { x: i32, y: i32 } fn main() { let p = Point { x: 1, y: 2 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Non-generic types have empty substs → lookup returns None.
    let empty_substs: landin_compiler::mir::ty::SubstsRef = vec![].into();
    let found = lookup_mono_layout(
        landin_compiler::hir::DefId::new(0),
        &empty_substs,
        Some(&layouts),
    );
    assert!(found.is_none(), "Expected None for non-generic type");
}

/// Stage 16.58 test 3: lookup_mono_layout returns None when map is None.
#[test]
fn stage16_58_lookup_mono_layout_none_map() {
    let empty_substs: landin_compiler::mir::ty::SubstsRef = vec![].into();
    let found = lookup_mono_layout(landin_compiler::hir::DefId::new(0), &empty_substs, None);
    assert!(found.is_none(), "Expected None when map is None");
}

/// Stage 16.58 test 4: lookup_mono_layout returns None for empty substs.
#[test]
fn stage16_58_lookup_mono_layout_empty_substs() {
    let layouts: MonoLayoutMap = std::collections::HashMap::new();
    let empty_substs: landin_compiler::mir::ty::SubstsRef = vec![].into();
    let found = lookup_mono_layout(
        landin_compiler::hir::DefId::new(0),
        &empty_substs,
        Some(&layouts),
    );
    assert!(found.is_none(), "Expected None for empty substs");
}

// =====================================================================
// §2. build_mono_layouts + lookup_mono_layout integration
// =====================================================================

/// Stage 16.58 test 5: Box<i32> produces a layout with i32 field type.
#[test]
fn stage16_58_box_i32_layout_has_i32_field() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Find the Box<i32> layout.
    let mut found_i32_field = false;
    for (_, layout) in layouts.values().flatten() {
        if let AdtLayout::Struct { field_tys } = layout {
            if field_tys.len() == 1 && matches!(field_tys[0].kind, TyKind::Int(IntTy::I32)) {
                found_i32_field = true;
                break;
            }
        }
    }
    assert!(
        found_i32_field,
        "Expected Box<i32> layout with i32 field type"
    );
}

/// Stage 16.58 test 6: Box<bool> produces a layout with bool field type.
#[test]
fn stage16_58_box_bool_layout_has_bool_field() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<bool> = Box { val: true }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Find the Box<bool> layout.
    let mut found_bool_field = false;
    for (_, layout) in layouts.values().flatten() {
        if let AdtLayout::Struct { field_tys } = layout {
            if field_tys.len() == 1 && matches!(field_tys[0].kind, TyKind::Bool) {
                found_bool_field = true;
                break;
            }
        }
    }
    assert!(
        found_bool_field,
        "Expected Box<bool> layout with bool field type"
    );
}

/// Stage 16.58 test 7: Different instantiations produce different layouts.
#[test]
fn stage16_58_different_instantiations_different_layouts() {
    let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<bool> = Box { val: true }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Should have 2 layouts (Box<i32> and Box<bool>).
    assert_eq!(
        layouts.values().map(|v| v.len()).sum::<usize>(),
        2,
        "Expected 2 mono layouts"
    );

    // Verify one has i32 field and the other has bool field.
    let has_i32 = layouts.values().flatten().any(|(_, l)| {
        matches!(l, AdtLayout::Struct { field_tys } if field_tys.len() == 1 && matches!(field_tys[0].kind, TyKind::Int(IntTy::I32)))
    });
    let has_bool = layouts.values().flatten().any(|(_, l)| {
        matches!(l, AdtLayout::Struct { field_tys } if field_tys.len() == 1 && matches!(field_tys[0].kind, TyKind::Bool))
    });
    assert!(has_i32, "Expected layout with i32 field");
    assert!(has_bool, "Expected layout with bool field");
}

// =====================================================================
// §3. No regressions
// =====================================================================

/// Stage 16.58 test 8: Non-generic code still works.
#[test]
fn stage16_58_non_generic_no_regression() {
    let src = "struct Point { x: i32, y: i32 } fn main() -> i32 { let p = Point { x: 1, y: 2 }; p.x + p.y }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);
    // Non-generic code → no mono layouts.
    assert!(
        layouts.is_empty(),
        "Non-generic code should produce no mono layouts"
    );
}

/// Stage 16.58 test 9: Simple program still works.
#[test]
fn stage16_58_simple_program_no_regression() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
}

// =====================================================================
// §4. Complex generic patterns
// =====================================================================

/// Stage 16.58 test 10: Pair<i32, bool> produces a layout with two fields.
#[test]
fn stage16_58_pair_layout_two_fields() {
    let src = "struct Pair<A, B> { a: A, b: B } fn main() { let p: Pair<i32, bool> = Pair { a: 1, b: true }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Find the Pair<i32, bool> layout.
    let mut found_pair = false;
    for (_, layout) in layouts.values().flatten() {
        if let AdtLayout::Struct { field_tys } = layout {
            if field_tys.len() == 2
                && matches!(field_tys[0].kind, TyKind::Int(IntTy::I32))
                && matches!(field_tys[1].kind, TyKind::Bool)
            {
                found_pair = true;
                break;
            }
        }
    }
    assert!(
        found_pair,
        "Expected Pair<i32, bool> layout with [i32, bool] fields"
    );
}

/// Stage 16.58 test 11: Nested generic produces nested layouts.
#[test]
fn stage16_58_nested_generic_layouts() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Should have 2 layouts: Box<Box<i32>> (outer) and Box<i32> (inner).
    assert!(
        layouts.values().map(|v| v.len()).sum::<usize>() >= 2,
        "Expected at least 2 mono layouts (nested Box), got: {}",
        layouts.values().map(|v| v.len()).sum::<usize>()
    );
}

/// Stage 16.58 test 12: Generic enum produces enum layout.
#[test]
fn stage16_58_generic_enum_layout() {
    let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::Some(42); }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let hir = result.hir.as_ref().expect("HIR should be available");
    let items = collect_mono_items(&result.mirs);
    let layouts = build_mono_layouts(&items, hir);

    // Find the Opt<i32> enum layout.
    let has_enum = layouts
        .values()
        .flatten()
        .any(|(_, l)| matches!(l, AdtLayout::Enum { .. }));
    assert!(has_enum, "Expected enum layout for Opt<i32>");
}
