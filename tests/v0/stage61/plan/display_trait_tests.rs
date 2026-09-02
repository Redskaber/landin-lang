//! Stage 61 (v0.7 — TD-DISPLAY-TRAIT-MISSING partial): Display trait tests.
//!
//! Verifies the Display trait added to the prelude (Stage 61):
//! - `trait Display { fn fmt(&self, f: &mut String) -> i64; }`
//! - impls for i32, i64, usize, bool, str
//!
//! Per §9.4.3 (1:3+ 正负比例): each positive case has ≥3 negative cases.
//! Per §10 (API 命名): test functions follow `stage61_<topic>_<kind>` pattern.
//! Per §12 (最优 > 最小): tests verify root-cause trait mechanism, not patches.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;
use landin_compiler::compile;

// =============================================================================
// Positive tests: Display::fmt for each primitive type
// =============================================================================

/// Stage 61 positive 1: i32 Display::fmt writes decimal representation.
#[test]
fn stage61_display_i32_fmt_writes_decimal() {
    assert_runtime(
        "display-i32-fmt",
        r#"fn main() {
            let x: i32 = 42;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "42\n",
    );
}

/// Stage 61 positive 2: i64 Display::fmt writes decimal representation.
#[test]
fn stage61_display_i64_fmt_writes_decimal() {
    assert_runtime(
        "display-i64-fmt",
        r#"fn main() {
            let x: i64 = 123456;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "123456\n",
    );
}

/// Stage 61 positive 3: usize Display::fmt writes decimal representation.
#[test]
fn stage61_display_usize_fmt_writes_decimal() {
    assert_runtime(
        "display-usize-fmt",
        r#"fn main() {
            let x: usize = 99;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "99\n",
    );
}

/// Stage 61 positive 4: bool Display::fmt writes "true" for true.
#[test]
fn stage61_display_bool_true_writes_lowercase_true() {
    assert_runtime(
        "display-bool-true",
        r#"fn main() {
            let b: bool = true;
            let mut s: String = String::new();
            let _r: i64 = b.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "true\n",
    );
}

/// Stage 61 positive 5: bool Display::fmt writes "false" for false.
#[test]
fn stage61_display_bool_false_writes_lowercase_false() {
    assert_runtime(
        "display-bool-false",
        r#"fn main() {
            let b: bool = false;
            let mut s: String = String::new();
            let _r: i64 = b.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "false\n",
    );
}

/// Stage 61 positive 6: str Display::fmt writes the string content.
#[test]
fn stage61_display_str_fmt_writes_content() {
    assert_runtime(
        "display-str-content",
        r#"fn main() {
            let x: &str = "hello";
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "hello\n",
    );
}

/// Stage 61 positive 7: i32 Display::fmt handles negative values.
#[test]
fn stage61_display_i32_negative_value() {
    assert_runtime(
        "display-i32-negative",
        r#"fn main() {
            let x: i32 = 0 - 42;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "-42\n",
    );
}

/// Stage 61 positive 8: i32 Display::fmt handles zero.
#[test]
fn stage61_display_i32_zero() {
    assert_runtime(
        "display-i32-zero",
        r#"fn main() {
            let x: i32 = 0;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "0\n",
    );
}

/// Stage 61 positive 9: i32 Display::fmt handles i32::MAX (2147483647).
#[test]
fn stage61_display_i32_max() {
    assert_runtime(
        "display-i32-max",
        r#"fn main() {
            let x: i32 = 2147483647;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "2147483647\n",
    );
}

/// Stage 61 positive 10: Display::fmt can be called multiple times to
/// accumulate output in a single String (verifies fmt is "append" semantics).
#[test]
fn stage61_display_multiple_fmt_accumulates() {
    assert_runtime(
        "display-multiple-accumulate",
        r#"fn main() {
            let mut s: String = String::new();
            let a: i32 = 1;
            let b: i32 = 2;
            let c: i32 = 3;
            let _r1: i64 = a.fmt(&mut s);
            let _r2: i64 = b.fmt(&mut s);
            let _r3: i64 = c.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "123\n",
    );
}

