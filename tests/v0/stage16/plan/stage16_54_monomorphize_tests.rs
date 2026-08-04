//! Stage 16.54 — Task 11 Phase 3: Monomorphization collection integration tests.
//!
//! Phase 3 implements the `collect_mono_items` function that walks MIR bodies
//! and collects `MonoItem { def_id, substs }` pairs. These tests verify
//! end-to-end that:
//! 1. Generic types in compiled programs produce MonoItems
//! 2. Multiple instantiations of the same generic produce distinct MonoItems
//! 3. The same instantiation used multiple times produces one MonoItem (dedup)
//! 4. Non-generic code produces no MonoItems
//! 5. Nested generics produce nested MonoItems

#![cfg(test)]
use landin_compiler::compile;
use landin_compiler::mir::{collect_mono_items, MonoItem};

// =====================================================================
// §1. Basic MonoItem collection
// =====================================================================

/// Stage 16.54 test 1: Non-generic program produces no MonoItems.
#[test]
fn stage16_54_non_generic_no_mono_items() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    assert!(
        items.is_empty(),
        "Non-generic program should produce no MonoItems, got: {:?}",
        items
    );
}

/// Stage 16.54 test 2: Generic struct instantiation produces a MonoItem.
#[test]
fn stage16_54_generic_struct_produces_mono_item() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<i32> = Box { val: 42 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have at least one Type MonoItem (Box<i32>)
    let has_box_i32 = items.iter().any(|item| {
        matches!(
            item,
            MonoItem::Type { substs, .. } if substs.len() == 1
        )
    });
    assert!(
        has_box_i32,
        "Expected a Type MonoItem with 1 subst (Box<i32>), got: {:?}",
        items
    );
}

/// Stage 16.54 test 3: Two different instantiations produce two MonoItems.
#[test]
fn stage16_54_two_instantiations_two_mono_items() {
    let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<bool> = Box { val: true }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have 2 Type MonoItems (Box<i32> and Box<bool>)
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 2,
        "Expected at least 2 Type MonoItems, got: {}",
        type_items.len()
    );
}

// =====================================================================
// §2. Deduplication
// =====================================================================

/// Stage 16.54 test 4: Same instantiation used twice produces one MonoItem.
#[test]
fn stage16_54_dedup_same_instantiation() {
    let src = "struct Box<T> { val: T } fn main() { let b1: Box<i32> = Box { val: 42 }; let b2: Box<i32> = Box { val: 43 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have exactly 1 Type MonoItem (Box<i32>) — dedup
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { substs, .. } if substs.len() == 1))
        .collect();
    assert_eq!(
        type_items.len(),
        1,
        "Expected exactly 1 Type MonoItem (Box<i32> deduped), got: {}",
        type_items.len()
    );
}

// =====================================================================
// §3. Nested generics
// =====================================================================

/// Stage 16.54 test 5: Nested generic produces nested MonoItems.
///
/// Stage 16.56 fix: The inner `Box<i32>` in `Box<Box<i32>>` is now
/// correctly resolved via `lookup_type_def_id_by_name`. Both the outer
/// `Box<Box<i32>>` and inner `Box<i32>` produce MonoItems.
#[test]
fn stage16_54_nested_generic_produces_nested_mono_items() {
    let src = "struct Box<T> { val: T } fn main() { let b: Box<Box<i32>> = Box { val: Box { val: 42 } }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have 2 Type MonoItems: Box<Box<i32>> (outer) and Box<i32> (inner)
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 2,
        "Expected at least 2 Type MonoItems (outer Box + inner Box), got: {}",
        type_items.len()
    );
}

// =====================================================================
// §4. Generic enum
// =====================================================================

