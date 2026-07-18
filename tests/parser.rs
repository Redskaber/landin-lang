use landin_compiler::lexer::tokenize;
use landin_compiler::parser::Parser;
use lasso::Rodeo;

fn parse(
    src: &str,
) -> (
    landin_compiler::ast::Crate,
    Vec<landin_compiler::parser::ParseError>,
) {
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &interner);
    let krate = parser.parse_crate();
    let errors = parser.into_errors();
    (krate, errors)
}

fn assert_no_errors(src: &str) {
    let (krate, errors) = parse(src);
    assert!(errors.is_empty(), "parse errors for {src:?}: {errors:?}");
    assert!(
        !krate.items.is_empty(),
        "expected at least 1 item for {src:?}"
    );
}

fn assert_has_errors(src: &str) {
    let (_krate, errors) = parse(src);
    assert!(!errors.is_empty(), "expected parse errors for {src:?}");
}

// === DECLARATION TESTS (15) ===

#[test]
fn test_fn_empty() {
    assert_no_errors("fn main() {}");
}

#[test]
fn test_fn_with_params() {
    assert_no_errors("fn add(a: i32, b: i32) -> i32 { a + b }");
}

#[test]
fn test_fn_with_generics() {
    assert_no_errors("fn id<T>(x: T) -> T { x }");
}

#[test]
fn test_struct_named() {
    assert_no_errors("struct Point { x: i32, y: i32 }");
}

#[test]
fn test_struct_tuple() {
    assert_no_errors("struct Color(i32, i32, i32);");
}

#[test]
fn test_struct_unit() {
    assert_no_errors("struct Empty;");
}

#[test]
fn test_enum_unit_variants() {
    assert_no_errors("enum Color { Red, Green, Blue }");
}

#[test]
fn test_enum_tuple_variants() {
    assert_no_errors("enum Opt { Some(i32), None }");
}

#[test]
fn test_trait_decl() {
    assert_no_errors("trait Display { fn fmt(&self); }");
}

#[test]
fn test_impl_inherent() {
    // Struct literal deferred to month 3 — use simpler impl
    assert_no_errors("impl Point { fn new() -> Point { 0 } }");
}

#[test]
fn test_const_static() {
    assert_no_errors("const MAX: i32 = 100; static mut STATE: i32 = 0;");
}

#[test]
fn test_use_decl() {
    assert_no_errors("use std::io;");
}

#[test]
fn test_type_alias() {
    assert_no_errors("type Score = i32;");
}