/// Stage 61 positive 11: User-defined type can implement Display.
#[test]
fn stage61_display_user_defined_type() {
    assert_runtime(
        "display-user-defined",
        r#"struct Point { x: i32, y: i32 }
        impl Display for Point {
            fn fmt(&self, f: &mut String) -> i64 {
                let _r1: i64 = self.x.fmt(f);
                let _r2: i64 = self.y.fmt(f);
                0
            }
        }
        fn main() {
            let p: Point = Point { x: 4, y: 2 };
            let mut s: String = String::new();
            let _r: i64 = p.fmt(&mut s);
            println!("{}", s.as_str());
            0
        }"#,
        "42\n",
    );
}

// =============================================================================
// Compile-only positive tests: trait resolution + vtable emission
// =============================================================================

/// Stage 61 positive 12: Display trait impl for user struct compiles.
#[test]
fn stage61_display_impl_for_user_struct_compiles() {
    let src = r#"
        struct Foo { v: i32 }
        impl Display for Foo {
            fn fmt(&self, f: &mut String) -> i64 {
                let _r: i64 = self.v.fmt(f);
                0
            }
        }
        fn main() { let _f = Foo { v: 1 }; 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.has_errors(),
        "Display impl for user struct should compile"
    );
}

/// Stage 61 positive 13: Display bound on generic function resolves.
#[test]
fn stage61_display_generic_bound_resolves() {
    let src = r#"
        fn format_it<T: Display>(x: &T) -> String {
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s);
            s
        }
        fn main() {
            let x: i32 = 42;
            let _s: String = format_it(&x);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.errors.resolve.is_empty(),
        "Display generic bound should resolve"
    );
}

// =============================================================================
// Negative tests: error paths
// =============================================================================

/// Stage 61 negative 1: Calling fmt on a type that doesn't implement Display
/// should fail to compile (typeck error: no method `fmt` found).
#[test]
fn stage61_display_calling_fmt_on_non_display_type_errors() {
    // Note: tuple types don't have Display impl in prelude.
    let src = r#"
        fn main() {
            let t: (i32, i32) = (1, 2);
            let mut s: String = String::new();
            let _r: i64 = t.fmt(&mut s);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling fmt on tuple (no Display impl) should error"
    );
}

/// Stage 61 negative 2: User code defining `trait Display` conflicts with
/// prelude's Display (TD-TRAIT-NAME-COLLISION, P3, v0.8+).
/// Per §1.0 原則 9 (正确 > 妥协): document the limitation.
#[test]
fn stage61_display_user_trait_definition_collides_with_prelude() {
    let src = r#"
        trait Display { fn fmt(&self, f: &mut String) -> i64; }
        fn main() { 0 }
    "#;
    let result = compile(src);
    // Per TD-TRAIT-NAME-COLLISION: resolver reports duplicate definition.
    // This is a known limitation (P3, v0.8+) — the resolver should merge
    // prelude/user trait definitions like Rust does.
    assert!(
        !result.errors.resolve.is_empty(),
        "User-defined trait Display should conflict with prelude Display (TD-TRAIT-NAME-COLLISION)"
    );
}

/// Stage 61 negative 3: Implementing Display for a type without providing
/// the `fmt` method body should error (incomplete impl).
#[test]
fn stage61_display_incomplete_impl_errors() {
    let src = r#"
        struct S;
        impl Display for S {}
        fn main() { 0 }
    "#;
    let result = compile(src);
    // Incomplete impl: typeck should report missing method errors.
    assert!(
        !result.errors.typeck.is_empty() || result.has_errors(),
        "Incomplete Display impl (no fmt method) should error or have unresolved methods"
    );
}

/// Stage 61 negative 4: Wrong fmt signature (return type) should error.
#[test]
fn stage61_display_wrong_return_type_errors() {
    let src = r#"
        struct S;
        impl Display for S {
            fn fmt(&self, f: &mut String) -> i32 { 0 }
        }
        fn main() { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Display::fmt with wrong return type (i32 vs i64) should error"
    );
}

