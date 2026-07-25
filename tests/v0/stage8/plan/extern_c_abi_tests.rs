//! Stage 8.3: extern "C" ABI support tests.
//!
//! Per stage-committee-process.md v3.21 §13.4 + §17.1.
//! Tests extern "C" fn declarations and C ABI codegen.

use landin_compiler::driver::compile;

#[test]
fn stage8_3_extern_c_fn_declaration() {
    // extern "C" fn should parse and compile
    let result = compile(r#"extern "C" fn add(a: i32, b: i32) -> i32 { a + b } fn main() {}"#);
    assert!(
        result.errors.is_empty(),
        "extern C fn should compile: {:?}",
        result.errors
    );
}

#[test]
fn stage8_3_extern_c_fn_called() {
    // Call an extern "C" fn from a regular fn
    let result = compile(
        r#"extern "C" fn double(x: i32) -> i32 { x * 2 } fn main() { let _y = double(21); }"#,
    );
    assert!(
        result.errors.is_empty(),
        "extern C fn call should work: {:?}",
        result.errors
    );
}

#[test]
fn stage8_3_regular_fn_still_works() {
    // Regular Landin fn should still work (regression)
    let result =
        compile("fn add(a: i32, b: i32) -> i32 { a + b } fn main() { let _x = add(1, 2); }");
    assert!(result.errors.is_empty(), "regular fn should still work");
}

#[test]
fn stage8_3_extern_c_void_fn() {
    // extern "C" fn with no return value
    let result = compile(r#"extern "C" fn log(msg: i32) { let _ = msg; } fn main() {}"#);
    assert!(result.errors.is_empty(), "extern C void fn should compile");
}

#[test]
fn stage8_3_extern_c_no_params() {
    // extern "C" fn with no parameters
    let result =
        compile(r#"extern "C" fn get_count() -> i32 { 42 } fn main() { let _x = get_count(); }"#);
    assert!(
        result.errors.is_empty(),
        "extern C no-param fn should compile"
    );
}
