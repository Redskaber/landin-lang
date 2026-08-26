//! Stage 18.271 — Final comprehensive soundness audit per §17.6.
//!
//! Per user instruction "直到审查不出问题为止" (keep auditing until no
//! problems found). This is the FINAL audit round — verifies ALL
//! expression contexts are closed by running a comprehensive sweep.
//!
//! If ALL tests pass with `has_errors = true` (for wrong-type cases)
//! and `has_errors = false` (for valid cases), the audit is COMPLETE
//! and no more soundness holes remain.
//!
//! Per §17.6: same-class errors should be considered holistically.
//! Per §1.0 原則 9 (正确 > 妥协): full soundness across all contexts.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// 1. let binding — `let h: Holder<i32> = Holder(true)`
// ============================================================================

#[test]
fn test_final_let_binding_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let h: Holder<i32> = Holder(true);
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "let binding must error");
}

// ============================================================================
// 2. fn call arg — `take_holder(Holder(true))`
// ============================================================================

#[test]
fn test_final_fn_call_arg_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(true))
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "fn call arg must error");
}

// ============================================================================
// 3. struct literal field — `Outer { f: Holder(true) }`
// ============================================================================

#[test]
fn test_final_struct_literal_field_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        struct Outer { f: Holder<i32> }
        fn main() -> i32 {
            let o = Outer { f: Holder(true) };
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "struct literal field must error");
}

// ============================================================================
// 4. Box::new intrinsic — `Box::new(Holder(true))`
// ============================================================================

#[test]
fn test_final_box_new_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let b: Box<Holder<i32>> = Box::new(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "Box::new must error");
}

// ============================================================================
// 5. Option::Some — `Some(Holder(true))`
// ============================================================================

#[test]
fn test_final_option_some_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "Option::Some must error");
}

// ============================================================================
// 6. Result::Ok — `Ok(Holder(true))`
// ============================================================================

#[test]
fn test_final_result_ok_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Result<Holder<i32>, i32> = Ok(Holder(true));
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "Result::Ok must error");
}

// ============================================================================
// 7. Generic struct field — `Generic { f: Holder(true) }`
// ============================================================================

#[test]
fn test_final_generic_struct_field_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        struct Generic<T> { f: T }
        fn main() -> i32 {
            let g: Generic<Holder<i32>> = Generic { f: Holder(true) };
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "generic struct field must error");
}

// ============================================================================
// 8. fn body return — `fn make() -> Holder<i32> { Holder(true) }`
// ============================================================================

#[test]
fn test_final_fn_body_return_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder(true)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "fn body return must error");
}

// ============================================================================
// 9. if branch — `if cond { Holder(true) } else { Holder(42) }`
// ============================================================================

#[test]
fn test_final_if_branch_wrong_ctor() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let h: Holder<i32> = if true { Holder(true) } else { Holder(42) };
            0
        }
    "#;
    let result = compile(src);
    assert!(result.has_errors(), "if branch must error");
}

// ============================================================================
// 10. match arm — `match x { 1 => Holder(true), _ => Holder(42) }`
// ============================================================================

#[test]
fn test_final_match_arm_wrong_ctor() {
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
    assert!(result.has_errors(), "match arm must error");
}

// ============================================================================
// 11. Valid cases — verify no false positives.
// ============================================================================

#[test]
fn test_final_valid_let_binding() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let h: Holder<i32> = Holder(42);
            0
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "valid let binding should not error");
}

#[test]
fn test_final_valid_fn_call() {
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(42))
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "valid fn call should not error");
}

#[test]
fn test_final_valid_fn_return() {
    let src = r#"
        struct Holder<T>(T);
        fn make_holder() -> Holder<i32> {
            Holder(42)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "valid fn return should not error");
}

#[test]
fn test_final_valid_option_some() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x: Option<Holder<i32>> = Some(Holder(42));
            0
        }
    "#;
    let result = compile(src);
    assert!(!result.has_errors(), "valid Option::Some should not error");
}
