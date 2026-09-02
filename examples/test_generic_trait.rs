use landin_compiler::compile;
fn main() {
    let src = "trait T { fn f(&self) -> i32; } struct S<X> { x: X } impl<X: T> T for S<X> { fn f(&self) -> i32 { self.x.f() } } fn main() { 0 }";
    let result = compile(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
}
