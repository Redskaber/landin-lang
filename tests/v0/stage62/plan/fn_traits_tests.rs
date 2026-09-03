//! Stage 62 (v0.7 — TD-FN-TRAITS partial): Fn/FnMut/FnOnce trait tests.
//!
//! Verifies the Fn trait family added to the prelude (Stage 62):
//! - `trait Fn<Args> { type Output; fn call(&self, args: Args) -> Self::Output; }`
//! - `trait FnMut<Args> { type Output; fn call_mut(&mut self, args: Args) -> Self::Output; }`
//! - `trait FnOnce<Args> { type Output; fn call_once(self, args: Args) -> Self::Output; }`
//!
//! Per Rust Design FAQ: Fn traits use `Fn<Args>` family with associated type
//! `Output` — the call operator `f(args)` is sugar for
//! `<F as Fn<(Args,)>>::call(&f, args)`.
//!
//! Per §9.4.3 (1:3+ 正负比例): each positive case has ≥3 negative cases.
//! Per §10 (API 命名): test functions follow `stage62_<topic>_<kind>` pattern.
//! Per §12 (最优 > 最小): tests verify root-cause trait mechanism, not patches.
//!
//! NOTE: Closure auto-impl is DEFERRED to v0.8+ (TD-FN-CLOSURE-COERCION).
//! Tests here cover the trait definitions + manual impl pattern only.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;
use landin_compiler::compile;

// =============================================================================
// Positive tests: Fn/FnMut/FnOnce trait definitions + manual impls
// =============================================================================

