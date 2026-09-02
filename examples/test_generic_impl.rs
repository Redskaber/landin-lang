use landin_compiler::compile;

fn main() {
    // Test user-defined generic struct + impl (not prelude)
    let src = r#"
        struct Container<T> { val: T }
        impl<T> Container<T> {
            fn get_val(&self) -> T { self.val }
        }
        fn main() -> i32 {
            let c: Container<i32> = Container { val: 42i32 };
            c.get_val()
        }
    "#;
    let result = compile(src);
    println!("has_errors: {}", result.has_errors());
    for e in &result.errors.typeck {
        println!("  typeck: {}", e.message);
    }
}