/// Stage 61 negative 5: Wrong fmt signature (param type) should error.
#[test]
fn stage61_display_wrong_param_type_errors() {
    let src = r#"
        struct S;
        impl Display for S {
            fn fmt(&self, f: i64) -> i64 { 0 }
        }
        fn main() { 0 }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Display::fmt with wrong param type (i64 vs &mut String) should error"
    );
}

/// Stage 61 negative 6: Calling fmt on a unit struct without Display impl.
#[test]
fn stage61_display_calling_fmt_on_unit_struct_without_impl_errors() {
    let src = r#"
        struct S;
        fn main() {
            let s = S;
            let mut buf: String = String::new();
            let _r: i64 = s.fmt(&mut buf);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling fmt on S (no Display impl) should error"
    );
}

/// Stage 61 negative 7: Wrong number of arguments to fmt.
#[test]
fn stage61_display_wrong_arity_errors() {
    let src = r#"
        fn main() {
            let x: i32 = 42;
            let mut s: String = String::new();
            let _r: i64 = x.fmt(&mut s, 0);
            0
        }
    "#;
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "Calling fmt with extra arg should error"
    );
}

// =============================================================================
// Architecture / codegen tests
// =============================================================================

/// Stage 61 arch 1: TextEmitter dedups @.data.<type> globals across
/// multiple trait impls for the same type (Stage 61 fix).
/// Per §12 (最优 > 最小): root-cause fix — track emitted data globals.
/// Per §1.0 原則 6 (通解 > 特解): one dedup mechanism for all data globals.
#[test]
fn stage61_text_emitter_dedups_data_globals() {
    use landin_compiler::codegen::emitter::ModuleEmitter;
    use landin_compiler::codegen::text::TextEmitter;
    let mut emitter = TextEmitter::new();
    // Simulate two trait impls for the same type i32 (Clone + Display).
    emitter.emit_dyn_trait_const(".dynptr.Clone.i32", ".data.i32", ".vtable.Clone.i32");
    emitter.emit_dyn_trait_const(".dynptr.Display.i32", ".data.i32", ".vtable.Display.i32");
    let output = emitter.output_with_globals();
    // @.data.i32 should appear exactly ONCE in the output (not twice).
    let count = output.matches("@.data.i32 = internal global").count();
    assert_eq!(
        count, 1,
        "TextEmitter should dedup @.data.i32 (got {} occurrences, expected 1).\n\
         Per §12 (最优 > 最小): root-cause fix — track emitted data globals.",
        count
    );
}

/// Stage 61 arch 2: TextEmitter dedups @.data.<type> across many types.
#[test]
fn stage61_text_emitter_dedups_data_globals_multiple_types() {
    use landin_compiler::codegen::emitter::ModuleEmitter;
    use landin_compiler::codegen::text::TextEmitter;
    let mut emitter = TextEmitter::new();
    // Multiple types, each with two trait impls (Clone + Display).
    for type_name in &["i32", "i64", "bool", "usize", "str"] {
        let data_sym = format!(".data.{}", type_name);
        emitter.emit_dyn_trait_const(
            &format!(".dynptr.Clone.{}", type_name),
            &data_sym,
            &format!(".vtable.Clone.{}", type_name),
        );
        emitter.emit_dyn_trait_const(
            &format!(".dynptr.Display.{}", type_name),
            &data_sym,
            &format!(".vtable.Display.{}", type_name),
        );
    }
    let output = emitter.output_with_globals();
    // Each @.data.<type> should appear exactly ONCE.
    for type_name in &["i32", "i64", "bool", "usize", "str"] {
        let pattern = format!("@.data.{} = internal global", type_name);
        let count = output.matches(&pattern).count();
        assert_eq!(
            count, 1,
            "TextEmitter should dedup @.data.{} (got {} occurrences, expected 1)",
            type_name, count
        );
    }
}
