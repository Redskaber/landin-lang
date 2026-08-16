//! API usage demo: compile a struct + function call, print LLVM IR.
//!
//! Run with: cargo run --example usage/struct_call_codegen
//!
//! Demonstrates the §16-compliant public API:
//!   1. `compile(src)` → `CompileResult`
//!   2. `codegen_crate(&result)` → LLVM IR string
//!
//! Stage 5.5 audit: fixed to use the new single-argument `codegen_crate`
//! API (was `codegen_crate(&hir, &interner)` which was removed in Stage 3.56
//! when codegen became a pure MIR consumer).

use landin_compiler::codegen::codegen_crate;
use landin_compiler::driver::compile;

fn main() {
    let src = "struct Point { x: i32, y: i32 } \
               fn get_x(p: Point) -> i32 { p.x } \
               fn f() -> i32 { get_x(Point { x: 1, y: 2 }) }";
    let result = compile(src);
    let ll = codegen_crate(&result).expect("codegen should succeed for valid test input");
    println!("{}", ll);
}
