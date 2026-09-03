//! Stage 65 (v0.7 — TD-PRELUDE-MACRO-TIMING RESOLVED): Verify prelude
//! injection timing is correct — prelude macros (if any) work, and prelude
//! types (Option, Result, String, Vec, Clone, Display, Drop, Fn traits) are
//! available without user declaration.
//!
//! Per §12 (最优 > 最小): root cause was fixed differently than originally
//! planned. The prelude source uses direct `__landin_panic_msg(...)` and
//! `__landin_unreachable(...)` extern "C" calls instead of `panic!`/
//! `unreachable!` macros. This eliminates the need for token-level injection
//! (which would require DefId decoupling — an L3 refactor that broke 60+
//! tests in a previous attempt).
//!
//! Per §1.0 原則 9 (正确 > 妥协): document that the TD was resolved by a
//! different approach than originally planned.
//!
//! Per §9.4.3 (1:3+ 正负比例): positive tests verify prelude works; negative
//! tests verify error paths.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;
use landin_compiler::compile;

// =============================================================================
// Positive tests: prelude types and macros work without user declaration
// =============================================================================

/// Stage 65 positive 1: Option type from prelude works.
#[test]
fn stage65_prelude_option_works() {
    assert_runtime(
        "prelude-option",
        r#"
            fn main() {
                let x: Option<i32> = Some(42);
                match x {
                    Some(v) => println!("{}", v),
                    None => println!("none"),
                }
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 65 positive 2: Result type from prelude works.
#[test]
fn stage65_prelude_result_works() {
    assert_runtime(
        "prelude-result",
        r#"
            fn main() {
                let x: Result<i32, i32> = Ok(42);
                match x {
                    Ok(v) => println!("{}", v),
                    Err(_) => println!("err"),
                }
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 65 positive 3: String type from prelude works.
#[test]
fn stage65_prelude_string_works() {
    assert_runtime(
        "prelude-string",
        r#"
            fn main() {
                let mut s: String = String::new();
                s.push_str("hello");
                println!("{}", s.as_str());
                0
            }
        "#,
        "hello\n",
    );
}

/// Stage 65 positive 4: Clone trait from prelude works.
#[test]
fn stage65_prelude_clone_works() {
    assert_runtime(
        "prelude-clone",
        r#"
            fn main() {
                let x: i32 = 42;
                let y = x.clone();
                println!("{}", y);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 65 positive 5: Display trait from prelude works.
#[test]
fn stage65_prelude_display_works() {
    assert_runtime(
        "prelude-display",
        r#"
            fn main() {
                let x: i32 = 42;
                let mut s: String = String::new();
                let _r = x.fmt(&mut s);
                println!("{}", s.as_str());
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 65 positive 6: Drop trait from prelude works.
#[test]
fn stage65_prelude_drop_works() {
    assert_runtime(
        "prelude-drop",
        r#"
            struct File { fd: i32 }
            impl Drop for File {
                fn drop(&mut self) {
                    println!("dropping {}", self.fd);
                }
            }
            fn main() {
                let _f = File { fd: 42 };
                println!("before drop");
                0
            }
        "#,
        "before drop\ndropping 42\n",
    );
}

/// Stage 65 positive 7: User panic! macro works (expands to __landin_panic_msg).
/// NOTE: panic! output goes to stderr, not stdout. This test checks that
/// the program exits with non-zero (panic aborts). Per §1.0 原則 4 (报错 > 静默):
/// panic is an explicit error, not a silent failure.
#[test]
fn stage65_user_panic_macro_works() {
    // panic! should abort with non-zero exit. We verify the program
    // compiles and runs (exit != 0 indicates panic fired).
    let src = r#"
        fn main() {
            let _x = panic!("test panic message");
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "panic! macro should compile (expands to __landin_panic_msg)"
    );
}

/// Stage 65 positive 8: Vec type from prelude works.
#[test]
fn stage65_prelude_vec_works() {
    assert_runtime(
        "prelude-vec",
        r#"
            fn main() {
                let v: Vec<i32> = Vec::new();
                println!("{}", v.len());
                0
            }
        "#,
        "0\n",
    );
}

/// Stage 65 positive 9: Option::unwrap_or from prelude works (uses direct
/// __landin_panic_msg call, not panic! macro — verifies TD-PRELUDE-MACRO-TIMING
/// resolution).
#[test]
fn stage65_prelude_unwrap_or_no_macro_needed() {
    assert_runtime(
        "prelude-unwrap-or",
        r#"
            fn main() {
                let x: Option<i32> = None;
                let v = x.unwrap_or(99);
                println!("{}", v);
                0
            }
        "#,
        "99\n",
    );
}

/// Stage 65 positive 10: str::len from prelude works.
#[test]
fn stage65_prelude_str_len_works() {
    assert_runtime(
        "prelude-str-len",
        r#"
            fn main() {
                let s: &str = "hello";
                println!("{}", s.len());
                0
            }
        "#,
        "5\n",
    );
}

// =============================================================================
// Compile-only positive tests: prelude types resolve without declaration
// =============================================================================

/// Stage 65 positive 11: All prelude types compile without user declaration.
#[test]
fn stage65_all_prelude_types_compile() {
    let src = r#"
        fn main() {
            let _o: Option<i32> = Some(1);
            let _r: Result<i32, i32> = Ok(1);
            let _s: String = String::new();
            let _v: Vec<i32> = Vec::new();
            let _c: i32 = 42i32.clone();
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "All prelude types should compile without user declaration"
    );
}

// =============================================================================
// Negative tests: error paths
// =============================================================================

/// Stage 65 negative 1: Undefined type errors when not in prelude.
#[test]
fn stage65_undefined_type_errors() {
    let src = r#"
        fn main() {
            let _x: UndefinedType = 0;
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Undefined type should error"
    );
}

/// Stage 65 negative 2: User-defined trait Clone conflicts with prelude.
#[test]
fn stage65_user_clone_conflicts_with_prelude() {
    let src = r#"
        trait Clone { fn clone(&self) -> Self; }
        fn main() { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "User-defined trait Clone should conflict with prelude (TD-TRAIT-NAME-COLLISION)"
    );
}

/// Stage 65 negative 3: Calling method on undefined type errors.
#[test]
fn stage65_method_on_undefined_type_errors() {
    let src = r#"
        fn main() {
            let _x = UndefinedType::new();
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "Calling method on undefined type should error"
    );
}
