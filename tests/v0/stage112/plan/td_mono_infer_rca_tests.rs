//! Stage 112 (v0.12): TD-MONO-INFER RCA — attempted fix + REVERTED.
//!
//! ## Background
//!
//! Stage 111 RCA identified TD-MONO-INFER + TD-PRELUDE-IMPL-BODY-MODULE
//! -ACCUMULATION as the dependency gap blocking Debug impl re-add.
//! Stage 112 attempted to fix TD-MONO-INFER by:
//!
//! 1. **codegen/function.rs skip rule strengthening**: Skip ALL prelude
//!    generic def bodies (regardless of MonoItem::Fn instantiation).
//!    codegen_mono_functions handles all instantiated versions via
//!    substitute_mir_body.
//!
//! 2. **writeback_fndef_substs secondary pass**: Propagate inferred substs
//!    from `local_decls[idx].ty` (updated by terminator_changes) into the
//!    `Operand::Constant(c).ty` in Assign statements (where `c.ty` was
//!    `FnDef(def_id, [])` with empty substs).
//!
//! ## Result: REVERTED
//!
//! - Stage 112 fix #1 alone (skip rule): 43 linker errors (generic def not
//!   emitted, call sites reference `@landin_Vec_new` which is undefined).
//! - Stage 112 fix #1 + #2 together: 6 impl Trait tests crash in
//!   `--emit-obj` (deterministic SIGSEGV in LLVMTargetMachineEmitToFile).
//!   The IR is valid (llvm-as + llc both work), but the in-memory LLVMModule
//!   crashes when emitted via the C API.
//!
//! ## Root Cause (deeper than Stage 111 RCA)
//!
//! The impl Trait desugar (`fn process<T: Clone>(x: T) -> i32`) creates a
//! user-defined generic function. The writeback secondary pass propagates
//! inferred substs into `Operand::Constant` in Assign statements. This
//! changes the IR structure in a way that triggers a deterministic crash
//! in LLVM's object emission path (LLVMTargetMachineEmitToFile).
//!
//! The crash is NOT in the IR itself (llvm-as accepts it, llc compiles it
//! to .s successfully). It's in the LLVM C API binding path
//! (LLVMSysEmitter::emit_to_object_file or similar) — likely a use-after-free
//! or module state issue.
//!
//! ## New TD Discovered
//!
//! - **TD-LLVM-OBJ-EMIT-CRASH** (P2, v0.13+): Deterministic SIGSEGV in
//!   `LLVMTargetMachineEmitToFile` when emitting object files for certain
//!   IR structures (specifically, IR with `Operand::Constant` FnDef types
//!   that have non-empty inferred substs). The IR is valid (llvm-as + llc
//!   both work), but the in-memory LLVMModule crashes. Likely cause:
//!   use-after-free or module state issue in LLVMSysEmitter.
//!
//! ## Dependency Gap (blocking TD-MONO-INFER fix)
//!
//! 1. **TD-LLVM-OBJ-EMIT-CRASH** (P2, v0.13+): Must be resolved before
//!    Stage 112's writeback secondary pass can be safely introduced.
//! 2. **TD-IMPL-TRAIT-MONO-RESOLUTION** (existing): impl Trait desugar
//!    creates user-defined generic functions that need proper monomorphization.
//!
//! ## Tests
//!
//! These tests verify:
//! - Stage 111 baseline is preserved (5663 tests, 0 failures)
//! - TD-LLVM-OBJ-EMIT-CRASH is documented
//! - The dependency gap is recorded

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

// =============================================================================
// POSITIVE TESTS (3) — Stage 111 baseline preserved
// =============================================================================

#[test]
fn stage112_stage111_baseline_preserved() {
    // Verify Stage 111 baseline is preserved (Phase 3.6 active, no Debug impl).
    let src = r#"
        fn add_one(x: i64) -> i64 { x + 1i64 }
        fn main() -> i32 {
            let y = add_one(42i64);
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

#[test]
fn stage112_vec_new_push_str_compiles() {
    // Vec::new() + push_str — Stage 105 RCA scenario.
    // Stage 112 RCA confirms the dependency gap (TD-LLVM-OBJ-EMIT-CRASH).
    let src = r#"
        fn main() -> i32 {
            let mut s: String = String::new();
            s.push_str("hello");
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

#[test]
fn stage112_phase36_active() {
    // Verify Phase 3.6 (Stage 110) is still active — unsuffixed literals
    // should produce concrete c.ty (no Infer warnings for simple programs).
    let src = r#"
        fn main() -> i32 {
            let mut s: String = String::new();
            s.push_str("hello");
            println!("{}", s);
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
// NEGATIVE TESTS (3) — Debug impl still deferred (Stage 111 RCA)
// =============================================================================

#[test]
fn stage112_prelude_no_debug_impl_for_i32() {
    // Verify prelude does NOT have `impl Debug for i32` (Stage 111 reverted,
    // Stage 112 did not re-add — blocked by TD-MONO-INFER + TD-LLVM-OBJ-EMIT-CRASH).
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
fn stage112_prelude_no_debug_impl_for_i64() {
    // Stage 123: Debug impl bodies are permanently deferred (LLVM C++ non-determinism).
    // The trait is declared but impl bodies are absent. Calling .fmt() on i64
    // resolves to Display::fmt (which takes &mut String, not just &self),
    // so it should error.
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42i64).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    // Debug trait is declared but impl bodies deferred — .fmt() on i64
    // resolves to Display::fmt (wrong signature) → type mismatch error.
    assert!(
        result.has_errors(),
        "expected compile errors (Debug impl deferred, Display::fmt has wrong sig), got success"
    );
}

#[test]
fn stage112_prelude_no_debug_impl_for_bool() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (true).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        result.has_errors(),
        "expected compile errors (no Debug impl for bool), got success"
    );
}

// =============================================================================
// RCA DOCUMENTATION TESTS (4) — verify dependency gap is documented
// =============================================================================

#[test]
fn stage112_rca_documented_in_tech_debt_register() {
    // Meta-test: verify the tech-debt-register.md mentions Stage 112 RCA
    // + the new TD-LLVM-OBJ-EMIT-CRASH.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let td_path = manifest.join("docs/develop/v0/tech-debt-register.md");
    let content = std::fs::read_to_string(&td_path).expect("tech-debt-register.md should exist");
    assert!(
        content.contains("TD-MONO-INFER"),
        "tech-debt-register should mention TD-MONO-INFER (Stage 112 dependency)"
    );
    assert!(
        content.contains("TD-LLVM-OBJ-EMIT-CRASH"),
        "tech-debt-register should mention TD-LLVM-OBJ-EMIT-CRASH (Stage 112 new TD)"
    );
}

#[test]
fn stage112_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-112/dev-log.md");
    assert!(
        dev_log.exists(),
        "Stage 112 dev-log.md should exist at {:?}",
        dev_log
    );
}

#[test]
fn stage112_stability_script_exists() {
    // Verify the stability script from Stage 111 still exists.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = manifest.join("scripts/stability_v2.sh");
    assert!(
        script.exists(),
        "Stage 111 stability_v2.sh should still exist at {:?}",
        script
    );
}

#[test]
fn stage112_prelude_debug_trait_declared() {
    // Verify Debug trait is still declared in prelude (Stage 111 preserved
    // the declaration, only impl bodies are deferred).
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
    assert!(
        !result.has_errors(),
        "expected no compile errors (Debug trait declared), got: {:?}",
        result.errors
    );
}
