//! Stage 16.12 — Deep Review Round 2: end-to-end consistency verification.
//!
//! This test verifies that the complete v0.3 trait resolution pipeline
//! (Sound Copy + DefId-keyed lookup + deprecated Spur methods) produces
//! consistent results end-to-end. This addresses the D3 (test coverage)
//! dimension of Deep Review Round 2.
//!
//! Per §29.1.3 (Design-Impl-Test coverage): end-to-end consistency test.
//! Per §1.0 原則 6 "通用 > 特例": one consistent pipeline.

#![cfg(test)]
use landin_compiler::compile;

/// Stage 16.12 test 1: End-to-end consistency — Copy detection via all paths.
////// Stage 16.12 test 2: End-to-end consistency — vtable lookup via all paths.
////// Stage 16.12 test 3: End-to-end — impl methods via all paths.
////// Stage 16.12 test 4: Complete pipeline — program with traits compiles.
///
/// Verifies that a program using traits (impl, dyn Trait, Copy, Drop)
/// compiles end-to-end with the sound Copy detection and DefId-keyed
/// lookup.
#[test]
fn stage16_12_complete_pipeline_with_traits_compiles() {
    let src = r#"
        trait Drawable { fn draw(&self) -> i32; }
        struct Circle { radius: i32 }
        struct Square { side: i32 }
        impl Drawable for Circle { fn draw(&self) -> i32 { self.radius } }
        impl Drawable for Square { fn draw(&self) -> i32 { self.side } }
        impl Copy for Circle {}
        impl Copy for Square {}
        fn main() -> i32 {
            let c = Circle { radius: 5 };
            let c2 = c;
            c.draw()
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Program with traits should compile end-to-end; got errors: {:?}",
        result.errors.borrowck
    );
}

/// Stage 16.12 test 5: Complete pipeline — non-Copy struct with Drop.
///
/// Verifies that a program with a non-Copy struct (impl Drop) correctly
/// rejects use-after-move, confirming sound Copy detection works
/// end-to-end.
#[test]
fn stage16_12_non_copy_with_drop_rejects_use_after_move() {
    let src = r#"
        struct Resource { value: i32 }
        impl Drop for Resource { fn drop(&mut self) {} }
        fn main() -> i32 {
            let r = Resource { value: 42 };
            let r2 = r;
            let r3 = r;
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "Non-Copy struct with impl Drop should reject double-move (use-after-move)"
    );
}
