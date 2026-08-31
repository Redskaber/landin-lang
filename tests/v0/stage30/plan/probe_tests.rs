#[test]
fn probe_rule4_case() {
    // Rule 4 case: multiple input lifetimes, no self, elided output
    let src = "fn f(x: &i32, y: &i32) -> &i32 { x } fn main() { let v = 42; let _ = f(&v, &v); }";
    let result = landin_compiler::driver::compile(src);
    println!("=== Rule 4 case (should error) ===");
    println!("errors: {}", result.errors.len());
    for e in &result.errors {
        println!("  - {}", e.message);
    }
}

#[test]
fn probe_no_input_case() {
    // No inputs case: fn f() -> &i32 (should error)
    let src = "fn f() -> &i32 { loop {} } fn main() { }";
    let result = landin_compiler::driver::compile(src);
    println!("=== No input case (should error) ===");
    println!("errors: {}", result.errors.len());
    for e in &result.errors {
        println!("  - {}", e.message);
    }
}

#[test]
fn probe_rule2_case() {
    // Rule 2 case: single input, should pass
    let src = "fn f(x: &i32) -> &i32 { x } fn main() { let v = 42; let _ = f(&v); }";
    let result = landin_compiler::driver::compile(src);
    println!("=== Rule 2 case (should pass) ===");
    println!("errors: {}", result.errors.len());
    for e in &result.errors {
        println!("  - {}", e.message);
    }
}
