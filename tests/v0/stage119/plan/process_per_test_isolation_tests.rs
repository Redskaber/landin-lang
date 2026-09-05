//! Stage 119 (v0.12): Process-per-test isolation implemented — compile_src
//! now uses subprocess (`landin-stage0 --check-errors`) for error-free
//! compilations, giving fresh LLVM C++ state per test. Debug impl re-add
//! attempted + REVERTED (0-2 residual non-determinism from run_program tests
//! that still use in-process `driver::compile_binary`).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage119_subprocess_compile_works() {
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
fn stage119_impl_trait_works() {
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
fn stage119_vec_new_push_works() {
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
fn stage119_check_errors_flag_works() {
    // Verify --check-errors flag outputs JSON.
    use std::process::Command;
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = manifest.join("target/release/landin-stage0");
    if !bin.exists() {
        return; // Skip if binary not built.
    }
    let tmp = std::env::temp_dir().join("stage119_check_errors_test.lin");
    std::fs::write(&tmp, "fn main() -> i32 { 0 }").unwrap();
    let output = Command::new(&bin)
        .arg("--check-errors")
        .arg(&tmp)
        .output()
        .expect("failed to execute");
    let _ = std::fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"has_errors\":false"), "got: {}", stdout);
}

#[test]
fn stage119_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-119/dev-log.md");
    assert!(dev_log.exists());
}
