// Test: user extern "C" + prelude extern "C" conflict
use landin_compiler::{codegen::codegen_crate, compile_no_opt};

fn main() {
    let src = r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(1);
    __landin_dealloc(p);
    0
}
"#;
    let result = compile_no_opt(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
    for e in &result.errors.resolve {
        println!("  resolve: {}", e.message);
    }
    for e in &result.errors.lower {
        println!("  lower: {:?}", e);
    }
    if !result.has_errors() {
        match codegen_crate(&result) {
            Ok(ir) => {
                for line in ir.lines() {
                    if line.contains("declare") && line.contains("__landin_alloc") {
                        println!("IR: {}", line.trim());
                    }
                    if line.contains("define") && line.contains("main") {
                        println!("IR: {}", line.trim());
                    }
                }
            }
            Err(e) => println!("codegen error: {:?}", e),
        }
    }
}
