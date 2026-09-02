// Test push_str prelude impl
use landin_compiler::{codegen::codegen_crate, compile_no_opt};

fn main() {
    let src =
        r#"fn main() { let mut s: String = String::from_str("hello"); s.push_str(" world"); }"#;
    let result = compile_no_opt(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
    for e in &result.errors.lower {
        println!("  lower: {:?}", e);
    }
    if result.has_errors() {
        return;
    }
    match codegen_crate(&result) {
        Ok(ir) => {
            // Print landin_main function body
            let mut in_fn = false;
            for line in ir.lines() {
                if line.contains("define") && line.contains("landin_main") {
                    in_fn = true;
                }
                if in_fn {
                    println!("{}", line);
                    if line.trim() == "}" {
                        break;
                    }
                }
            }
        }
        Err(e) => println!("codegen error: {:?}", e),
    }
}
