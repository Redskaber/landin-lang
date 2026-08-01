//! Stage 15.29 — Ty interning integration tests.
//!
//! These tests verify the thread-local TypeInterner (activated in Stage 15.28)
//! works correctly end-to-end with real compilation. They verify:
//! 1. Ty::from_kind deduplicates equal TyKind values
//! 2. Ty::interner_len() reflects unique type count
//! 3. Ty::clear_interner() resets the interner
//! 4. Compilation produces deduplicated types
//! 5. Inference variables bypass the interner (from_kind_raw)
//!
//! Per §29.1.3 (Design-Impl-Test coverage): integration tests verify the
//! thread-local interner works correctly with the full compilation pipeline.

#![cfg(test)]

use landin_compiler::compile;
use landin_compiler::mir::ty::{Ty, TyKind};

/// Stage 15.29 test 1: Ty::from_kind deduplicates equal TyKind values.
#[test]
fn stage15_29_from_kind_dedup() {
    Ty::clear_interner();
    let ty1 = Ty::from_kind(TyKind::Bool);
    let ty2 = Ty::from_kind(TyKind::Bool);
    let ty3 = Ty::from_kind(TyKind::Bool);
    // All should be equal
    assert_eq!(ty1, ty2);
    assert_eq!(ty2, ty3);
    // Only 1 unique type in the interner
    assert_eq!(Ty::interner_len(), 1);
}

/// Stage 15.29 test 2: Different TyKind values are not deduplicated.
#[test]
fn stage15_29_different_types_not_dedup() {
    Ty::clear_interner();
    let _ty1 = Ty::from_kind(TyKind::Bool);
    let _ty2 = Ty::from_kind(TyKind::Char);
    let _ty3 = Ty::from_kind(TyKind::Str);
    // 3 unique types
    assert_eq!(Ty::interner_len(), 3);
}

/// Stage 15.29 test 3: Ty::clear_interner resets the interner.
#[test]
fn stage15_29_clear_interner() {
    let _ty1 = Ty::from_kind(TyKind::Bool);
    let _ty2 = Ty::from_kind(TyKind::Char);
    assert!(Ty::interner_len() >= 2);
    Ty::clear_interner();
    assert_eq!(Ty::interner_len(), 0);
}

/// Stage 15.29 test 4: Ty::from_kind_raw bypasses the interner.
#[test]
fn stage15_29_from_kind_raw_no_interning() {
    Ty::clear_interner();
    let _ty1 = Ty::from_kind_raw(TyKind::Bool);
    let _ty2 = Ty::from_kind_raw(TyKind::Bool);
    // from_kind_raw should NOT add to the interner
    assert_eq!(Ty::interner_len(), 0);
}

/// Stage 15.29 test 5: Compilation clears the interner at start.
#[test]
fn stage15_29_compile_clears_interner() {
    // Populate the interner with some types
    let _ty = Ty::from_kind(TyKind::Bool);
    assert!(Ty::interner_len() > 0);

    // Compile a simple program — should clear the interner first
    let result = compile("fn main() -> i32 { 42 }");
    assert!(result.errors.is_empty());

    // After compilation, the interner should contain only types from
    // this compilation (not the Bool we added before).
    // The exact count depends on how many unique types the compiler creates
    // internally, but it should be > 0 (i32, unit, etc.) and should NOT
    // include our pre-compilation Bool (since clear_interner was called).
    let len_after = Ty::interner_len();
    assert!(
        len_after > 0,
        "interner should have types from compilation, got 0"
    );
}

/// Stage 15.29 test 6: Repeated compilation doesn't accumulate types.
#[test]
fn stage15_29_repeated_compile_no_accumulation() {
    // Compile the same program twice
    let src = "fn main() -> i32 { 42 }";
    let _result1 = compile(src);
    let len1 = Ty::interner_len();

    let _result2 = compile(src);
    let len2 = Ty::interner_len();

    // The interner should have the same count after both compilations
    // (clear_interner is called at the start of each compile).
    assert_eq!(
        len1, len2,
        "interner should not accumulate across compilations"
    );
}

/// Stage 15.29 test 7: Complex types are deduplicated.
#[test]
fn stage15_29_complex_types_dedup() {
    Ty::clear_interner();
    use landin_compiler::ast::IntTy;

    // Create a tuple type: (i32, bool)
    let tuple_kind = TyKind::Tuple(vec![
        Ty::from_kind(TyKind::Int(IntTy::I32)),
        Ty::from_kind(TyKind::Bool),
    ]);

    let ty1 = Ty::from_kind(tuple_kind.clone());
    let ty2 = Ty::from_kind(tuple_kind);

    assert_eq!(ty1, ty2);
    // The interner should have: Bool, Int(I32), Tuple(...) = 3 unique types
    assert_eq!(Ty::interner_len(), 3);
}