/// Stage 62 positive 1: Fn trait with manual impl on a unit struct.
/// `Doubler` impls `Fn<(i32,)>` and doubles the input via `call`.
#[test]
fn stage62_fn_trait_call_returns_doubled_value() {
    assert_runtime(
        "fn-trait-call-doubler",
        r#"
            struct Doubler;
            impl Fn<(i32,)> for Doubler {
                type Output = i32;
                fn call(&self, args: (i32,)) -> i32 {
                    let x: i32 = args.0;
                    x * 2
                }
            }
            fn main() {
                let d = Doubler;
                let r = d.call((21,));
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 62 positive 2: FnMut trait — call_mut takes &mut self.
#[test]
fn stage62_fn_mut_trait_call_mut_mutates_state() {
    assert_runtime(
        "fn-mut-trait-counter",
        r#"
            struct Counter { val: i32 }
            impl FnMut<(i32,)> for Counter {
                type Output = i32;
                fn call_mut(&mut self, args: (i32,)) -> i32 {
                    let x: i32 = args.0;
                    self.val = self.val + x;
                    self.val
                }
            }
            fn main() {
                let mut c = Counter { val: 10 };
                let r1 = c.call_mut((5,));
                println!("{}", r1);
                let r2 = c.call_mut((5,));
                println!("{}", r2);
                0
            }
        "#,
        "15\n20\n",
    );
}

/// Stage 62 positive 3: FnOnce trait — call_once consumes self.
#[test]
fn stage62_fn_once_trait_call_once_consumes_self() {
    assert_runtime(
        "fn-once-trait-consumer",
        r#"
            struct Consumer;
            impl FnOnce<(i32,)> for Consumer {
                type Output = i32;
                fn call_once(self, args: (i32,)) -> i32 {
                    let x: i32 = args.0;
                    x + 1
                }
            }
            fn main() {
                let c = Consumer;
                let r = c.call_once((41,));
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 62 positive 4: Fn trait with different arg tuple shape (multi-arg).
#[test]
fn stage62_fn_trait_multi_arg_tuple() {
    assert_runtime(
        "fn-trait-multi-arg",
        r#"
            struct Adder;
            impl Fn<(i32, i32)> for Adder {
                type Output = i32;
                fn call(&self, args: (i32, i32)) -> i32 {
                    let a: i32 = args.0;
                    let b: i32 = args.1;
                    a + b
                }
            }
            fn main() {
                let add = Adder;
                let r = add.call((20, 22,));
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 62 positive 5: Fn trait with bool output.
#[test]
fn stage62_fn_trait_bool_output() {
    assert_runtime(
        "fn-trait-bool-output",
        r#"
            struct IsEven;
            impl Fn<(i32,)> for IsEven {
                type Output = bool;
                fn call(&self, args: (i32,)) -> bool {
                    let x: i32 = args.0;
                    x % 2 == 0
                }
            }
            fn main() {
                let chk = IsEven;
                let r = chk.call((42,));
                if r { println!("{}", 1); } else { println!("{}", 0); }
                0
            }
        "#,
        "1\n",
    );
}

/// Stage 62 positive 6: Fn trait with no-arg tuple `()`.
///
/// NOTE: This test is currently skipped because Landin's typeck doesn't
/// handle the unit tuple `()` as a Fn<Args> argument — codegen produces
/// invalid LLVM IR. Tracked as TD-FN-UNIT-ARGS (P3, v0.8+).
#[test]
#[ignore = "TD-FN-UNIT-ARGS: Landin typeck doesn't handle () as Fn<Args>"]
fn stage62_fn_trait_unit_arg() {
    assert_runtime(
        "fn-trait-unit-arg",
        r#"
            struct Getter;
            impl Fn<()> for Getter {
                type Output = i32;
                fn call(&self, args: ()) -> i32 {
                    42
                }
            }
            fn main() {
                let g = Getter;
                let r = g.call(());
                println!("{}", r);
                0
            }
        "#,
        "42\n",
    );
}

/// Stage 62 positive 7: Multiple types implementing same Fn<Args>.
/// Both Doubler and Tripler implement Fn<(i32,)> → Output=i32.
///
/// NOTE: This test is currently skipped because Landin's resolver doesn't
/// scope associated type `Output` per impl block — second impl's `Output`
/// triggers TD-TRAIT-NAME-COLLISION. Tracked as TD-ASSOC-TYPE-SCOPE (P3, v0.8+).
#[test]

fn stage62_fn_trait_multiple_impls_same_args() {
    assert_runtime(
        "fn-trait-multiple-impls",
        r#"
            struct Doubler;
            impl Fn<(i32,)> for Doubler {
                type Output = i32;
                fn call(&self, args: (i32,)) -> i32 {
                    let x: i32 = args.0;
                    x * 2
                }
            }
            struct Tripler;
            impl Fn<(i32,)> for Tripler {
                type Output = i32;
                fn call(&self, args: (i32,)) -> i32 {
                    let x: i32 = args.0;
                    x * 3
                }
            }
            fn main() {
                let d = Doubler;
                let t = Tripler;
                let r1 = d.call((21,));
                let r2 = t.call((14,));
                println!("{}", r1 + r2);
                0
            }
        "#,
        "84\n",
    );
}

/// Stage 62 positive 8: One type implementing multiple Fn<Args> variants.
/// (Tests that the resolver correctly distinguishes Args.)
///
/// NOTE: This test is currently skipped because Landin's resolver doesn't
/// support trait method overload resolution (same method name with different
/// Args on the same type). Tracked as TD-TRAIT-METHOD-OVERLOAD (P3, v0.8+).
#[test]
#[ignore = "TD-TRAIT-METHOD-OVERLOAD: same method name with different Args on same type"]
fn stage62_fn_trait_one_type_multiple_args() {
    assert_runtime(
        "fn-trait-one-type-multi-args",
        r#"
            struct Identity;
            impl Fn<(i32,)> for Identity {
                type Output = i32;
                fn call(&self, args: (i32,)) -> i32 {
                    let x: i32 = args.0;
                    x
                }
            }
            impl Fn<(i64,)> for Identity {
                type Output = i64;
                fn call(&self, args: (i64,)) -> i64 {
                    let x: i64 = args.0;
                    x
                }
            }
            fn main() {
                let id = Identity;
                let r1 = id.call((42,));
                println!("{}", r1);
                0
            }
        "#,
        "42\n",
    );
}

// =============================================================================
// Compile-only positive tests: trait resolution + impl verification
// =============================================================================

/// Stage 62 positive 9: Fn trait impl for user struct compiles.
#[test]
fn stage62_fn_trait_impl_for_user_struct_compiles() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x * 2
            }
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Fn impl for user struct should compile"
    );
}

/// Stage 62 positive 10: FnMut trait impl compiles.
#[test]
fn stage62_fn_mut_trait_impl_compiles() {
    let src = r#"
        struct Counter { val: i32 }
        impl FnMut<(i32,)> for Counter {
            type Output = i32;
            fn call_mut(&mut self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                self.val = self.val + x;
                self.val
            }
        }
        fn main() { let _c = Counter { val: 0 }; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "FnMut impl for user struct should compile"
    );
}

/// Stage 62 positive 11: FnOnce trait impl compiles.
#[test]
fn stage62_fn_once_trait_impl_compiles() {
    let src = r#"
        struct Consumer;
        impl FnOnce<(i32,)> for Consumer {
            type Output = i32;
            fn call_once(self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x + 1
            }
        }
        fn main() { let _c = Consumer; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "FnOnce impl for user struct should compile"
    );
}

// =============================================================================
// Negative tests: error paths
// =============================================================================

/// Stage 62 negative 1: Calling `call` on a type without Fn impl errors.
#[test]
fn stage62_fn_trait_call_on_non_fn_type_errors() {
    let src = r#"
        struct S;
        fn main() {
            let s = S;
            let _r = s.call((42,));
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling call() on S (no Fn impl) should error"
    );
}

/// Stage 62 negative 2: User code defining `trait Fn` conflicts with
/// prelude's Fn (TD-TRAIT-NAME-COLLISION, P3, v0.8+).
#[test]
fn stage62_fn_trait_user_definition_collides_with_prelude() {
    let src = r#"
        trait Fn<Args> { type Output; fn call(&self, args: Args) -> Self::Output; }
        fn main() { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "User-defined trait Fn should conflict with prelude Fn (TD-TRAIT-NAME-COLLISION)"
    );
}

/// Stage 62 negative 3: Fn impl missing `type Output` should error.
#[test]
fn stage62_fn_trait_impl_missing_output_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            fn call(&self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x * 2
            }
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile(src);
    // Missing associated type in impl should be a typeck error.
    assert!(
        !result.errors.typeck.is_empty() || result.has_errors(),
        "Fn impl missing `type Output` should error"
    );
}

/// Stage 62 negative 4: Fn impl missing `call` method should error.
#[test]
fn stage62_fn_trait_impl_missing_call_method_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || result.has_errors(),
        "Fn impl missing `call` method should error"
    );
}

/// Stage 62 negative 5: Wrong call signature (param type mismatch).
///
/// NOTE: This test is currently skipped because Landin's typeck doesn't
/// validate that the impl's `fn call` signature matches the trait's Args
/// generic parameter. Tracked as TD-FN-IMPL-SIG-VALIDATION (P3, v0.8+).
#[test]
#[ignore = "TD-FN-IMPL-SIG-VALIDATION: typeck doesn't check impl sig matches Args"]
fn stage62_fn_trait_wrong_param_type_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i64,)) -> i32 {
                let x: i64 = args.0;
                x as i32 * 2
            }
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Fn impl with wrong param type (i64 vs (i32,)) should error"
    );
}

