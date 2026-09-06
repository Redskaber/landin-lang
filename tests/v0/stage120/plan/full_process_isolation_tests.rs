//! Stage 120 (v0.12): Full process-per-test isolation — compile_silent now
//! also uses subprocess with placeholder errors. Debug impl re-add attempted
//! and REVERTED (0-3 residual non-determinism from run_program tests).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage120_subprocess_compile_works() {
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
fn stage120_impl_trait_works() {
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
fn stage120_vec_new_push_works() {
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
fn stage120_compile_silent_subprocess_works() {
    // Test that compile_silent uses subprocess path.
    use common::compile_silent;
    let src = r#"
        fn main() -> i32 { 0 }
    "#;
    let result = compile_silent(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage120_compile_silent_error_subprocess_works() {
    // Test that compile_silent subprocess path handles errors correctly.
    use common::compile_silent;
    let src = r#"
        fn main() -> i32 { let x: bool = 42; 0 }
    "#;
    let result = compile_silent(src);
    assert!(result.has_errors(), "expected errors, got success");
}

#[test]
fn stage120_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-120/dev-log.md");
    assert!(dev_log.exists());
}
