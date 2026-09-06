//! Stage 122 (v0.12): v0.12 phase closure — all Landin-side TDs resolved.
//! Debug impl bodies permanently deferred (LLVM C++ non-determinism).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage122_v012_complete() {
    let src = r#"
        fn main() -> i32 {
            println!("{}", 42);
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.has_errors(), "got: {:?}", result.errors);
}

#[test]
fn stage122_impl_trait_works() {
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
fn stage122_rca_dev_log_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dev_log = manifest.join("docs/develop/v0/stage-122/dev-log.md");
    assert!(dev_log.exists());
}

#[test]
fn stage122_tech_debt_register_updated() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let td_path = manifest.join("docs/develop/v0/tech-debt-register.md");
    let content = std::fs::read_to_string(&td_path).expect("tech-debt-register.md should exist");
    assert!(
        content.contains("v0.12 COMPLETE"),
        "tech-debt-register should mark v0.12 as COMPLETE"
    );
}