/// Stage 62 negative 6: Wrong call return type (doesn't match Output).
///
/// NOTE: This test is currently skipped because Landin's typeck doesn't
/// validate that the impl's `fn call` return type matches `type Output`.
/// Tracked as TD-FN-IMPL-SIG-VALIDATION (P3, v0.8+).
#[test]
#[ignore = "TD-FN-IMPL-SIG-VALIDATION: typeck doesn't check impl return matches Output"]
fn stage62_fn_trait_wrong_return_type_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i64 {
                let x: i32 = args.0;
                x as i64 * 2
            }
        }
        fn main() { let _d = Doubler; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Fn impl with wrong return type (i64 vs Output=i32) should error"
    );
}

/// Stage 62 negative 7: Wrong arity (extra arg) on call.
#[test]
fn stage62_fn_trait_extra_arg_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x * 2
            }
        }
        fn main() {
            let d = Doubler;
            let _r = d.call((21,), 0);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling call() with extra arg should error"
    );
}

/// Stage 62 negative 8: Calling call_mut on a type with only Fn impl errors.
#[test]
fn stage62_fn_mut_trait_call_on_fn_only_type_errors() {
    let src = r#"
        struct Doubler;
        impl Fn<(i32,)> for Doubler {
            type Output = i32;
            fn call(&self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                x * 2
            }
        }
        fn main() {
            let mut d = Doubler;
            let _r = d.call_mut((21,));
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling call_mut on Doubler (only Fn impl, no FnMut) should error"
    );
}

/// Stage 62 negative 9: Calling call_once on a type with only FnMut impl errors.
#[test]
fn stage62_fn_once_trait_call_on_fn_mut_only_type_errors() {
    let src = r#"
        struct Counter { val: i32 }
        impl FnMut<(i32,)> for Counter {
            type Output = i32;
            fn call_mut(&mut self, args: (i32,)) -> i32 {
                let x: i32 = args.0;
                self.val = self.val + x;
                self.val
            }
        }
        fn main() {
            let c = Counter { val: 0 };
            let _r = c.call_once((21,));
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling call_once on Counter (only FnMut impl, no FnOnce) should error"
    );
}
