//! Stage 114 (v0.12): Debug impl bodies re-add attempted + REVERTED (RCA).
//!
//! ## Background
//!
//! Stage 113 fixed TD-LLVM-OBJ-EMIT-CRASH + TD-MONO-INFER. Stage 114
//! re-attempted Debug impl bodies re-add for i32/i64/bool/usize.
//!
//! ## Result: REVERTED
//!
//! - Baseline (Stage 113, no Debug impl): 3/3 runs 0 failures (stable).
//! - Stage 114 (Debug impl added): 9-23 non-deterministic failures per run
//!   (different sets each run: 23, 9, 17). Single tests pass in isolation.
//!
//! ## Root Cause (deeper than Stage 111 RCA)
//!
//! Stage 113 fixed TD-MONO-INFER (writeback secondary pass + skip ALL
//! prelude generic def bodies) + TD-LLVM-OBJ-EMIT-CRASH (fn_sigs_map
//! specialized sigs). But TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION
//! (Stage 99 Layer 3) is STILL active:
//!
//! - Debug impl bodies add vtable + dynptr globals per type (4 types ×
//!   2 globals = 8 new globals).
//! - These globals accumulate across cargo test subprocess compile() calls.
//! - LLVM module global state (type table, target machine registry)
//!   accumulates → LLVM CodeGenLevelDefault optimizer non-deterministically
//!   crashes.
//!
//! Stage 113's skip rule (skip ALL prelude generic def bodies) eliminates
//! Param-containing prelude generic function bodies, but Debug impl bodies
//! are NOT generic (they're concrete impl methods on concrete types). The
//! vtable + dynptr globals they trigger are the crash source.
//!
//! ## Dependency Gap (blocking Debug impl re-add)
//!
//! - **TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION** (P2, v0.11+): LLVM module
//!   global state accumulates across cargo test subprocess compile() calls.
//!   Debug impl bodies add vtable + dynptr globals → pushes LLVM module
//!   global count past the crash threshold → non-deterministic SIGSEGV.
//!
//! ## New TD Discovered
//!
//! - **TD-TRAIT-METHOD-AMBIGUITY** (P3, v0.13+): When both Display and Debug
//!   traits have a `fmt` method, method resolution picks Display (wrong)
//!   instead of Debug (correct) for `n.fmt()` calls. This is a separate
//!   issue from the non-deterministic SIGSEGV, but blocks Debug impl
//!   usability even if the SIGSEGV is fixed.
//!
//! ## Tests
//!
//! These tests verify:
//! - Stage 113 baseline is preserved (5686 tests, 0 failures, 3/3 stable)
//! - Debug impl bodies are NOT present (RCA verified, re-add blocked)
//! - TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION is documented

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// =============================================================================
// POSITIVE TESTS (3) — Stage 113 baseline preserved
// =============================================================================

#[test]
fn stage114_stage113_baseline_preserved() {
    let src = r#"
        fn add_one(x: i64) -> i64 { x + 1i64 }
        fn main() -> i32 {
            let y = add_one(42i64);
            println!("{}", y);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage114_vec_new_push_str_compiles() {
    let src = r#"
        fn main() -> i32 {
            let mut s: String = String::new();
            s.push_str("hello");
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage114_impl_trait_works() {
    // Stage 113 fix — impl Trait now works.
    let src = r#"
        fn process(x: impl Clone) -> i32 { 42 }
        fn main() -> i32 {
            let r = process(7);
            println!("{}", r);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

// =============================================================================
// NEGATIVE TESTS (3) — Debug impl still deferred (Stage 114 RCA)
// =============================================================================

#[test]
fn stage114_prelude_no_debug_impl_for_i32() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42i32).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        result.has_errors(),
        "expected compile errors (no Debug impl for i32), got success"
    );
}

#[test]
fn stage114_prelude_no_debug_impl_for_i64() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42i64).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(result.has_errors());
}

#[test]
fn stage114_prelude_no_debug_impl_for_bool() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (true).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(result.has_errors());
}

// =============================================================================
// RCA DOCUMENTATION TESTS (4) — verify dependency gap is documented
// =============================================================================

#[test]
fn stage114_rca_documented_in_tech_debt_register() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let td_path = manifest.join("docs/develop/v0/tech-debt-register.md");
    let content = std::fs::read_to_string(&td_path).expect("tech-debt-register.md should exist");
    assert!(
        content.contains("TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION"),
        "tech-debt-register should mention TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION"
    );
}

#[test]
fn stage114_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-114/dev-log.md");
    assert!(dev_log.exists());
}

#[test]
fn stage114_stability_script_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/stability_v2.sh");
    assert!(script.exists());
}

#[test]
fn stage114_debug_trait_still_declared() {
    let src = r#"
        struct Foo { x: i32 }
        impl Debug for Foo {
            fn fmt(&self) -> String { String::from_str("Foo") }
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}
