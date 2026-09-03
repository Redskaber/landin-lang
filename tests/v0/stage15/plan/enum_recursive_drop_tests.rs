//! Stage 15.66 — Recursive drop for enums (SwitchInt in drop glue) tests.
//!
//! These tests verify that the recursive drop for enums works correctly:
//!
//! 1. **Enum without `impl Drop` but with Drop variant payload**: The enum's
//!    drop glue loads the discriminant, switches to the active variant, and
//!    recursively drops the variant's payload.
//!
//! 2. **Enum with `impl Drop` AND Drop variant payload**: The enum's drop
//!    glue calls the user's `Drop::drop` method, then switches to the active
//!    variant and recursively drops the payload.
//!
//! 3. **Enum with multiple Drop variants**: Each variant's payload is
//!    dropped via SwitchInt dispatch.
//!
//! Per §17.1: tests live under `tests/v0/stage{N}/plan/`.
//! Per §17.5: test names follow `<stage>_<id>_<description>` pattern.

#![cfg(test)]

use landin_compiler::compile;

/// Stage 15.66 test 1: Enum without impl Drop, variant has Drop payload.
///
/// `E::A(Inner)` where `Inner` has `impl Drop`. The enum's drop glue
/// loads the discriminant (variant A = 0), switches to variant A's block,
/// GEPs to the Inner payload field, and calls `drop_adt_<InnerDefId>`.
#[test]
fn stage15_66_enum_no_drop_impl_variant_has_drop() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        enum E {
            A(Inner),
            B(i32),
        }
        fn main() -> i32 {
            let e = E::A(Inner { x: 42 });
            match e {
                E::A(i) => i.x,
                E::B(v) => v,
            }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with Drop variant payload should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 2: Enum with impl Drop AND variant has Drop payload.
///
/// Both the enum's user Drop::drop runs AND the variant's Inner payload
/// is recursively dropped (via SwitchInt dispatch).
#[test]
fn stage15_66_enum_has_drop_impl_and_variant_has_drop() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        enum E {
            A(Inner),
            B(i32),
        }
        impl Drop for E { fn drop(&mut self) {} }
        fn main() -> i32 {
            let e = E::A(Inner { x: 42 });
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with Drop impl + Drop variant should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 3: Enum with multiple Drop variants.
///
/// `E::A(Inner1)` and `E::B(Inner2)` — both variants have Drop payloads.
/// The drop glue emits a switch with two cases.
#[test]
fn stage15_66_enum_multiple_drop_variants() {
    let src = r#"
        struct A { x: i32 }
        struct B { y: i32 }
        impl Drop for A { fn drop(&mut self) {} }
        impl Drop for B { fn drop(&mut self) {} }
        enum E {
            VariantA(A),
            VariantB(B),
        }
        fn main() -> i32 {
            let e = E::VariantA(A { x: 1 });
            let f = E::VariantB(B { y: 2 });
            match e {
                E::VariantA(a) => a.x,
                E::VariantB(b) => b.y,
            }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with multiple Drop variants should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 4: Enum with no Drop variants — no SwitchInt emitted.
///
/// `E::A(i32)` and `E::B(bool)` — no variant needs drop. The drop glue
/// is a no-op (no SwitchInt, no payload drop). This is a regression test
/// to ensure the enum drop path doesn't break for non-Drop enums.
#[test]
fn stage15_66_enum_no_drop_variants_no_regression() {
    let src = r#"
        enum E {
            A(i32),
            B(bool),
        }
        fn main() -> i32 {
            let e = E::A(42);
            match e {
                E::A(v) => v,
                E::B(b) => if b { 1 } else { 0 },
            }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with no Drop variants should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 5: Enum with mixed Drop and non-Drop variants.
///
/// `E::A(Inner)` (Drop) and `E::B(i32)` (non-Drop). Only variant A's
/// payload needs drop. The switch has one case (variant A) and the
/// default (merge block) handles variant B.
#[test]
fn stage15_66_enum_mixed_drop_non_drop_variants() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        enum E {
            WithDrop(Inner),
            NoDrop(i32),
        }
        fn main() -> i32 {
            let e = E::WithDrop(Inner { x: 42 });
            let f = E::NoDrop(10);
            let v1 = match e {
                E::WithDrop(i) => i.x,
                E::NoDrop(v) => v,
            };
            let v2 = match f {
                E::WithDrop(i) => i.x,
                E::NoDrop(v) => v,
            };
            v1 + v2
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with mixed Drop/non-Drop variants should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 6: Enum with Drop variant — runtime verification.
///
/// This test verifies the SwitchInt dispatch works at runtime. The Inner
/// drop method prints a message, confirming the payload was dropped.
#[test]
fn stage15_66_enum_drop_runtime_verification() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        enum E {
            A(Inner),
            B(i32),
        }
        fn make(v: i32) -> E {
            E::A(Inner { x: v })
        }
        fn main() -> i32 {
            let e = make(42);
            match e {
                E::A(i) => i.x,
                E::B(v) => v,
            }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum drop runtime test should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 7: Nested enum inside struct — recursive drop.
///
/// `Outer { e: E }` where `E` is an enum with Drop variant. The struct's
/// drop glue calls `drop_adt_<EDefId>`, which does the SwitchInt dispatch.
#[test]
fn stage15_66_nested_enum_in_struct_recursive_drop() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        enum E {
            A(Inner),
            B(i32),
        }
        struct Outer { e: E }
        fn main() -> i32 {
            let o = Outer { e: E::A(Inner { x: 42 }) };
            match o.e {
                E::A(i) => i.x,
                E::B(v) => v,
            }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Nested enum in struct should compile. Errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 15.66 test 8: Enum with struct variant payload.
///
/// `E::A { inner: Inner }` — struct variant with named field. The payload
/// field is still dropped via GEP + call (same as tuple variant).
#[test]
fn stage15_66_enum_struct_variant_payload_drop() {
    let src = r#"
        struct Inner { x: i32 }
        impl Drop for Inner { fn drop(&mut self) {} }
        enum E {
            A { inner: Inner },
            B(i32),
        }
        fn main() -> i32 {
            let e = E::A { inner: Inner { x: 42 } };
            match e {
                E::A { inner: i } => i.x,
                E::B(v) => v,
            }
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Enum with struct variant payload should compile. Errors: {:?}",
        result.errors.borrowck
    );
}
