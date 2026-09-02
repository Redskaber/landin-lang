use landin_compiler::{codegen::codegen_crate, compile_no_opt};

fn main() {
    let src = r#"fn f(s: &[i32]) -> i32 { s[0] + s[1] }"#;
    let result = compile_no_opt(src);
    if result.has_errors() {
        eprintln!("errors: {:?}", result.errors);
        return;
    }
    match codegen_crate(&result) {
        Ok(ir) => {
            // Print the f function
            let mut in_fn = false;
            for line in ir.lines() {
                if line.contains("define") && line.contains("landin_f") {
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
        Err(e) => eprintln!("codegen error: {:?}", e),
    }
}
