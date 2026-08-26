//! Stage 18.267 — Continued holistic soundness audit per §17.6.
//!
//! Per user instruction "直到审查不出问题为止" (keep auditing until no
//! problems are found). Stage 18.264 found struct literal + Box::new
//! gaps. This stage audits additional expression contexts that may have
//! similar expected-ty propagation gaps.
//!
//! Per §17.6 缺陷纳入: same-class errors should be considered holistically.
//! Per §1.0 原則 9 (正确 > 妥协): full soundness, not just MVP.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Assignment target — `s.f = Holder(true)` (struct field update)
// ============================================================================

#[test]
fn test_audit_struct_field_assignment_with_wrong_ctor() {
    // Assigning to a struct field where the assigned value is a generic
    // tuple struct ctor with wrong type. The expected type comes from the
    // field's declared type.
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let mut o = Outer { f: Holder(42) };
            o.f = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[struct field assignment] o.f = Holder(true) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: struct field assignment may not propagate expected_ty");
    }
}

// ============================================================================
// Tuple index assignment — `t.0 = Holder(true)`
// ============================================================================

#[test]
fn test_audit_tuple_index_assignment_with_wrong_ctor() {
    // Assigning to a tuple element where the assigned value is a generic
    // tuple struct ctor with wrong type.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let mut t: (Holder<i32>,) = (Holder(42),);
            t.0 = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[tuple index assignment] t.0 = Holder(true) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: tuple index assignment may not propagate expected_ty");
    }
}

// ============================================================================
// Local variable reassignment — `let mut x: Holder<i32> = ...; x = Holder(true)`
// ============================================================================

#[test]
fn test_audit_local_reassignment_with_wrong_ctor() {
    // Reassigning a local variable where the new value is a generic tuple
    // struct ctor with wrong type.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let mut x: Holder<i32> = Holder(42);
            x = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[local reassignment] x = Holder(true) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: local reassignment may not propagate expected_ty");
    }
}

// ============================================================================
// Array index assignment — `arr[0] = Holder(true)`
// ============================================================================

#[test]
fn test_audit_array_index_assignment_with_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let mut arr: [Holder<i32>; 2] = [Holder(42), Holder(99)];
            arr[0] = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[array index assignment] arr[0] = Holder(true) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: array index assignment may not propagate expected_ty");
    }
}

// ============================================================================
// Function call where struct literal arg has wrong field —
// `take_outer(Outer { f: Holder(true) })`
// ============================================================================

#[test]
fn test_audit_fn_call_with_struct_literal_arg_wrong_field() {
    // Combined case: fn call arg is a struct literal where one field has
    // a generic tuple struct ctor with wrong type.
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn take_outer(o: Outer) -> i32 { 0 }
        fn main() -> i32 {
            take_outer(Outer { f: Holder(true) })
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[fn call with struct literal arg] take_outer(Outer {{ f: Holder(true) }}) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: fn call with struct literal arg may have compound gap");
    }
}

// ============================================================================
// Closure body returning wrong ctor — `let c = || Holder(true)`
// ============================================================================

#[test]
fn test_audit_closure_return_with_wrong_ctor() {
    // Closure body returns a generic tuple struct ctor with wrong type
    // when the closure's expected return type is Holder<i32>.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let c: fn() -> Holder<i32> = || Holder(true);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[closure return] let c: fn() -> Holder<i32> = || Holder(true) — has_errors = {} (expected: true)",
        result.has_errors()
    );
}

// ============================================================================
// Match arm returning wrong ctor — `match x { 1 => Holder(true), _ => Holder(42) }`
// ============================================================================

#[test]
fn test_audit_match_arm_return_with_wrong_ctor() {
    // Match arm returns a generic tuple struct ctor with wrong type when
    // the match expression's expected type is Holder<i32>.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x = 1;
            let h: Holder<i32> = match x {
                1 => Holder(true),
                _ => Holder(42),
            };
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[match arm return] match x {{ 1 => Holder(true), ... }} — has_errors = {} (expected: true)",
        result.has_errors()
    );
}

// ============================================================================
// If expression returning wrong ctor — `if cond { Holder(true) } else { Holder(42) }`
// ============================================================================

#[test]
fn test_audit_if_expr_return_with_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let cond = true;
            let h: Holder<i32> = if cond { Holder(true) } else { Holder(42) };
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[if expr return] if cond {{ Holder(true) }} else {{ Holder(42) }} — has_errors = {} (expected: true)",
        result.has_errors()
    );
}

// ============================================================================
// Multi-field struct literal with multiple wrong ctors
// ============================================================================

#[test]
fn test_audit_multi_field_struct_literal_with_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        struct Pair { a: Holder<i32>, b: Holder<bool> }
        fn main() -> i32 {
            let p = Pair { a: Holder(true), b: Holder(42) };
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[multi-field struct] Pair {{ a: Holder(true), b: Holder(42) }} — has_errors = {} (expected: true)",
        result.has_errors()
    );
}
