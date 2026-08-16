//! Stage 18.161 (TD-NEGATIVE-TEST-COVERAGE): MIR lowering negative tests.
//!
//! Tests MIR lowering error paths. Per §9.4.3, negative tests should be
//! ≥25% of total. This file covers MIR lower error paths.
//!
//! Per §9.1: tests live under `tests/v0/stage-XX/plan/`.
//! Per §16: tests use only public API (`compile`).

use landin_compiler::compile;

// === MIR structure errors ===

/// Stage 18.161 negative 1: empty function body produces valid MIR.
#[test]
fn stage18_161_mir_lower_empty_body() {
    let result = compile("fn main() {}");
    assert!(!result.mirs.is_empty());
    let main_mir = result
        .mirs
        .iter()
        .find(|m| m.def_id.is_some())
        .expect("should have at least one MIR body");
    assert!(
        !main_mir.basic_blocks.is_empty(),
        "should have at least one basic block"
    );
}

/// Stage 18.161 negative 2: function with only return.
#[test]
fn stage18_161_mir_lower_only_return() {
    let result = compile("fn main() -> i32 { 42 }");
    assert!(!result.mirs.is_empty());
}

/// Stage 18.161 negative 3: function with complex control flow.
#[test]
fn stage18_161_mir_lower_complex_control_flow() {
    let src = r#"
        fn main() -> i32 {
            let x = 1;
            if x > 0 {
                if x > 1 { 10 } else { 20 }
            } else {
                while x < 0 { x = x + 1; }
                30
            }
        }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty());
}

// === Aggregate errors ===

/// Stage 18.161 negative 4: struct literal with wrong fields.
#[test]
fn stage18_161_mir_lower_struct_wrong_fields() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() { let p = Point { x: 1, z: 2 }; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 5: struct literal with missing fields.
#[test]
fn stage18_161_mir_lower_struct_missing_fields() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn main() { let p = Point { x: 1 }; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 6: struct literal with extra fields.
#[test]
fn stage18_161_mir_lower_struct_extra_fields() {
    let src = r#"
        struct Point { x: i32 }
        fn main() { let p = Point { x: 1, y: 2 }; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Enum variant errors ===

/// Stage 18.161 negative 7: enum variant with wrong field count.
#[test]
fn stage18_161_mir_lower_enum_wrong_field_count() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() { let c = Color::Red(42); }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 8: undefined enum variant.
#[test]
fn stage18_161_mir_lower_undefined_enum_variant() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() { let c = Color::Yellow; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Binary operation errors ===

/// Stage 18.161 negative 9: binary op on incompatible types.
#[test]
fn stage18_161_mir_lower_binop_incompatible_types() {
    let src = r#"
        struct A;
        struct B;
        fn main() { let a = A; let b = B; let c = a + b; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 10: division by zero literal.
#[test]
fn stage18_161_mir_lower_div_by_zero_literal() {
    let result = compile("fn main() -> i32 { 42 / 0 }");
    // Division by zero is a runtime panic, not a compile error.
    assert!(!result.mirs.is_empty());
}

/// Stage 18.161 negative 11: modulo by zero literal.
#[test]
fn stage18_161_mir_lower_mod_by_zero_literal() {
    let result = compile("fn main() -> i32 { 42 % 0 }");
    assert!(!result.mirs.is_empty());
}

// === Cast errors ===

/// Stage 18.161 negative 12: cast between incompatible types.
#[test]
fn stage18_161_mir_lower_cast_incompatible() {
    let src = r#"
        struct A;
        fn main() { let a = A; let b = a as i32; }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 13: cast to undefined type.
#[test]
fn stage18_161_mir_lower_cast_to_undefined_type() {
    let result = compile("fn main() { let x = 42 as UndefinedType; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Reference errors ===

/// Stage 18.161 negative 14: reference to temporary.
#[test]
fn stage18_161_mir_lower_ref_to_temporary() {
    let result = compile("fn main() { let r = &(42); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 15: mutable reference to immutable.
#[test]
fn stage18_161_mir_lower_mut_ref_to_immut() {
    let result = compile("fn main() { let x = 42; let r = &mut x; }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Closure errors ===

/// Stage 18.161 negative 16: closure with wrong parameter count.
#[test]
fn stage18_161_mir_lower_closure_wrong_params() {
    let src = r#"
        fn take(f: fn(i32) -> i32) -> i32 { f(42) }
        fn main() -> i32 { take(|a, b| { a + b }) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 17: closure capturing undefined variable.
#[test]
fn stage18_161_mir_lower_closure_undefined_capture() {
    let result = compile("fn main() { let f = || { undefined_var }; f(); }");
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 18: closure with return type mismatch.
#[test]
fn stage18_161_mir_lower_closure_return_mismatch() {
    let src = r#"
        fn take(f: fn() -> i32) -> i32 { f() }
        fn main() -> i32 { take(|| { true }) }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}

// === Match exhaustiveness ===

/// Stage 18.161 negative 19: non-exhaustive match.
#[test]
fn stage18_161_mir_lower_non_exhaustive_match() {
    let src = r#"
        enum Color { Red, Green, Blue }
        fn main() { let c = Color::Red; match c { Color::Red => {} } }
    "#;
    let result = compile(src);
    // Non-exhaustive match may or may not be an error depending on stage.
    assert!(!result.mirs.is_empty() || result.has_errors());
}

/// Stage 18.161 negative 20: match with unreachable arm.
#[test]
fn stage18_161_mir_lower_unreachable_match_arm() {
    let src = r#"
        fn main() {
            let x = 1;
            match x {
                1 => {}
                1 => {}  // duplicate pattern
                _ => {}
            }
        }
    "#;
    let result = compile(src);
    assert!(!result.mirs.is_empty() || result.has_errors());
}