/// Stage 16.54 test 6: Generic enum produces MonoItems.
#[test]
fn stage16_54_generic_enum_produces_mono_items() {
    let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::Some(42); }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have at least one Type MonoItem (Opt<i32>)
    let has_opt_i32 = items.iter().any(|item| {
        matches!(
            item,
            MonoItem::Type { substs, .. } if substs.len() == 1
        )
    });
    assert!(
        has_opt_i32,
        "Expected a Type MonoItem with 1 subst (Opt<i32>), got: {:?}",
        items
    );
}

/// Stage 16.54 test 7: Generic enum with multiple variants.
#[test]
fn stage16_54_generic_enum_multiple_variants() {
    let src = "enum Opt<T> { Some(T), None } fn main() { let x: Opt<i32> = Opt::Some(42); let y: Opt<i32> = Opt::None; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have exactly 1 Type MonoItem (Opt<i32>) — dedup
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { substs, .. } if substs.len() == 1))
        .collect();
    assert_eq!(
        type_items.len(),
        1,
        "Expected exactly 1 Type MonoItem (Opt<i32> deduped), got: {}",
        type_items.len()
    );
}

// =====================================================================
// §5. Multiple generic types
// =====================================================================

/// Stage 16.54 test 8: Multiple generic structs produce multiple MonoItems.
#[test]
fn stage16_54_multiple_generic_structs() {
    let src = r#"struct Box<T> { val: T }
        struct Pair<A, B> { a: A, b: B }
        fn main() {
            let b: Box<i32> = Box { val: 42 };
            let p: Pair<i32, bool> = Pair { a: 1, b: true };
        }"#;
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    // Should have at least 2 Type MonoItems (Box<i32> and Pair<i32, bool>)
    let type_items: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, MonoItem::Type { .. }))
        .collect();
    assert!(
        type_items.len() >= 2,
        "Expected at least 2 Type MonoItems, got: {}",
        type_items.len()
    );
}

// =====================================================================
// §6. No regressions on non-generic code
// =====================================================================

/// Stage 16.54 test 9: Non-generic struct produces no MonoItems.
#[test]
fn stage16_54_non_generic_struct_no_mono_items() {
    let src = "struct Point { x: i32, y: i32 } fn main() { let p = Point { x: 1, y: 2 }; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    assert!(
        items.is_empty(),
        "Non-generic struct should produce no MonoItems, got: {:?}",
        items
    );
}

/// Stage 16.54 test 10: Non-generic enum produces no MonoItems.
#[test]
fn stage16_54_non_generic_enum_no_mono_items() {
    let src = "enum Color { Red, Green, Blue } fn main() { let c = Color::Red; }";
    let result = compile(src);
    assert!(!result.has_errors(), "errors: {:?}", result.errors);
    let items = collect_mono_items(&result.mirs);
    assert!(
        items.is_empty(),
        "Non-generic enum should produce no MonoItems, got: {:?}",
        items
    );
}

// =====================================================================
// §7. MonoItem accessor tests
// =====================================================================

/// Stage 16.54 test 11: MonoItem::def_id accessor works.
#[test]
fn stage16_54_mono_item_def_id_accessor() {
    use landin_compiler::ast::IntTy;
    use landin_compiler::hir::DefId;
    use landin_compiler::mir::ty::{Ty, TyKind};
    use landin_compiler::session::Span;

    let i32_ty = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
    let item = MonoItem::Type {
        def_id: DefId::new(42),
        substs: vec![i32_ty].into(),
    };
    assert_eq!(item.def_id(), DefId::new(42));
}

/// Stage 16.54 test 12: MonoItem::substs accessor works.
#[test]
fn stage16_54_mono_item_substs_accessor() {
    use landin_compiler::ast::IntTy;
    use landin_compiler::hir::DefId;
    use landin_compiler::mir::ty::{Ty, TyKind};
    use landin_compiler::session::Span;

    let i32_ty = Ty::new(TyKind::Int(IntTy::I32), Span::DUMMY);
    let item = MonoItem::Type {
        def_id: DefId::new(42),
        substs: vec![i32_ty].into(),
    };
    assert_eq!(item.substs().len(), 1);
}
