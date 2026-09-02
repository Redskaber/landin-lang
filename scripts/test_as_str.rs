// Quick test: compile a Landin source and print errors
use landin_compiler::{compile, compile_no_opt};

fn main() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _r: &str = s.as_str(); }"#;
    let result = compile(src);
    println!("=== compile result ===");
    println!("has_errors: {}", result.has_errors());
    println!("parse errors: {}", result.errors.parse.len());
    println!("resolve errors: {}", result.errors.resolve.len());
    println!("typeck errors: {}", result.errors.typeck.len());
    println!("borrowck errors: {}", result.errors.borrowck.len());
    println!("codegen errors: {}", result.errors.codegen.len());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
    for e in &result.errors.borrowck {
        println!("  borrowck: {}", e.message);
    }
    for e in &result.errors.codegen {
        println!("  codegen: {:?}", e);
    }
    
    let result2 = compile_no_opt(src);
    println!("\n=== compile_no_opt result ===");
    println!("has_errors: {}", result2.has_errors());
    for e in &result2.errors.typeck {
        println!("  typeck: {}", e.message);
    }
}
