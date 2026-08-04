//! Stage 16.56 — Task 11 Phase 4b prerequisite: Nested generic args resolution.
//!
//! Stage 16.56 fixes the limitation where nested generic type paths (e.g.,
//! the inner `Box` in `Box<Box<i32>>`) were lowered as `Error` because
//! `lower_ast_ty_to_mir_ty` couldn't resolve AST paths without HIR context.
//!
//! The fix threads `hir: Option<&HirCrate>` through the type lowering
//! functions. When HIR is available, AST paths in generic args are resolved
//! to DefIds via `lookup_type_def_id_by_name`.
//!
//! These tests verify:
//! 1. Nested generics compile and produce correct MIR types
//! 2. Nested generics produce multiple MonoItems (outer + inner)
//! 3. Triple-nested generics work
//! 4. No regressions on non-nested generics
//! 5. No regressions on non-generic code

#![cfg(test)]
use landin_compiler::compile;
use landin_compiler::mir::{collect_mono_items, MonoItem};

// =====================================================================
// §1. Nested generics — basic compilation
// =====================================================================

/// Stage 16.56 test 1: Box<Box<i32>> compiles without errors.
#[test]
fn stage16_56_nested_generic_box_box_i32() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.56 test 2: Box<Box<bool>> compiles without errors.
#[test]
fn stage16_56_nested_generic_box_box_bool() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<bool>> = Box { val: Box { val: true } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.56 test 3: Triple-nested generic compiles.
#[test]
fn stage16_56_triple_nested_generic() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<Box<i32>>> = Box { val: Box { val: Box { val: 42 } } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

// =====================================================================
// §2. Nested generics — MonoItem collection
// =====================================================================

/// Stage 16.56 test 4: Box<Box<i32>> produces 2 MonoItems (outer + inner).
#[test]
fn stage16_56_nested_generic_produces_two_mono_items() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 2,
        "Expected at least 2 Type MonoItems (outer + inner Box), got: {}",
        type_items.len()
    );
}

/// Stage 16.56 test 5: Triple-nested generic produces 3 MonoItems.
#[test]
fn stage16_56_triple_nested_produces_three_mono_items() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<Box<i32>>> = Box { val: Box { val: Box { val: 42 } } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 3,
        "Expected at least 3 Type MonoItems (triple nested Box), got: {}",
        type_items.len()
    );
}

// =====================================================================
// §3. Nested generics with different inner types
// =====================================================================

/// Stage 16.56 test 6: Box<Box<i32>> and Box<Box<bool>> produce 4 MonoItems.
#[test]
fn stage16_56_nested_different_inner_types() {
    let src = "struct Box<T> { val: T } fn main() { let b1: Box<Box<i32>> = Box { val: Box { val: 42 } }; let b2: Box<Box<bool>> = Box { val: Box { val: true } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have: Box<Box<i32>>, Box<i32>, Box<Box<bool>>, Box<bool> = 4
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 4,
        "Expected at least 4 Type MonoItems (2 outer + 2 inner), got: {}",
        type_items.len()
    );
}

// =====================================================================
// §4. Nested generics with Pair (two type params)
// =====================================================================

/// Stage 16.56 test 7: Pair<Box<i32>, bool> compiles.
#[test]
fn stage16_56_nested_generic_with_pair() {
    let src = "struct Box<T> { val: T } struct Pair<A, B> { a: A, b: B } fn main() { let p: Pair<Box<i32>, bool> = Pair { a: Box { val: 42 }, b: true }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.56 test 8: Pair<Box<i32>, Box<bool>> produces multiple MonoItems.
#[test]
fn stage16_56_nested_generic_pair_of_boxes() {
    let src = "struct Box<T> { val: T } struct Pair<A, B> { a: A, b: B } fn main() { let p: Pair<Box<i32>, Box<bool>> = Pair { a: Box { val: 42 }, b: Box { val: true } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have: Pair<Box<i32>, Box<bool>>, Box<i32>, Box<bool> = 3
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 3,
        "Expected at least 3 Type MonoItems (Pair + 2 Boxes), got: {}",
        type_items.len()
    );
}

// =====================================================================
// §5. No regressions
// =====================================================================

/// Stage 16.56 test 9: Non-nested generic still works.
#[test]
fn stage16_56_non_nested_generic_no_regression() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
}

/// Stage 16.56 test 10: Non-generic code still works.
#[test]
fn stage16_56_non_generic_no_regression() {
    let src = "fn main() -> i32 { 42 }";
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    assert!(items.is_empty());
}
