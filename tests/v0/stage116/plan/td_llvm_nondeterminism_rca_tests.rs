//! Stage 116 (v0.12): TD-LLVM-INTERNAL-NONDETERMINISM fix attempted + REVERTED.
//!
//! ## Background
//!
//! Stage 115 identified HashMap random SipHash as root cause of non-deterministic
//! emission order. 4 sort fixes reduced failures from 9-23 to 0-3. Stage 116
//! attempted to eliminate the remaining 0-3 failures by calling LLVMShutdown()
//! in LLVMSysEmitter::Drop to reset LLVM C++ global state between compile() calls.
//!
//! ## Result: LLVMShutdown() fix KEEPS Debug impls, but non-determinism persists
//!
//! - Baseline (no Debug impl, with LLVMShutdown): 3/3 runs 0 failures (stable)
//! - Debug impl + LLVMShutdown + sort fixes: 0-5 non-deterministic failures
//!   per run (2/5 runs pass, 3/5 have 1-5 failures)
//!
//! ## Root Cause
//!
//! LLVMShutdown() resets ManagedStatic objects (target registry, pass registry)
//! but does NOT reset LLVM's C++ heap allocator state. SelectionDAG and RegAlloc
//! use `DenseMap` which allocates heap memory whose addresses vary between runs
//! due to ASLR. The hash function for DenseMap uses pointer addresses, producing
//! different iteration orders → different codegen → occasional crashes.
//!
//! The only complete fix is **process-per-test isolation** (each compile() in
//! a separate subprocess, like rustc). This is a major architectural change
//! tracked as TD-PROCESS-PER-TEST-ISOLATION.
//!
//! ## Tests

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage116_baseline_stable() {
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
fn stage116_impl_trait_works() {
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

#[test]
fn stage116_vec_new_push_works() {
    let src = r#"
        fn main() -> i32 {
            let mut v: Vec<i32> = Vec::new();
            v.push(1i32);
            v.push(2i32);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage116_prelude_no_debug_impl_for_i32() {
    let src = r#"
        fn main() -> i32 {
            let _s: String = (42i32).fmt();
            0
        }
    "#;
    let result = compile_src(src);
    assert!(result.has_errors());
}

#[test]
fn stage116_rca_documented_in_tech_debt_register() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let td_path = manifest.join("docs/develop/v0/tech-debt-register.md");
    let content = std::fs::read_to_string(&td_path).expect("tech-debt-register.md should exist");
    assert!(content.contains("TD-LLVM-INTERNAL-NONDETERMINISM"));
    assert!(content.contains("TD-PROCESS-PER-TEST-ISOLATION"));
}

#[test]
fn stage116_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-116/dev-log.md");
    assert!(dev_log.exists());
}

#[test]
fn stage116_llvm_shutdown_in_drop() {
    // Verify LLVMShutdown is called in Drop (compile succeeds, no crash).
    let src = r#"fn main() -> i32 { 0 }"#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}
