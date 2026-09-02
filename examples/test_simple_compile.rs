use landin_compiler::compile;

fn main() {
    let src = r#"fn main() -> i32 { 0 }"#;
    let result = compile(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
    for e in &result.errors.lower {
        println!("  lower: {:?}", e);
    }
}
