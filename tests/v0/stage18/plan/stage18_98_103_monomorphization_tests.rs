//! Stage 18.98-18.103: Monomorphization tests (relocated from stage2/typeck_tests.rs)
//!
//! Tests for Adt substs soundness, FnDef↔FnPtr soundness, turbofish/implicit
//! inference, per-mono codegen, and call-site specialized names.
//!
//! Per §8 doc organization: tests live under their stage number directory.
//! Per §9.4.3: positive:negative ratio ≥ 1:3.

use landin_compiler::compile;
use landin_compiler::mir::collect_mono_items;

// =================================================================
// Stage 18.98: Adt Substs Soundness Tests
// =================================================================

#[test]
fn stage18_98_adt_substs_mismatch_rejected() {
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<i32> = Vec { data: 42, len: 1 };
    let v2: Vec<bool> = Vec { data: true, len: 1 };
    let v3: Vec<i32> = v2;
}
"#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Vec<i32> = Vec<bool> must be rejected (soundness)"
    );
}

#[test]
fn stage18_98_adt_substs_match_accepted() {
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<i32> = Vec { data: 42, len: 1 };
    let v2: Vec<i32> = v1;
}
"#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Vec<i32> = Vec<i32> should be accepted"
    );
}

#[test]
fn stage18_98_adt_empty_substs_inference() {
    let src = r#"
struct Wrapper<T> { inner: T }
fn make<T>(x: T) -> Wrapper<T> { Wrapper { inner: x } }
fn main() {
    let w: Wrapper<i32> = make(42);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors(), "empty-substs inference should work");
}

// =================================================================
// Stage 18.99: Nested Adt + FnDef↔FnPtr Soundness Tests
// =================================================================

#[test]
fn stage18_99_nested_adt_substs_mismatch_rejected() {
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<Vec<i32>> = Vec { data: Vec { data: 42, len: 1 }, len: 1 };
    let v2: Vec<Vec<bool>> = Vec { data: Vec { data: true, len: 1 }, len: 1 };
    let v3: Vec<Vec<i32>> = v2;
}
"#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Vec<Vec<i32>> = Vec<Vec<bool>> must be rejected (nested substs soundness)"
    );
}

#[test]
fn stage18_99_nested_adt_substs_match_accepted() {
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<Vec<i32>> = Vec { data: Vec { data: 42, len: 1 }, len: 1 };
    let v2: Vec<Vec<i32>> = v1;
}
"#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Vec<Vec<i32>> = Vec<Vec<i32>> should be accepted"
    );
}

#[test]
fn stage18_99_fndef_fnptr_sig_mismatch_rejected() {
    let src = r#"
fn add_one(x: i32) -> i32 { x + 1 }
fn main() {
    let f: fn(bool) -> i32 = add_one;
}
"#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "fn(i32)->i32 assigned to fn(bool)->i32 must be rejected (TD-13)"
    );
}

#[test]
fn stage18_99_fndef_fnptr_sig_match_accepted() {
    let src = r#"
fn add_one(x: i32) -> i32 { x + 1 }
fn main() {
    let f: fn(i32) -> i32 = add_one;
}
"#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "fn(i32)->i32 assigned to fn(i32)->i32 should be accepted"
    );
}

// =================================================================
// Stage 18.101: Turbofish Monomorphization Tests
// =================================================================

#[test]
fn stage18_101_turbofish_produces_mono_item() {
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id::<i32>(42);
    let b: bool = id::<bool>(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        2,
        "turbofish calls should produce 2 MonoItems"
    );
}

#[test]
fn stage18_101_non_generic_no_mono_items() {
    let src = r#"
fn add(x: i32, y: i32) -> i32 { x + y }
fn main() {
    let a = add(1, 2);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        0,
        "non-generic call should produce 0 Fn MonoItems"
    );
}

// =================================================================
// Stage 18.102: Implicit Generic Inference Tests
// =================================================================

#[test]
fn stage18_102_implicit_inference_produces_mono_item() {
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id(42);
    let b: bool = id(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(
        fn_items.len(),
        2,
        "implicit calls should produce 2 MonoItems"
    );
}

#[test]
fn stage18_102_non_generic_implicit_no_mono_items() {
    let src = r#"
fn add(x: i32, y: i32) -> i32 { x + y }
fn main() {
    let a = add(1, 2);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(fn_items.len(), 0);
}

#[test]
fn stage18_102_mixed_turbofish_and_implicit() {
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id::<i32>(42);
    let b: bool = id(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let items = collect_mono_items(&result.mirs);
    let fn_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, landin_compiler::mir::MonoItem::Fn { .. }))
        .collect();
    assert_eq!(fn_items.len(), 2);
}

// =================================================================
// Stage 18.103: Per-Mono Codegen Tests
// =================================================================

#[test]
fn stage18_103_turbofish_produces_specialized_functions() {
    use landin_compiler::codegen::codegen_crate;
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id::<i32>(42);
    let b: bool = id::<bool>(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let ir = codegen_crate(&result).expect("codegen should succeed for valid test input");
    assert!(
        ir.contains("define i32 @id_i32("),
        "expected specialized function id_i32 in IR"
    );
    assert!(
        ir.contains("define i1 @id_bool("),
        "expected specialized function id_bool in IR"
    );
}

#[test]
fn stage18_103_calls_use_specialized_names() {
    use landin_compiler::codegen::codegen_crate;
    let src = r#"
fn id<T>(x: T) -> T { x }
fn main() {
    let a: i32 = id::<i32>(42);
    let b: bool = id::<bool>(true);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let ir = codegen_crate(&result).expect("codegen should succeed for valid test input");
    assert!(
        ir.contains("call i32 @landin_id_i32("),
        "expected call to landin_id_i32 in IR"
    );
    assert!(
        ir.contains("call i1 @landin_id_bool("),
        "expected call to landin_id_bool in IR"
    );
}

#[test]
fn stage18_103_non_generic_uses_base_name() {
    use landin_compiler::codegen::codegen_crate;
    let src = r#"
fn add(x: i32, y: i32) -> i32 { x + y }
fn main() {
    let a = add(1, 2);
}
"#;
    let result = compile(src);
    assert!(!result.has_errors());
    let ir = codegen_crate(&result).expect("codegen should succeed for valid test input");
    assert!(
        ir.contains("define i32 @landin_add("),
        "expected base function landin_add in IR"
    );
    assert!(
        !ir.contains("landin_add_"),
        "non-generic function should not have specialized variants"
    );
}
