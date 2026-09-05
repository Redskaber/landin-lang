//! Stage 117 (v0.12): TD-PROCESS-PER-TEST-ISOLATION RCA — process-per-test
//! confirmed as viable fix (non-determinism is cross-compilation accumulation,
//! NOT single-compilation).
//!
//! ## Key Findings
//!
//! 1. **Simple programs work 10/10 in subprocess** (fresh LLVM each time)
//! 2. **Tests that fail in full suite pass in isolation** (confirming
//!    cross-compilation accumulation, not single-compilation non-determinism)
//! 3. **ASLR off doesn't help** (6-8 failures still — non-determinism is from
//!    C++ heap allocator, not just ASLR)
//! 4. **Process-per-test isolation would work** — changing `compile_src` to
//!    use subprocess (like `run_program` already does) would give each test
//!    fresh LLVM state
//! 5. **Implementation requires structured error serialization** — `CompileResult`
//!    contains Vec<TypeError>, Vec<BorrowError>, etc. that need to be
//!    serialized/deserialized across process boundary

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage117_baseline_stable() {
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
fn stage117_impl_trait_works() {
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
fn stage117_vec_new_push_works() {
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
fn stage117_prelude_no_debug_impl_for_i32() {
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
fn stage117_rca_documented() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let td_path = manifest.join("docs/develop/v0/tech-debt-register.md");
    let content = std::fs::read_to_string(&td_path).expect("tech-debt-register.md should exist");
    assert!(content.contains("TD-PROCESS-PER-TEST-ISOLATION"));
    assert!(content.contains("TD-LLVM-INTERNAL-NONDETERMINISM"));
}

#[test]
fn stage117_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-117/dev-log.md");
    assert!(dev_log.exists());
}
