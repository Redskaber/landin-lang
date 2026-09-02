// Check if prelude parses correctly + codegen
use landin_compiler::{codegen::codegen_crate, compile};

fn main() {
    let src = r#"fn main() -> i32 { 0 }"#;
    let result = compile(src);
    println!("has_errors: {}", result.has_errors());
    if result.has_errors() {
        return;
    }
    match codegen_crate(&result) {
        Ok(ir) => {
            // Print all declare lines
            for line in ir.lines() {
                if line.contains("declare") || line.contains("realloc") {
                    println!("{}", line.trim());
                }
            }
        }
        Err(e) => println!("codegen error: {:?}", e),
    }
}
