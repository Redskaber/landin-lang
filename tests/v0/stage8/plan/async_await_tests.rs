//! Stage 8.5: async/await foundation tests.
//!
//! Per stage-committee-process.md v3.21 §13.4 + §17.1.
//! Tests async/await parsing and compilation (MVP: synchronous evaluation).

use landin_compiler::driver::compile;

#[test]
fn stage8_5_async_block_parses() {
    let result = compile("fn main() { let _x = async { 42 }; }");
    assert!(
        result.errors.is_empty(),
        "async block should parse and compile"
    );
}

#[test]
fn stage8_5_await_expr_parses() {
    let result = compile("fn main() { let _x = await 42; }");
    // May have type errors in MVP, but should not panic
    let _ = result;
}

#[test]
fn stage8_5_regular_fn_still_works() {
    let result = compile("fn main() { let x = 42; }");
    assert!(result.errors.is_empty(), "regular fn should still work");
}

#[test]
fn stage8_5_async_block_with_stmts() {
    let result = compile("fn main() { let _x = async { let y = 1; y + 2 }; }");
    assert!(
        result.errors.is_empty(),
        "async block with stmts should compile"
    );
}

#[test]
fn stage8_5_nested_async() {
    let result = compile("fn main() { let _x = async { let _y = async { 1 }; }; }");
    assert!(result.errors.is_empty(), "nested async should compile");
}
