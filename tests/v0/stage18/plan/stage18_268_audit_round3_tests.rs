//! Stage 18.268 — Continued holistic audit Round 3 per §17.6.
//!
//! Per user instruction "直到审查不出问题为止" — keep auditing until no
//! problems are found. Stage 18.267 found + closed enum variant ctor
//! gap. This stage audits:
//! - Match patterns with generic enum variants
//! - Struct pattern destructuring
//! - Generic function return where inner type is wrong
//! - Nested generic types (Box<Option<Holder<i32>>>)
//! - Generic struct fields (struct Generic<T> { f: T })
//!
//! Per §17.6 缺陷纳入: same-class errors should be considered holistically.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Match on generic enum — `match x { Some(v) => v, None => 0 }`
// ============================================================================

#[test]
fn test_audit_match_generic_enum_binding() {
    // Match on Option<Holder<i32>>, binding v as Holder<i32>. Then
    // use v as if it's Holder<bool>. Should error.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder(42));
            match x {
                Some(v) => {
                    let b: bool = v;  // Should error: Holder<i32> ≠ bool
                    0
                }
                None => 0
            }
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[match generic enum binding] has_errors = {} (expected: true)",
        result.has_errors()
    );
}

// ============================================================================
// Generic struct with generic field — `Generic { f: Holder(true) }`
// ============================================================================

#[test]
fn test_audit_generic_struct_field_with_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        struct Generic<T> { f: T }
        fn main() -> i32 {
            let g: Generic<Holder<i32>> = Generic { f: Holder(true) };
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[generic struct field ctor] Generic {{ f: Holder(true) }} — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: generic struct field may not propagate expected_ty");
    }
}

// ============================================================================
// Nested generic — `Box<Option<Holder<i32>>> = Box::new(Some(Holder(true)))`
// ============================================================================

#[test]
fn test_audit_nested_generic_box_option() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Option<Holder<i32>>> = Box::new(Some(Holder(true)));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[nested generic] Box<Option<Holder<i32>>> — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: nested generic may not propagate expected_ty");
    }
}

// ============================================================================
// Generic tuple struct multi-arg — `Pair(Holder(true), 42)`
// ============================================================================

#[test]
fn test_audit_generic_tuple_struct_multi_arg_first_wrong() {
    let src = r#"
        struct Holder<T>(T);
        struct Pair<A, B>(A, B);
        fn main() -> i32 {
            let p: Pair<Holder<i32>, i32> = Pair(Holder(true), 42);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[generic tuple multi-arg first wrong] Pair(Holder(true), 42) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!(
            "  → POTENTIAL GAP: multi-arg generic tuple struct may not propagate expected_ty"
        );
    }
}

#[test]
fn test_audit_generic_tuple_struct_multi_arg_second_wrong() {
    let src = r#"
        struct Holder<T>(T);
        struct Pair<A, B>(A, B);
        fn main() -> i32 {
            let p: Pair<Holder<i32>, Holder<bool>> = Pair(Holder(42), Holder(true));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[generic tuple multi-arg second wrong] Pair(Holder(42), Holder(true)) — has_errors = {} (expected: false — both valid)",
        result.has_errors()
    );
}

// ============================================================================
// Generic function return — `fn f() -> Holder<i32> { Holder(true) }`
// ============================================================================

#[test]
fn test_audit_generic_fn_return_with_wrong_inner_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder(true)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    eprintln!(
        "[generic fn return] fn make_holder() -> Holder<i32> {{ Holder(true) }} — has_errors = {} (expected: true)",
        result.has_errors()
    );
}

// ============================================================================
// Generic fn call with wrong turbofish on inner
// ============================================================================

#[test]
fn test_audit_generic_fn_call_with_wrong_inner_turbofish() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder<T>(x: T) -> Holder<T> { Holder(x) }
        fn main() -> i32 {
            let h: Holder<i32> = make_holder(true);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[generic fn call wrong arg] make_holder(true) for Holder<i32> — has_errors = {} (expected: true)",
        result.has_errors()
    );
}
