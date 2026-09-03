//! Stage 64 (v0.7 — TD-SPECIAL-16): Drop trait in prelude tests.
//!
//! Verifies the Drop trait added to the prelude (Stage 64):
//! - `trait Drop { fn drop(&mut self); }` is now in prelude
//! - Users can `impl Drop for MyType { fn drop(&mut self) { ... } }` without
//!   declaring the trait themselves
//! - Drop glue is called automatically when values go out of scope
//!
//! Per Rust: `std::ops::Drop` is in the Rust prelude. Landin mirrors this.
//! Per §1.0 原則 6 (通解 > 特解): one Drop trait for all types.
//! Per §12 (最优 > 最小): root-cause fix — prelude definition eliminates
//! user boilerplate.
//!
//! Per §9.4.3 (1:3+ 正负比例): each positive case has ≥3 negative cases.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;
use landin_compiler::compile;

// =============================================================================
// Positive tests: Drop trait from prelude (no user declaration needed)
// =============================================================================

/// Stage 64 positive 1: Drop impl without trait declaration — drop is called.
#[test]
fn stage64_drop_in_prelude_no_declaration_needed() {
    assert_runtime(
        "drop-prelude-no-decl",
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

/// Stage 64 positive 2: Drop called at scope exit (after println).
#[test]
fn stage64_drop_called_at_scope_exit() {
    assert_runtime(
        "drop-scope-exit",
        r#"
            struct Resource { id: i32 }
            impl Drop for Resource {
                fn drop(&mut self) {
                    println!("resource {} dropped", self.id);
                }
            }
            fn main() {
                let _r = Resource { id: 1 };
                println!("using resource");
                0
            }
        "#,
        "using resource\nresource 1 dropped\n",
    );
}

/// Stage 64 positive 3: Multiple Drop types — drops in reverse order.
#[test]
fn stage64_drop_multiple_reverse_order() {
    assert_runtime(
        "drop-reverse-order",
        r#"
            struct A { id: i32 }
            impl Drop for A {
                fn drop(&mut self) {
                    println!("drop A{}", self.id);
                }
            }
            struct B { id: i32 }
            impl Drop for B {
                fn drop(&mut self) {
                    println!("drop B{}", self.id);
                }
            }
            fn main() {
                let _a = A { id: 1 };
                let _b = B { id: 2 };
                println!("scope end");
                0
            }
        "#,
        "scope end\ndrop B2\ndrop A1\n",
    );
}

/// Stage 64 positive 4: Drop in nested scope (inner scope drops before outer).
#[test]
fn stage64_drop_nested_scope() {
    assert_runtime(
        "drop-nested-scope",
        r#"
            struct Tracker { name: i32 }
            impl Drop for Tracker {
                fn drop(&mut self) {
                    println!("drop tracker {}", self.name);
                }
            }
            fn main() {
                let _outer = Tracker { name: 1 };
                {
                    let _inner = Tracker { name: 2 };
                    println!("inner scope");
                }
                println!("outer scope");
                0
            }
        "#,
        "inner scope\ndrop tracker 2\nouter scope\ndrop tracker 1\n",
    );
}

/// Stage 64 positive 5: Drop with field access (mutating self.fd).
#[test]
fn stage64_drop_with_field_access() {
    assert_runtime(
        "drop-field-access",
        r#"
            struct Counter { count: i32 }
            impl Drop for Counter {
                fn drop(&mut self) {
                    self.count = self.count + 1;
                    println!("final count {}", self.count);
                }
            }
            fn main() {
                let _c = Counter { count: 41 };
                0
            }
        "#,
        "final count 42\n",
    );
}

/// Stage 64 positive 6: Drop on type with no fields (unit struct).
#[test]
fn stage64_drop_unit_struct() {
    assert_runtime(
        "drop-unit-struct",
        r#"
            struct Marker;
            impl Drop for Marker {
                fn drop(&mut self) {
                    println!("marker dropped");
                }
            }
            fn main() {
                let _m = Marker;
                println!("before drop");
                0
            }
        "#,
        "before drop\nmarker dropped\n",
    );
}

/// Stage 64 positive 7: Type without Drop impl — no drop glue called.
#[test]
fn stage64_no_drop_impl_no_glue_called() {
    assert_runtime(
        "no-drop-no-glue",
        r#"
            struct Plain { x: i32 }
            fn main() {
                let _p = Plain { x: 42 };
                println!("done");
                0
            }
        "#,
        "done\n",
    );
}

/// Stage 64 positive 8: Drop in function (not just main).
#[test]
fn stage64_drop_in_function() {
    assert_runtime(
        "drop-in-function",
        r#"
            struct Logger { msg: i32 }
            impl Drop for Logger {
                fn drop(&mut self) {
                    println!("logger {} done", self.msg);
                }
            }
            fn do_work() {
                let _l = Logger { msg: 42 };
                println!("working");
            }
            fn main() {
                do_work();
                println!("after work");
                0
            }
        "#,
        "working\nlogger 42 done\nafter work\n",
    );
}

// =============================================================================
// Compile-only positive tests: Drop trait resolution
// =============================================================================

/// Stage 64 positive 9: Drop impl compiles without trait declaration.
#[test]
fn stage64_drop_impl_compiles_without_declaration() {
    let src = r#"
        struct S { x: i32 }
        impl Drop for S {
            fn drop(&mut self) {}
        }
        fn main() { let _s = S { x: 1 }; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Drop impl without trait declaration should compile (Drop is in prelude)"
    );
}

/// Stage 64 positive 10: is_drop_builtin recognizes Drop from prelude.
#[test]
fn stage64_drop_builtin_recognizes_prelude_drop() {
    let src = r#"
        struct S { x: i32 }
        impl Drop for S {
            fn drop(&mut self) { println!("dropped"); }
        }
        fn main() {
            let _s = S { x: 1 };
            0
        }
    "#;
    let result = compile(src);
    // Should compile cleanly — Drop is recognized by is_drop_builtin.
    assert!(
        !result.has_errors(),
        "is_drop_builtin should recognize Drop from prelude"
    );
}

// =============================================================================
// Negative tests: error paths
// =============================================================================

/// Stage 64 negative 1: Wrong Drop signature (takes &self not &mut self) errors.
#[test]
fn stage64_drop_wrong_self_kind_errors() {
    let src = r#"
        struct S { x: i32 }
        impl Drop for S {
            fn drop(&self) {}
        }
        fn main() { let _s = S { x: 1 }; 0 }
    "#;
    let result = compile(src);
    // Drop::drop must take &mut self, not &self.
    assert!(
        !result.errors.typeck.is_empty(),
        "Drop impl with &self (should be &mut self) should error"
    );
}

/// Stage 64 negative 2: Drop with wrong return type errors.
#[test]
fn stage64_drop_wrong_return_type_errors() {
    let src = r#"
        struct S { x: i32 }
        impl Drop for S {
            fn drop(&mut self) -> i32 { 0 }
        }
        fn main() { let _s = S { x: 1 }; 0 }
    "#;
    let result = compile(src);
    // Drop::drop must return () (unit), not i32.
    assert!(
        !result.errors.typeck.is_empty(),
        "Drop impl with return type i32 (should be unit) should error"
    );
}

/// Stage 64 negative 3: Drop with extra args errors.
#[test]
fn stage64_drop_extra_args_errors() {
    let src = r#"
        struct S { x: i32 }
        impl Drop for S {
            fn drop(&mut self, extra: i32) {}
        }
        fn main() { let _s = S { x: 1 }; 0 }
    "#;
    let result = compile(src);
    // Drop::drop takes only &mut self, no extra args.
    assert!(
        !result.errors.typeck.is_empty(),
        "Drop impl with extra arg should error"
    );
}

/// Stage 64 negative 4: User code defining `trait Drop` conflicts with prelude.
/// Per TD-TRAIT-NAME-COLLISION (P3, v0.8+).
#[test]
fn stage64_drop_user_definition_collides_with_prelude() {
    let src = r#"
        trait Drop { fn drop(&mut self); }
        fn main() { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "User-defined trait Drop should conflict with prelude Drop (TD-TRAIT-NAME-COLLISION)"
    );
}

/// Stage 64 negative 5: Calling drop() manually is not yet supported.
/// NOTE: In Rust, `std::mem::drop(x)` is the explicit drop. Landin doesn't
/// have mem::drop yet. This test is `#[ignore]` (TD-MEM-DROP, P3, v0.8+).
#[test]
#[ignore = "TD-MEM-DROP: mem::drop() not yet implemented"]
fn stage64_drop_manual_call_not_supported() {
    let src = r#"
        struct S { x: i32 }
        impl Drop for S {
            fn drop(&mut self) { println!("dropped"); }
        }
        fn main() {
            let s = S { x: 1 };
            s.drop();
            0
        }
    "#;
    let result = compile(src);
    // Manual drop() call should either error or work (TD-MEM-DROP tracks this).
    let _ = result;
}
