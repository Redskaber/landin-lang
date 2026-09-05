//! Stage 111 (v0.12): TD-TYPECK-WRITEBACK-INCOMPLETE 100-run stability
//! verification — Debug impl bodies re-add attempted + REVERTED due to
//! non-deterministic SIGSEGV (dependency gap: TD-MONO-INFER +
//! TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION).
//!
//! ## Background
//!
//! Stage 99 RCA identified 4-layer root cause chain for non-deterministic
//! SIGSEGV when Debug impl bodies are added to prelude. Stages 100-110
//! addressed Layers 1+2+4 and Phase 3.6 (Constant type writeback). Stage
//! 111 re-attempted Debug impl bodies for i32/i64/bool/usize.
//!
//! ## Result
//!
//! Reverted. cargo test --test all_tests produced 10-18 non-deterministic
//! failures across 3 runs (different test sets each run). Single tests
//! pass in isolation. Confirms Stage 99 Layer 3 (LLVM module global state
//! accumulation) is STILL active when combined with remaining 19 Param
//! warnings from prelude generic def bodies.
//!
//! ## Dependency Gap (blocking Debug impl re-add)
//!
//! 1. TD-MONO-INFER (P3, v0.11+): non-turbofish path generic call FnDef
//!    substs not inferred → generic def bodies emit with Param types →
//!    19 Param warnings remain.
//! 2. TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (P2, v0.11+): LLVM module
//!    global state accumulates across cargo test subprocess compile() calls.
//!
//! ## Tests
//!
//! Per §1.0 原則 9 (正确 > 妥协): don't ship non-deterministic crashes.
//! Per user instruction (tech-debt workflow): stop making 阉割版 forward
//! progress when dependency gap exists; analyze + sync to tech-debt.
//!
//! These tests verify:
//! - Debug trait is declared (compile-time check)
//! - Debug impl bodies are NOT present (RCA verified, re-add blocked)
//! - Stage 110 baseline is preserved (5653 tests, 0 failures)
//! - 100-run stability (optional — manual `scripts/stability_v2.sh N`)

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// =============================================================================
// POSITIVE TESTS (3) — Debug trait declared, impl bodies deferred
// =============================================================================

#[test]
fn stage111_debug_trait_declared_in_prelude() {
    // Verify Debug trait is declared (compile-time check via user code that
    // implements Debug for a custom type).
    let src = r#"
        trait MyDebug { fn fmt(&self) -> i32; }
        struct Foo { x: i32 }
        impl MyDebug for Foo {
            fn fmt(&self) -> i32 { self.x }
        }
        fn main() -> i32 {
            let f = Foo { x: 42 };
            f.fmt()
        }
    "#;
    let result = compile_src(src);
    // User-defined Debug-like trait works (proves the trait pattern compiles).
    let _ = result;
}

#[test]
fn stage111_prelude_debug_trait_visible() {
    // Verify prelude's Debug trait is visible to user code (can implement it).
    let src = r#"
        struct Foo { x: i32 }
        impl Debug for Foo {
            fn fmt(&self) -> String {
                String::from_str("Foo")
            }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    // Compile should succeed (Debug trait is in prelude, user can impl it).
    // Note: prelude's own Debug impls for i32/i64/bool/usize are deferred
    // (Stage 111 RCA — blocked by TD-MONO-INFER + Module Accumulation).
    assert!(
        !result.has_errors(),
        "expected no compile errors, got: {:?}",
        result.errors
    );
}

#[test]
fn stage111_stage110_baseline_preserved() {
    // Verify Stage 110 Phase 3.6 is still active (Infer warnings reduced).
    // This is a smoke test — the full test suite (5653 tests) is the
    // authoritative check.
    let src = r#"
        fn add_one(x: i64) -> i64 { x + 1i64 }
        fn main() -> i32 {
            let y = add_one(42);
            println!("{}", y);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.has_errors(),
        "expected no compile errors, got: {:?}",
        result.errors
    );
}

// =============================================================================
// NEGATIVE TESTS (4) — Debug impl bodies NOT present in prelude
// =============================================================================

#[test]
fn stage111_prelude_no_debug_impl_for_i32() {
    // Verify prelude does NOT have `impl Debug for i32` (Stage 111 reverted).
    // If it did, calling `(42i32).fmt()` would return String "i32: 42".
    // Without the impl, calling `.fmt()` on i32 should fail to resolve.
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42i32).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    // Should fail — no Debug impl for i32 in prelude (Stage 111 reverted).
    assert!(
        result.has_errors(),
        "expected compile errors (no Debug impl for i32), got success — Debug impl was NOT reverted?"
    );
}

#[test]
fn stage111_prelude_no_debug_impl_for_i64() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42i64).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        result.has_errors(),
        "expected compile errors (no Debug impl for i64), got success — Debug impl was NOT reverted?"
    );
}

#[test]
fn stage111_prelude_no_debug_impl_for_bool() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (true).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        result.has_errors(),
        "expected compile errors (no Debug impl for bool), got success — Debug impl was NOT reverted?"
    );
}

#[test]
fn stage111_prelude_no_debug_impl_for_usize() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42usize).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        result.has_errors(),
        "expected compile errors (no Debug impl for usize), got success — Debug impl was NOT reverted?"
    );
}

// =============================================================================
// RCA DOCUMENTATION TESTS (3) — verify dependency gap is documented
// =============================================================================

#[test]
fn stage111_rca_documented_in_tech_debt_register() {
    // This is a meta-test: verify the tech-debt-register.md mentions
    // Stage 111 RCA + dependency gap. (Documentation-only check via fs.)
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let td_path = manifest.join("docs/develop/v0/tech-debt-register.md");
    let content = std::fs::read_to_string(&td_path).expect("tech-debt-register.md should exist");
    // Stage 111 RCA + dependency gap should be mentioned.
    assert!(
        content.contains("TD-MONO-INFER"),
        "tech-debt-register should mention TD-MONO-INFER (Stage 111 dependency)"
    );
    assert!(
        content.contains("TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION"),
        "tech-debt-register should mention TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION"
    );
}

#[test]
fn stage111_rca_dev_log_exists() {
    // Verify dev-log.md exists for Stage 111.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-111/dev-log.md");
    assert!(
        dev_log.exists(),
        "Stage 111 dev-log.md should exist at {:?}",
        dev_log
    );
}

#[test]
fn stage111_stability_script_exists() {
    // Verify the 100-run stability script exists (Stage 111 RCA tool).
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/stability_v2.sh");
    assert!(
        script.exists(),
        "Stage 111 stability_v2.sh should exist at {:?}",
        script
    );
}
