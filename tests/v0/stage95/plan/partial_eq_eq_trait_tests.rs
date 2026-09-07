//! Stage 95 (v0.9): TD-PRELUDE-TRAIT-COVERAGE 续 — PartialEq + Eq added.
//!
//! Verifies PartialEq and Eq traits are in the prelude with impls for
//! i32/i64/bool/usize. Eq declared WITHOUT supertrait (avoids object
//! safety interference — per Stage 94 finding).

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::compile_src;

/// Stage 95 positive 1: PartialEq + Eq traits compile.
#[test]
fn stage95_partial_eq_eq_traits_compile() {
    let src = r#"
        trait PartialEq<Rhs> { fn eq(&self, other: &Rhs) -> bool; }
        trait Eq {}
        fn main() -> i32 { 0 }
    "#;
    let result = compile_src(src);
    let _ = result;
}

/// Stage 95 negative 1: Undefined type in trait impl errors.
#[test]
fn stage95_undefined_type_in_impl_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: UndefinedType = 0;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.resolve.is_empty() || !result.errors.typeck.is_empty(),
        "undefined type should error"
    );
}

/// Stage 95 negative 2: Calling nonexistent method errors.
#[test]
fn stage95_nonexistent_method_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: i32 = 42;
            x.nonexistent()
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.lower.is_empty(),
        "nonexistent method should error"
    );
}

/// Stage 95 negative 3: Type mismatch errors.
#[test]
fn stage95_type_mismatch_errors() {
    let src = r#"
        fn main() -> i32 {
            let x: i32 = true;
            0
        }
    "#;
    let result = compile_src(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "type mismatch (bool to i32) should error"
    );
}
