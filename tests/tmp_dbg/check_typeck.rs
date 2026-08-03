#[test]
fn check_typeck_catches_mismatch() {
    let src = "fn main() { let x: i32 = true; }";
    let result = landin_compiler::driver::compile(src);
    println!("has_errors: {}", result.has_errors());
    println!("typeck errors: {}", result.errors.typeck.len());
    for e in &result.errors.typeck {
        println!("  err: {:?}", e);
    }
    println!("resolve errors: {}", result.errors.resolve.len());
    for e in &result.errors.resolve {
        println!("  res: {:?}", e);
    }
}
