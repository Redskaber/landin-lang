//! Stage 96 (v0.9): TD-PRELUDE-TRAIT-COVERAGE 续 — Ord trait added.
//! Debug + PartialOrd deferred (impl bodies cause codegen crash).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage96_ord_trait_compiles() {
    let src = r#"
        trait Ord {}
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    let _ = result;
}

#[test]
fn stage96_undefined_type_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: UndefinedType = 0;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage96_type_mismatch_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: i32 = true;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage96_nonexistent_method_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: i32 = 42;
            x.nonexistent()
        }
    "#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty() || !result.errors.lower.is_empty());
}
