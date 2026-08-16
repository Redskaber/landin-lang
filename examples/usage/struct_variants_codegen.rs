//! API usage demo: compile multiple struct variants, print LLVM IR.
//!
//! Run with: cargo run --example usage/struct_variants_codegen
//!
//! Demonstrates the §16-compliant public API on named structs, tuple
//! structs, and field access patterns.
//!
//! Stage 5.5 audit: fixed to use the new single-argument `codegen_crate`
//! API (was `codegen_crate(&hir, &interner)` which was removed in Stage 3.56).

use landin_compiler::codegen::codegen_crate;
use landin_compiler::driver::compile;

fn main() {
    let cases = [
        (
            "struct Point { x: i32, y: i32 } fn f() -> i32 { let p = Point { x: 1, y: 2 }; p.x }",
            "named struct + field access",
        ),
        (
            "struct Pair(i32, i64); fn f() -> i64 { let p = Pair(1, 2); p.0 }",
            "tuple struct + field access",
        ),
    ];
    for (src, desc) in cases {
        println!("========== {} ==========", desc);
        let result = compile(src);
        let ll = codegen_crate(&result).expect("codegen should succeed for valid test input");
        println!("{}", ll);
    }
}
