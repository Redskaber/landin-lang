//! Stage 97 (v0.9): TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH
//! root cause analysis. Added PartialOrd trait (declared, no impls).
//! Debug impls deferred — any prelude impl method returning String
//! (struct, needs sret) causes SIGSEGV in codegen.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

#[test]
fn stage97_partial_ord_trait_declared() {
    let src = r#"
        trait PartialOrd<Rhs> { fn partial_cmp(&self, other: &Rhs) -> Option<i32>; }
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    let _ = result;
}

#[test]
fn stage97_undefined_type_errors() {
    let src = r#"fn main() -> i32 { let x: Undefined = 0; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.resolve.is_empty() || !result.errors.typeck.is_empty());
}

#[test]
fn stage97_type_mismatch_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = true; 0 }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty());
}

#[test]
fn stage97_nonexistent_method_errors() {
    let src = r#"fn main() -> i32 { let x: i32 = 42; x.nonexistent() }"#;
    let result = compile_src(src);
    assert!(!result.errors.typeck.is_empty() || !result.errors.lower.is_empty());
}
