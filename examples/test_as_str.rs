// Print LLVM IR for as_str test
use landin_compiler::{codegen::codegen_crate, compile_no_opt};

fn main() {
    let src = r#"fn main() { let s: String = String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize }; let _r: &str = s.as_str(); }"#;
    let result = compile_no_opt(src);
    if result.has_errors() {
        eprintln!("compile errors: {:?}", result.errors);
        return;
    }
    match codegen_crate(&result) {
        Ok(ir) => {
            // Print all lines containing "bitcast" or "insertvalue" to trace
            for line in ir.lines() {
                if line.contains("bitcast")
                    || line.contains("insertvalue")
                    || line.contains("store { ptr, i64 }")
                {
                    println!("{}", line.trim());
                }
            }
        }
        Err(e) => eprintln!("codegen error: {:?}", e),
    }
}