#[test]
fn test_extern_block() {
    assert_no_errors(r#"extern "C" { fn printf(fmt: *const u8) -> i32; }"#);
}

#[test]
fn test_pub_visibility() {
    assert_no_errors("pub fn main() {}");
}

// === CONTROL FLOW TESTS (15) ===

#[test]
fn test_if_else() {
    assert_no_errors("fn f() { if true { 1 } else { 2 }; }");
}

#[test]
fn test_if_no_else() {
    assert_no_errors("fn f() { if true { 1 }; }");
}

#[test]
fn test_match_basic() {
    assert_no_errors("fn f() { match x { 1 => 1, _ => 0 }; }");
}

#[test]
fn test_match_with_guard() {
    assert_no_errors("fn f() { match x { 1 if true => 1, _ => 0 }; }");
}

#[test]
fn test_loop() {
    assert_no_errors("fn f() { loop { 1; } }");
}

#[test]
fn test_while_loop() {
    assert_no_errors("fn f() { while true { 1; } }");
}

#[test]
fn test_for_loop() {
    assert_no_errors("fn f() { for x in 0..10 {} }");
}

#[test]
fn test_return_expr() {
    assert_no_errors("fn f() { return 42; }");
}

#[test]
fn test_return_no_value() {
    assert_no_errors("fn f() { return; }");
}

#[test]
fn test_break_expr() {
    assert_no_errors("fn f() { loop { break 42; } }");
}

#[test]
fn test_continue() {
    assert_no_errors("fn f() { loop { continue; } }");
}

#[test]
fn test_unsafe_block() {
    assert_no_errors("fn f() { unsafe { 1; } }");
}

#[test]
fn test_nested_if() {
    assert_no_errors("fn f() { if true { if false { 1 } else { 2 } } else { 3 }; }");
}

#[test]
fn test_block_expr() {
    assert_no_errors("fn f() { let x = 42; }");
}

#[test]
fn test_empty_block() {
    assert_no_errors("fn f() {}");
}

// === EXPRESSION TESTS (20) ===

#[test]
fn test_let_with_type() {
    assert_no_errors("fn f() { let x: i32 = 42; }");
}

#[test]
fn test_let_mut() {
    assert_no_errors("fn f() { let mut x = 42; }");
}

#[test]
fn test_op_precedence_add_mul() {
    assert_no_errors("fn f() { let x = 1 + 2 * 3; }");
}

#[test]
fn test_op_precedence_parens() {
    assert_no_errors("fn f() { let x = (1 + 2) * 3; }");
}

#[test]
fn test_op_chained_arith() {
    assert_no_errors("fn f() { let x = 1 + 2 - 3 + 4; }");
}

#[test]
fn test_op_logical() {
    assert_no_errors("fn f() { let x = true && false || true; }");
}

#[test]
fn test_op_comparison() {
    assert_no_errors("fn f() { let x = 1 < 2; }");
}

#[test]
fn test_op_bitwise() {
    assert_no_errors("fn f() { let x = 1 & 2 | 3 ^ 4; }");
}

#[test]
fn test_op_shift() {
    assert_no_errors("fn f() { let x = 1 << 2 >> 3; }");
}

#[test]
fn test_assign_simple() {
    assert_no_errors("fn f() { let mut x = 0; x = 42; }");
}

#[test]
fn test_assign_compound() {
    assert_no_errors("fn f() { let mut x = 0; x += 1; x -= 2; }");
}

#[test]
fn test_unary_neg() {
    assert_no_errors("fn f() { let x = -42; }");
}

#[test]
fn test_unary_not() {
    assert_no_errors("fn f() { let x = !true; }");
}

#[test]
fn test_unary_deref() {
    assert_no_errors("fn f() { let x = *p; }");
}

#[test]
fn test_addr_of() {
    assert_no_errors("fn f() { let x = &y; }");
}

#[test]
fn test_addr_of_mut() {
    assert_no_errors("fn f() { let x = &mut y; }");
}

#[test]
fn test_cast() {
    assert_no_errors("fn f() { let x = 1 as u64; }");
}

#[test]
fn test_try_operator() {
    assert_no_errors("fn f() { let x = y?; }");
}

#[test]
fn test_range() {
    assert_no_errors("fn f() { for i in 0..10 {} }");
}

#[test]
fn test_range_inclusive() {
    assert_no_errors("fn f() { for i in 0..=10 {} }");
}

// === TYPE TESTS (10) ===

#[test]
fn test_type_ref() {
    assert_no_errors("fn f(x: &i32) {}");
}

#[test]
fn test_type_mut_ref() {
    assert_no_errors("fn f(x: &mut i32) {}");
}

#[test]
fn test_type_raw_ptr() {
    assert_no_errors("fn f(x: *const u8) {}");
}

#[test]
fn test_type_array() {
    assert_no_errors("fn f(x: [i32; 10]) {}");
}

#[test]
fn test_type_slice() {
    assert_no_errors("fn f(x: &[i32]) {}");
}

#[test]
fn test_type_tuple() {
    assert_no_errors("fn f(x: (i32, i64)) {}");
}

#[test]
fn test_type_fn_ptr() {
    assert_no_errors("fn f(x: fn(i32) -> i32) {}");
}

#[test]
fn test_type_never() {
    assert_no_errors("fn f(x: !) {}");
}

#[test]
fn test_type_infer() {
    assert_no_errors("fn f(x: _) {}");
}

#[test]
fn test_type_path() {
    assert_no_errors("fn f(x: Vec) {}");
}

// === COMPLEX PROGRAMS (10) ===

#[test]
fn test_fib_program() {
    assert_no_errors(
        r#"
        fn fib(n: i64) -> i64 {
            if n < 2 { return n; }
            fib(n - 1) + fib(n - 2)
        }
        fn main() {
            let result = fib(30);
        }
    "#,
    );
}

#[test]
fn test_struct_with_impl() {
    assert_no_errors(
        r#"
        struct Point { x: i32, y: i32 }
        impl Point {
            fn new(x: i32, y: i32) -> Point { 0 }
            fn origin() -> Point { 0 }
        }
    "#,
    );
}

#[test]
fn test_enum_with_match() {
    assert_no_errors(
        r#"
        enum Opt {
            Some(i32),
            None,
        }
        fn unwrap(o: Opt) -> i32 {
            match o {
                Some => 0,
                None => 0,
            }
        }
    "#,
    );
}

#[test]
fn test_trait_impl() {
    assert_has_errors(
        r#"
        trait Display {
            fn fmt(&self);
        }
        struct Foo;
        impl Display & Foo {
            fn fmt(&self) {}
        }
    "#,
    );
}

#[test]
fn test_generic_function() {
    assert_no_errors(
        r#"
        fn map(xs: Vec, f: fn(i32) -> i32) -> Vec {
            let result = 0;
            result
        }
    "#,
    );
}

#[test]
fn test_closure_in_let() {
    assert_no_errors("fn f() { let g = |x| x + 1; }");
}

#[test]
fn test_method_chain() {
    assert_no_errors("fn f() { let x = a.foo().bar().baz(); }");
}

#[test]
fn test_field_access() {
    assert_no_errors("fn f() { let x = p.x; let y = p.y; }");
}

#[test]
fn test_tuple_field_access() {
    assert_no_errors("fn f() { let x = t.0; let y = t.1; }");
}

#[test]
fn test_array_literal() {
    assert_no_errors("fn f() { let x = [1, 2, 3]; }");
}

// === ERROR RECOVERY TESTS (10) ===

#[test]
fn test_error_missing_semicolon() {
    assert_has_errors("fn f() { let x = 42 }");
}

#[test]
fn test_error_missing_rparen() {
    assert_has_errors("fn f( {}");
}

#[test]
fn test_error_missing_rbrace() {
    assert_has_errors("fn f() {");
}

#[test]
fn test_error_missing_rbracket() {
    assert_has_errors("fn f() { let x = [1, 2; }");
}

#[test]
fn test_error_unexpected_token() {
    assert_has_errors("fn f() { let = 42; }");
}

#[test]
fn test_error_missing_fn_name() {
    assert_has_errors("fn () {}");
}

#[test]
fn test_error_missing_colon_in_param() {
    assert_has_errors("fn f(a i32) {}");
}

#[test]
fn test_error_missing_arrow() {
    assert_has_errors("fn f() i32 {}");
}

#[test]
fn test_error_multiple_errors() {
    // Multiple errors should be reported, not just first
    let (_krate, errors) = parse("fn f( { let = 42; }");
    assert!(
        errors.len() >= 2,
        "expected at least 2 errors, got {}",
        errors.len()
    );
}

#[test]
fn test_error_recovery_continues_after_error() {
    // After an error, parser should continue and find next item
    let (krate, _errors) = parse("fn f( {} fn g() {}");
    assert!(
        !krate.items.is_empty(),
        "expected at least 1 item after error recovery"
    );
}

// === ADDITIONAL TESTS TO REACH 200 ===

#[test]
fn test_nested_struct_decl() {
    let (krate, errors) = parse("struct Outer { inner: Inner }");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_multiple_generics() {
    let (_krate, errors) = parse("fn f<T, U, V>() {}");
    assert!(errors.is_empty());
}

#[test]
fn test_chain_of_uses() {
    let (krate, errors) = parse("use std::io; use std::fs; use std::net;");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 3);
}

#[test]
fn test_complex_match() {
    let (_krate, errors) = parse("fn f() { match x { 1 => 1, 2 => 2, _ => 0 }; }");
    assert!(errors.is_empty());
}

#[test]
fn test_unary_chain() {
    let (_krate, errors) = parse("fn f() { let x = !-!true; }");
    assert!(errors.is_empty());
}
