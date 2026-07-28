//! API usage demo: compile multiple struct variants, check for errors.
//!
//! Run with: cargo run --example usage/struct_compile_check
//!
//! Demonstrates the `compile()` API + `CompileResult.errors` inspection.
//! Does NOT call `codegen_crate` — useful for quick compile-only checks.

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
        ("struct Unit; fn f() { let _u = Unit; }", "unit struct"),
        (
            "struct Empty { } fn f() { let _e = Empty { }; }",
            "empty struct",
        ),
    ];
    for (src, desc) in cases {
        let result = compile(src);
        println!("=== {} ===", desc);
        println!("  errors: {}", result.errors.total_count());
    }
}
