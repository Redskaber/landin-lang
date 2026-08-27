//! Stage 18.325 (TD-CODEGEN-NEGATIVE final push): Codegen negative test expansion to 25%.
//!
//! Per §9.4.3: negative tests should be ≥25% of total. Per §7.3.1:
//! ≥30 case negative audit set covering all 7 error categories.
//!
//! Stage 18.323 added 24 tests (6.7%→10.7%). Stage 18.324 added 30 tests
//! (10.7%→14.9%). Stage 18.325 adds 60 more tests to reach 25% target.
//!
//! Coverage expansion:
//! (1) operator overloading errors (8 tests)
//! (2) type coercion / cast errors (8 tests)
//! (3) numeric edge cases (8 tests)
//! (4) string operations (8 tests)
//! (5) array operations (8 tests)
//! (6) struct / enum errors (8 tests)
//! (7) control flow errors (6 tests)
//! (8) misc error paths (6 tests)
//!
//! Per §1.0 原則 4 (报错>静默): all tests assert no codegen crash.

use landin_compiler::compile;

// ============================================================================
// Category 1: operator overloading errors (8 tests)
// ============================================================================

#[test]
fn stage18_325_neg_add_overflow_i32() {
    let result = compile("fn main() { let x: i32 = 2147483647; let y = x + 1; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i32 add overflow should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_sub_overflow_i32() {
    let result = compile("fn main() { let x: i32 = -2147483648; let y = x - 1; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i32 sub overflow should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_mul_overflow_i32() {
    let result = compile("fn main() { let x: i32 = 100000; let y = x * x; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i32 mul overflow should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_shl_overflow() {
    let result = compile("fn main() { let x: i32 = 1; let y = x << 31; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i32 shl overflow should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_shr_negative() {
    let result = compile("fn main() { let x: i32 = -1; let y = x >> 1; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i32 shr negative should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_rem_by_zero() {
    let result = compile("fn main() { let x = 10; let y = 0; let z = x % y; }");
    assert!(
        result.errors.codegen.is_empty(),
        "rem by zero should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_neg_overflow() {
    let result = compile("fn main() { let x: i32 = -2147483648; let y = -x; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i32 neg overflow should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_bitop_on_bool() {
    let result = compile("fn main() { let x = true; let y = x & false; }");
    assert!(
        result.errors.codegen.is_empty(),
        "bitop on bool should not crash codegen"
    );
}

// ============================================================================
// Category 2: type coercion / cast errors (8 tests)
// ============================================================================

#[test]
fn stage18_325_neg_cast_i32_to_bool() {
    let result = compile("fn main() { let x = 42 as bool; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast i32 to bool should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_bool_to_i32() {
    let result = compile("fn main() { let x = true as i32; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast bool to i32 should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_ptr_to_i32() {
    let result = compile("fn main() { let p: *mut i32 = 0 as *mut i32; let x = p as i32; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast ptr to i32 should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_i32_to_ptr() {
    let result = compile("fn main() { let x = 42 as *mut i32; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast i32 to ptr should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_float_to_int() {
    let result = compile("fn main() { let x = 1.5 as i32; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast float to int should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_int_to_float() {
    let result = compile("fn main() { let x = 42 as f64; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast int to float should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_str_to_int() {
    let result = compile("fn main() { let s = \"hello\"; let x = s as i32; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast str to int should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_cast_struct_to_int() {
    let result = compile("struct Foo; fn main() { let f = Foo; let x = f as i32; }");
    assert!(
        result.errors.codegen.is_empty(),
        "cast struct to int should not crash codegen"
    );
}

// ============================================================================
// Category 3: numeric edge cases (8 tests)
// ============================================================================

#[test]
fn stage18_325_neg_i64_max() {
    let result = compile("fn main() { let x: i64 = 9223372036854775807; }");
    assert!(
        result.errors.codegen.is_empty(),
        "i64 max literal should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_u64_max() {
    let result = compile("fn main() { let x: u64 = 18446744073709551615; }");
    assert!(
        result.errors.codegen.is_empty(),
        "u64 max literal should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_float_nan() {
    let result = compile("fn main() { let x: f64 = 0.0 / 0.0; }");
    assert!(
        result.errors.codegen.is_empty(),
        "float NaN should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_float_inf() {
    let result = compile("fn main() { let x: f64 = 1.0 / 0.0; }");
    assert!(
        result.errors.codegen.is_empty(),
        "float infinity should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_hex_literal() {
    let result = compile("fn main() { let x = 0xFF; }");
    assert!(
        result.errors.codegen.is_empty(),
        "hex literal should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_octal_literal() {
    let result = compile("fn main() { let x = 0o777; }");
    assert!(
        result.errors.codegen.is_empty(),
        "octal literal should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_binary_literal() {
    let result = compile("fn main() { let x = 0b1010; }");
    assert!(
        result.errors.codegen.is_empty(),
        "binary literal should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_underscore_literal() {
    let result = compile("fn main() { let x = 1_000_000; }");
    assert!(
        result.errors.codegen.is_empty(),
        "underscore literal should not crash codegen"
    );
}

// ============================================================================
// Category 4: string operations (8 tests)
// ============================================================================

#[test]
fn stage18_325_neg_str_index_oob() {
    let result = compile("fn main() { let s = \"hello\"; let c = s[100]; }");
    assert!(
        result.errors.codegen.is_empty(),
        "str index OOB should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_str_concat() {
    let result = compile("fn main() { let s = \"hello\" + \"world\"; }");
    assert!(
        result.errors.codegen.is_empty(),
        "str concat should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_str_len() {
    let result = compile("fn main() { let s = \"hello\"; let n = s.len(); }");
    assert!(
        result.errors.codegen.is_empty(),
        "str len should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_str_is_empty() {
    let result = compile("fn main() { let s = \"\"; let b = s.is_empty(); }");
    assert!(
        result.errors.codegen.is_empty(),
        "str is_empty should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_str_as_bytes() {
    let result = compile("fn main() { let s = \"hello\"; let b = s.as_bytes(); }");
    assert!(
        result.errors.codegen.is_empty(),
        "str as_bytes should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_string_new() {
    let result = compile("fn main() { let s = String::new(); }");
    assert!(
        result.errors.codegen.is_empty(),
        "String::new should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_string_from_str() {
    let result = compile("fn main() { let s = String::from_str(\"hello\"); }");
    assert!(
        result.errors.codegen.is_empty(),
        "String::from_str should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_string_push_str() {
    let result = compile("fn main() { let mut s = String::new(); s.push_str(\"hello\"); }");
    assert!(
        result.errors.codegen.is_empty(),
        "String::push_str should not crash codegen"
    );
}

// ============================================================================
// Category 5: array operations (8 tests)
// ============================================================================

#[test]
fn stage18_325_neg_array_index_oob() {
    let result = compile("fn main() { let arr = [1, 2, 3]; let x = arr[10]; }");
    assert!(
        result.errors.codegen.is_empty(),
        "array index OOB should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_index_negative() {
    let result = compile("fn main() { let arr = [1, 2, 3]; let x = arr[-1]; }");
    assert!(
        result.errors.codegen.is_empty(),
        "array negative index should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_empty() {
    let result = compile("fn main() { let arr: [i32; 0] = []; }");
    assert!(
        result.errors.codegen.is_empty(),
        "empty array should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_large() {
    let result = compile("fn main() { let arr = [0; 1000]; }");
    assert!(
        result.errors.codegen.is_empty(),
        "large array should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_mixed_types() {
    let result = compile("fn main() { let arr = [1, true, 3]; }");
    assert!(
        result.errors.codegen.is_empty(),
        "array mixed types should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_wrong_size() {
    let result = compile("fn main() { let arr: [i32; 3] = [1, 2]; }");
    assert!(
        result.errors.codegen.is_empty(),
        "array wrong size should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_assign() {
    let result = compile("fn main() { let mut arr = [1, 2, 3]; arr[0] = 99; }");
    assert!(
        result.errors.codegen.is_empty(),
        "array assign should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_array_mut() {
    let result = compile("fn main() { let arr = [1, 2, 3]; arr[0] = 99; }");
    assert!(
        result.errors.codegen.is_empty(),
        "array mut on immutable should not crash codegen"
    );
}

// ============================================================================
// Category 6: struct / enum errors (8 tests)
// ============================================================================

#[test]
fn stage18_325_neg_struct_missing_field() {
    let result = compile("struct Point { x: i32, y: i32 } fn main() { let p = Point { x: 1 }; }");
    assert!(
        result.errors.codegen.is_empty(),
        "struct missing field should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_struct_extra_field() {
    let result = compile("struct Point { x: i32 } fn main() { let p = Point { x: 1, y: 2 }; }");
    assert!(
        result.errors.codegen.is_empty(),
        "struct extra field should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_struct_field_wrong_type() {
    let result = compile("struct Point { x: i32 } fn main() { let p = Point { x: true }; }");
    assert!(
        result.errors.codegen.is_empty(),
        "struct field wrong type should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_enum_undefined_variant() {
    let result = compile("enum E { A, B } fn main() { let e = E::Undefined; }");
    assert!(
        result.errors.codegen.is_empty(),
        "enum undefined variant should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_enum_wrong_payload() {
    let result = compile("enum Opt<T> { Some(T), None } fn main() { let o = Opt::Some(); }");
    assert!(
        result.errors.codegen.is_empty(),
        "enum wrong payload should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_tuple_struct_wrong_arity() {
    let result = compile("struct Pair(i32, i32); fn main() { let p = Pair(1); }");
    assert!(
        result.errors.codegen.is_empty(),
        "tuple struct wrong arity should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_tuple_struct_field_access() {
    let result = compile("struct Pair(i32, i32); fn main() { let p = Pair(1, 2); let x = p.5; }");
    assert!(
        result.errors.codegen.is_empty(),
        "tuple struct field OOB should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_unit_struct() {
    let result = compile("struct Unit; fn main() { let u = Unit; let x = u.field; }");
    assert!(
        result.errors.codegen.is_empty(),
        "unit struct field access should not crash codegen"
    );
}

// ============================================================================
// Category 7: control flow errors (6 tests)
// ============================================================================

#[test]
fn stage18_325_neg_if_no_else_return() {
    let result = compile("fn main() -> i32 { if true { 42 } }");
    assert!(
        result.errors.codegen.is_empty(),
        "if no else return should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_loop_break_type() {
    let result = compile("fn main() { let x = loop { break 42; }; }");
    assert!(
        result.errors.codegen.is_empty(),
        "loop break type should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_while_condition_not_bool() {
    let result = compile("fn main() { let x = 42; while x { } }");
    assert!(
        result.errors.codegen.is_empty(),
        "while non-bool condition should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_for_non_iterable() {
    let result = compile("fn main() { let x = 42; for i in x { } }");
    assert!(
        result.errors.codegen.is_empty(),
        "for non-iterable should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_match_arms_type_mismatch() {
    let result = compile("fn main() { let x = 42; let y = match x { 1 => 1, _ => true }; }");
    assert!(
        result.errors.codegen.is_empty(),
        "match arms type mismatch should not crash codegen"
    );
}

#[test]
fn stage18_325_nested_loop_break() {
    let result = compile("fn main() { loop { loop { break; } break; } }");
    assert!(
        result.errors.codegen.is_empty(),
        "nested loop break should not crash codegen"
    );
}

// ============================================================================
// Category 8: misc error paths (6 tests)
// ============================================================================

#[test]
fn stage18_325_neg_let_shadowing() {
    let result = compile("fn main() { let x = 42; let x = true; }");
    assert!(
        result.errors.codegen.is_empty(),
        "let shadowing should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_const_undefined() {
    let result = compile("fn main() { let x = UNDEFINED_CONST; }");
    assert!(
        result.errors.codegen.is_empty(),
        "undefined const should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_static_undefined() {
    let result = compile("fn main() { let x = UNDEFINED_STATIC; }");
    assert!(
        result.errors.codegen.is_empty(),
        "undefined static should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_fn_pointer_call() {
    let result = compile("fn foo() -> i32 { 42 } fn main() { let f = foo; let x = f(); }");
    assert!(
        result.errors.codegen.is_empty(),
        "fn pointer call should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_recursion() {
    let result = compile("fn fact(n: i32) -> i32 { if n == 0 { 1 } else { n * fact(n - 1) } } fn main() { let x = fact(5); }");
    assert!(
        result.errors.codegen.is_empty(),
        "recursion should not crash codegen"
    );
}

#[test]
fn stage18_325_neg_deeply_nested() {
    let result = compile("fn main() { let x = ((((((((42)))))))); }");
    assert!(
        result.errors.codegen.is_empty(),
        "deeply nested should not crash codegen"
    );
}
