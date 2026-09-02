// Test from_str prelude impl
use landin_compiler::{codegen::codegen_crate, compile_no_opt};

fn main() {
    let src = r#"fn main() { let _s: String = String::from_str("hello"); }"#;
    let result = compile_no_opt(src);
    if result.has_errors() {
        eprintln!("compile errors: {:?}", result.errors);
        return;
    }
    match codegen_crate(&result) {
        Ok(ir) => {
            // Print the from_str function body
            let mut in_fn = false;
            let mut count = 0;
            for line in ir.lines() {
                if line.contains("define") && line.contains("from_str") {
                    in_fn = true;
                }
                if in_fn {
                    println!("{}", line);
                    if line.trim() == "}" {
                        in_fn = false;
                        count += 1;
                        if count >= 1 {
                            break;
                        }
                    }
                }
            }
            // Also check if __landin_alloc is declared
            println!("\n=== __landin_alloc declarations ===");
            for line in ir.lines() {
                if line.contains("__landin_alloc") || line.contains("declare") {
                    println!("{}", line.trim());
                }
            }
        }
        Err(e) => eprintln!("codegen error: {:?}", e),
    }
}
