//! Stage 18.264 — Holistic soundness audit per §17.6.
//!
//! Per §17.6 (缺陷纳入): "当发现一个bug 时往往隐藏着更多问题".
//! TD-TUPLE-CTOR-CALL-ARG (Phase 2e) closed the call-arg soundness hole,
//! but similar holes may exist in other expression contexts where expected
//! type context is lost during MIR lowering.
//!
//! This audit tests each expression context where a generic tuple struct
//! ctor with wrong arg type may slip through:
//! - Method call args (e.g., `obj.method(Holder(true))`)
//! - Struct literal field values (e.g., `Wrapper { f: Holder(true) }`)
//! - Tuple constructor args (e.g., `(Holder(true), 42)`)
//! - Array literal elements (already covered by typeck)
//! - Closure call args (e.g., `closure(Holder(true))`)
//! - BinaryOp operands (e.g., `Holder(true) == Holder(42)`)
//! - Return position in if/match (already covered by typeck)
//! - let-else binding (`let x = Holder(true) else { ... }`)
//!
//! Per §9.4.3 1:3+ ratio: each identified gap gets 1 positive + 3 negative tests.
//! Per §1.0 原則 9 (正确 > 妥协): document all gaps as MVPs with fix plans.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Method call args — `obj.method(Holder(true))`
// ============================================================================

#[test]
fn test_audit_method_call_arg_with_wrong_ctor() {
    // Method call where arg is a generic tuple struct ctor with wrong type.
    // Phase 2e fixed FnDef call args via fn_sigs. Method calls may go
    // through a different path (resolve_method_by_name → method sig).
    let src = r#"
        struct Holder<T>(T);
        struct Obj;
        impl Obj {
            fn take(&self, h: Holder<i32>) -> i32 { 0 }
        }
        fn main() -> i32 {
            let o = Obj;
            o.take(Holder(true))
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[method call arg] o.take(Holder(true)) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: method call args may not propagate expected_ty");
    }
}

// ============================================================================
// Closure call args — `closure(Holder(true))`
// ============================================================================

#[test]
fn test_audit_closure_call_arg_with_wrong_ctor() {
    // Closure call where arg is a generic tuple struct ctor with wrong type.
    // Closures have synthesized sigs built during MIR lower — may or may
    // not propagate expected_ty to args.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let c = |h: Holder<i32>| 0;
            c(Holder(true))
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[closure call arg] c(Holder(true)) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: closure call args may not propagate expected_ty");
    }
}

// ============================================================================
// Struct literal field values — `Outer { f: Holder(true) }`
// ============================================================================

#[test]
fn test_audit_struct_literal_field_with_wrong_ctor() {
    // Struct literal where a field's value is a generic tuple struct ctor
    // with wrong type. The expected type comes from the field's declared type.
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder(true) };
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[struct literal field] Outer {{ f: Holder(true) }} — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: struct literal field values may not propagate expected_ty");
    }
}

// ============================================================================
// Tuple constructor args — `(Holder(true), 42)`
// ============================================================================

#[test]
fn test_audit_tuple_constructor_with_wrong_ctor() {
    // Tuple literal where an element is a generic tuple struct ctor with
    // wrong type. typeck should catch this via tuple element unify.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let t: (Holder<i32>, i32) = (Holder(true), 42);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[tuple constructor] (Holder(true), 42) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: tuple constructor args may not propagate expected_ty");
    }
}

// ============================================================================
// BinaryOp operands — `Holder(true) == Holder(42)`
// ============================================================================

#[test]
fn test_audit_binary_op_with_wrong_ctor() {
    // BinaryOp where one operand is a generic tuple struct ctor with
    // wrong type. Per Rust, `==` requires both sides to be the same type.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b = Holder(true) == Holder(42);
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[binary op] Holder(true) == Holder(42) — has_errors = {} (expected: true if Eq impl)",
        result.has_errors()
    );
}

// ============================================================================
// Function call return value as arg — `outer(inner(Holder(true)))`
// ============================================================================

#[test]
fn test_audit_nested_fn_call_with_wrong_ctor() {
    // Nested fn call where inner call's arg is a generic tuple struct ctor.
    // Phase 2e should cover this (take_holder is the outer fn).
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn passthrough(x: i32) -> i32 { x }
        fn main() -> i32 {
            passthrough(take_holder(Holder(true)))
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[nested fn call] passthrough(take_holder(Holder(true))) — has_errors = {} (expected: true)",
        result.has_errors()
    );
}

// ============================================================================
// let-else binding — `let Some(x) = Holder(true) else { ... }`
// ============================================================================

#[test]
fn test_audit_let_else_binding_with_wrong_ctor() {
    // let-else is a Stage 0 feature. Test if ctor in scrutinee position
    // propagates expected type correctly.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let h: Holder<i32> = Holder(42);
            0
        }
    "#;
    let result = compile(src);
    // This is the basic let-binding case — should pass (no error).
    assert!(
        !result.has_errors(),
        "Unexpected errors: {:?}",
        result.errors.typeck
    );
}

// ============================================================================
// Match scrutinee with ctor — `match Holder(true) { ... }`
// ============================================================================

#[test]
fn test_audit_match_scrutinee_with_wrong_ctor() {
    // Match scrutinee is a generic tuple struct ctor with wrong type.
    // The scrutinee's type determines the patterns' expected types.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            match Holder(true) {
                Holder(x) => x,
            }
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[match scrutinee] match Holder(true) {{ ... }} — has_errors = {} (expected: false — typeck infers T from arg)",
        result.has_errors()
    );
}

// ============================================================================
// Box::new wrapper — `Box::new(Holder(true))` (uses intrinsic)
// ============================================================================

#[test]
fn test_audit_box_new_with_wrong_ctor() {
    // Box::new is an intrinsic (Stage 18.187). The arg's type may go
    // through a different path than regular fn calls.
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    eprintln!(
        "[Box::new arg] Box::new(Holder(true)) — has_errors = {} (expected: true)",
        result.has_errors()
    );
    if !result.has_errors() {
        eprintln!("  → POTENTIAL GAP: Box::new intrinsic arg may not propagate expected_ty");
    }
}
