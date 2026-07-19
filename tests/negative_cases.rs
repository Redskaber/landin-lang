//! Stage 2.4e negative-case test suite.
//!
//! These tests verify that the compiler correctly *rejects* invalid
//! programs. They were added as a result of the Stage 2.x Phase Gate
//! Review Round 2 (per §9.3 of the Stage Committee Process v3.0),
//! which found that the existing test suite was unbalanced toward
//! positive cases and missed 9 of 13 negative cases.
//!
//! Each test corresponds to a specific G-fix from the gate review:
//!   - G1: HirId mismatch (local variable resolution)
//!   - G2: NLL kill timing
//!   - G3: Call type checking (arg count + types)
//!   - G4: Undefined function detection
//!   - G5: Mutability tracking
//!
//! The remaining 1 missed case (loop_borrow_assign) is a known Stage 3
//! limitation (NLL requires full fixpoint dataflow for loops).

use landin_compiler::driver::compile;

// =====================================================================
// G1: Local variable resolution (HirId mismatch fix)
// =====================================================================

#[test]
fn g1_local_variable_resolves_in_path() {
    // After G1 fix, `let s = "hi"; 1 - s;` correctly errors (Int - Str).
    let src = "fn f() { let s = \"hi\"; 1 - s; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty() || !result.errors.borrowck.is_empty(),
        "expected error for Int - Str mismatch, got 0 errors"
    );
}

#[test]
fn g1_let_bound_variable_usable() {
    // Positive case: `let x = 1; x` should compile cleanly.
    let src = "fn f() -> i32 { let x = 1; x }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for valid let + use"
    );
}

// =====================================================================
// G2: NLL borrow expiry timing
// =====================================================================

#[test]
fn g2_assign_to_borrowed_detected() {
    // `let r = &x; x = 2;` should error (assign to borrowed).
    let src = "fn f() { let mut x = 1; let r = &x; x = 2; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for assign-to-borrowed"
    );
}

#[test]
fn g2_double_mut_borrow_detected() {
    let src = "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &mut x; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for double mut borrow"
    );
}

#[test]
fn g2_mut_borrow_then_shared_detected() {
    let src = "fn f() { let mut x = 1; let r1 = &mut x; let r2 = &x; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for mut-then-shared"
    );
}

#[test]
fn g2_move_borrowed_detected() {
    let src = "fn f() { let s = \"hi\"; let r = &s; let t = s; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for move-borrowed"
    );
}

#[test]
fn g2_shared_borrow_ok() {
    // Positive: `let r = &x; let y = *r;` should compile.
    let src = "fn f() { let x = 1; let r = &x; let y = *r; }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for valid shared borrow"
    );
}

// =====================================================================
// G3: Call type checking (arg count + types)
// =====================================================================

#[test]
fn g3_wrong_arg_count_detected() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() { add(1); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for wrong arg count"
    );
}

#[test]
fn g3_correct_arg_count_ok() {
    let src = "fn add(a: i32, b: i32) -> i32 { a + b } fn main() -> i32 { add(1, 2) }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for correct call"
    );
}

#[test]
fn g3_return_type_unified_with_body() {
    // `fn f() -> bool { 42 }` should error (return type mismatch).
    let src = "fn f() -> bool { 42 }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for return type mismatch"
    );
}

// =====================================================================
// G4: Undefined function/name detection
// =====================================================================

#[test]
fn g4_undefined_function_detected() {
    let src = "fn f() { undefined_fn(); }";
    let result = compile(src);
    assert!(
        !result.errors.resolve.is_empty(),
        "expected resolve error for undefined function"
    );
}

#[test]
fn g4_defined_function_ok() {
    let src = "fn g() {} fn f() { g(); }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for defined function call"
    );
}

// =====================================================================
// G5: Mutability tracking
// =====================================================================

#[test]
fn g5_assign_to_immutable_detected() {
    let src = "fn f() { let x = 1; x = 2; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for assign-to-immutable"
    );
}

#[test]
fn g5_assign_to_mutable_ok() {
    let src = "fn f() { let mut x = 1; x = 2; }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for assign-to-mutable"
    );
}

#[test]
fn g5_let_ascription_mismatch_detected() {
    let src = "fn f() { let x: bool = 42; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for let ascription mismatch"
    );
}

// =====================================================================
// Use-after-move (G1 side-effect — was missed before G1 fix)
// =====================================================================

#[test]
fn g6_use_after_move_str_detected() {
    // Str is not Copy; `let t = s; let u = s;` should error.
    let src = "fn f() { let s = \"hi\"; let t = s; let u = s; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for use-after-move on Str"
    );
}

// =====================================================================
// Type system basics
// =====================================================================

#[test]
fn type_mismatch_int_plus_bool_detected() {
    let src = "fn f() { 1 + true; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Int + Bool"
    );
}

#[test]
fn if_branch_type_mismatch_detected() {
    let src = "fn f() -> i32 { if true { 1 } else { true } }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for if branch type mismatch"
    );
}

