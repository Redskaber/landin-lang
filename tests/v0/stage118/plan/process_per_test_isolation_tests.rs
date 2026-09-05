//! Stage 118 (v0.12): Process-per-test isolation infrastructure investigation.
//! Stage 117 baseline verified stable after clean rebuild. --check-errors flag
//! tested. Debug impl re-add deferred to Stage 119.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage118_baseline_stable() {
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
fn stage118_impl_trait_works() {
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
fn stage118_vec_new_push_works() {
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
fn stage118_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-118/dev-log.md");
    assert!(dev_log.exists());
}
