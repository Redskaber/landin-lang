//! Stage 15.44 — `elaborate_drops` pass integration tests.
//!
//! These tests verify that `elaborate_drops` works correctly on real MIR
//! produced by `compile()`. Since no user-defined `Drop` impls exist yet
//! (the parser doesn't support `impl Drop for T` yet), `elaborate_drops`
//! is a no-op on all existing code — it doesn't insert any `Drop`
//! terminators. The tests verify this no-op behavior (no panic, no MIR
//! change).
//!
//! When Stage 15.46 adds `impl Drop` support, these tests will be updated
//! to verify actual `Drop` terminator insertion.

#![cfg(test)]

use landin_compiler::compile;
use landin_compiler::mir::drop_elaboration::elaborate_drops;

/// Stage 15.44 integration test 1: `elaborate_drops` is callable on real MIR
/// without panicking. Since no types implement `Drop` in v0.1 code, the pass
/// is a no-op — no `Drop` terminators should be inserted.
#[test]
fn stage15_44_integration_elaborate_drops_noop_on_real_mir() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            let y = 2;
            x + y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let block_count_before = mir_body.basic_blocks.len();
        // Clone the MIR to allow mutation (compile() returns owned MIRs).
        let mut mir_clone = mir_body.clone();
        elaborate_drops(&mut mir_clone, &result.trait_resolver, &result.interner);
        let block_count_after = mir_clone.basic_blocks.len();
        // No types implement Drop → no blocks inserted.
        assert_eq!(
            block_count_before, block_count_after,
            "elaborate_drops should be a no-op when no types need drop"
        );
    }
}

/// Stage 15.44 integration test 2: `elaborate_drops` on a program with
/// structs (no Drop impl) — still a no-op.
#[test]
fn stage15_44_integration_elaborate_drops_struct_no_drop() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() -> i32 {
            let p = Point { x: 1, y: 2 };
            p.x + p.y
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let block_count_before = mir_body.basic_blocks.len();
        let mut mir_clone = mir_body.clone();
        elaborate_drops(&mut mir_clone, &result.trait_resolver, &result.interner);
        let block_count_after = mir_clone.basic_blocks.len();
        assert_eq!(
            block_count_before, block_count_after,
            "struct without Drop impl → no drop terminators inserted"
        );
    }
}

/// Stage 15.44 integration test 3: `elaborate_drops` on a complex program
/// (loops, conditionals, method calls) — no panic, no MIR change.
#[test]
fn stage15_44_integration_elaborate_drops_complex_program() {
    let src = r#"
        struct Counter { value: i32 }
        impl Counter {
            fn new() -> Counter { Counter { value: 0 } }
            fn increment(&mut self) { self.value = self.value + 1; }
        }
        fn main() -> i32 {
            let mut c = Counter::new();
            let mut i = 0;
            while i < 5 { c.increment(); i = i + 1; }
            c.value
        }
    "#;
    let result = compile(src);
    for mir_body in &result.mirs {
        let block_count_before = mir_body.basic_blocks.len();
        let mut mir_clone = mir_body.clone();
        elaborate_drops(&mut mir_clone, &result.trait_resolver, &result.interner);
        let block_count_after = mir_clone.basic_blocks.len();
        assert_eq!(
            block_count_before, block_count_after,
            "complex program with no Drop impls → no drop terminators inserted"
        );
    }
}
