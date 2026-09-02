// Simple test: does basic main still work?
use landin_compiler::compile;

fn main() {
    let src = r#"fn main() -> i32 { 7 }"#;
    let result = compile(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
}