#[test]
fn return_unit_as_int_detected() {
    let src = "fn f() -> i32 { () }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for returning () as i32"
    );
}

// =====================================================================
// Known Stage 3 limitations (documented as ignored)
// =====================================================================

#[test]
#[ignore = "Stage 3: NLL requires full fixpoint dataflow for loops"]
fn loop_borrow_assign_stage3_limitation() {
    let src = "fn f() { let mut s = 0; let i = 0; while i < 10 { let r = &s; s = s + *r; } }";
    let result = compile(src);
    // Currently missed — NLL doesn't handle borrows across loop iterations.
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error (currently missed — Stage 3 will fix)"
    );
}

// =====================================================================
// G7 (Stage 2.4f): Type system strictness
// =====================================================================

#[test]
fn g7_bool_plus_bool_detected() {
    // Bool is not arithmetic — `true + false` should error.
    let src = "fn f() { true + false; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for Bool + Bool"
    );
}

#[test]
fn g7_negate_bool_detected() {
    // `-true` should error (Bool is not negatable).
    let src = "fn f() { -true; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for -Bool"
    );
}

#[test]
fn g7_array_elem_mismatch_detected() {
    // `[1, true, 2]` — element types must unify.
    let src = "fn f() { let x = [1, true, 2]; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for array elem mismatch"
    );
}

#[test]
fn g7_array_with_annotation_mismatch_detected() {
    // `let x: [i32; 2] = [1, true];` — bool doesn't unify with i32.
    let src = "fn f() { let x: [i32; 2] = [1, true]; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for array type mismatch"
    );
}

#[test]
fn g7_call_non_function_detected() {
    // `let x = 1; x();` — calling a non-function value.
    let src = "fn f() { let x = 1; x(); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for calling non-function"
    );
}

#[test]
fn g7_if_cond_not_bool_detected() {
    // `if 42 { ... }` — condition must be bool.
    let src = "fn f() { if 42 { 1 } else { 2 } }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for non-bool if condition"
    );
}

#[test]
fn g7_while_cond_not_bool_detected() {
    // `while 42 { ... }` — condition must be bool.
    let src = "fn f() { while 42 { 1; } }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for non-bool while condition"
    );
}

#[test]
fn g7_mut_borrow_immutable_detected() {
    // `let x = 1; let r = &mut x;` — cannot mut-borrow immutable.
    let src = "fn f() { let x = 1; let r = &mut x; }";
    let result = compile(src);
    assert!(
        !result.errors.borrowck.is_empty(),
        "expected borrowck error for mut-borrow of immutable"
    );
}

#[test]
fn g7_not_int_ok() {
    // `!42` is OK (bitwise NOT on int).
    let src = "fn f() { !42; }";
    let result = compile(src);
    assert_eq!(result.errors.total_count(), 0, "expected 0 errors for !Int");
}

#[test]
fn g7_not_bool_ok() {
    // `!true` is OK (logical NOT on bool).
    let src = "fn f() { !true; }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for !Bool"
    );
}

// =====================================================================
// G8 (Stage 2.4g): Resolve-before-check for type system strictness
// =====================================================================

#[test]
fn g8_not_float_detected() {
    // !3.14 should error (Float is not notable).
    // G8 fix: FloatVar is now excluded from is_notable_ty, and
    // infer_rvalue resolves the operand type before checking.
    let src = "fn f() { !3.14; }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for !Float"
    );
}

#[test]
fn g8_negate_tuple_detected() {
    // -(1, 2) should error (Tuple is not negatable).
    // G8 fix: infer_rvalue now resolves operand type before checking
    // is_negatable_ty, so TyVar bound to Tuple is correctly rejected.
    let src = "fn f() { -(1, 2); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for -Tuple"
    );
}

#[test]
fn g8_negate_float_ok() {
    // -3.14 is OK (Float is negatable).
    let src = "fn f() { -3.14; }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for -Float"
    );
}

#[test]
fn g8_not_bool_ok() {
    // !true is OK (Bool is notable).
    let src = "fn f() { !true; }";
    let result = compile(src);
    assert_eq!(
        result.errors.total_count(),
        0,
        "expected 0 errors for !Bool"
    );
}

// =====================================================================
// Stage 3 limitations (documented as ignored)
// =====================================================================

#[test]
#[ignore = "Stage 3: closure type inference (param count not checked)"]
fn closure_wrong_arg_count_stage3_limitation() {
    // apply(|a, b| a + b, 1) — closure has 2 params, fn sig expects 1.
    // Currently missed because closure types aren't inferred against
    // the expected fn signature. Stage 3 (TraitResolver) will fix.
    let src =
        "fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) } fn main() { apply(|a, b| a + b, 1); }";
    let result = compile(src);
    assert!(
        !result.errors.typeck.is_empty(),
        "expected typeck error for closure arg count (currently missed — Stage 3)"
    );
}
